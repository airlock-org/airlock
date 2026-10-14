use crate::{
    Decision,
    approval::Session,
    diff_context::{ContextView, DisplayRow, Section, adjacent_section},
    effects::{self, PlannedEffects, names_text},
    file_tree::{FileTree, RowKind, TreeRow},
    icons::{self, Icon},
    model::{FileChange, Review},
    selection::{self, SourceSelection, copyable_label, read_only_text},
    syntax,
    theme::{
        self, ACCENT, ACCENT_TINT, BACKGROUND, BORDER, BRAND, BUTTON_TEXT, FOREGROUND, GREEN,
        MUTED, PANEL, RED,
    },
};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeSet;
use std::path::PathBuf;

struct CodeLine {
    number: usize,
    source_start: usize,
    job: egui::text::LayoutJob,
    kind: ChangeTag,
}

struct DiffRow {
    left: Option<CodeLine>,
    right: Option<CodeLine>,
}

fn operations(file: &FileChange) -> Vec<similar::DiffOp> {
    TextDiff::configure()
        .timeout(std::time::Duration::from_millis(250))
        .diff_lines(&file.before, &file.after)
        .ops()
        .to_vec()
}

fn rows(file: &FileChange, ops: &[similar::DiffOp]) -> Vec<DiffRow> {
    let starts = |source: &str| {
        let mut offset = 0;
        source
            .split_inclusive('\n')
            .map(|line| {
                let start = offset;
                offset += line.len();
                start
            })
            .collect::<Vec<_>>()
    };
    let before_starts = starts(&file.before);
    let after_starts = starts(&file.after);
    let before_syntax = syntax::highlight(&file.path, &file.before);
    let after_syntax = syntax::highlight(&file.path, &file.after);
    let diff = TextDiff::configure()
        .timeout(std::time::Duration::from_millis(250))
        .diff_lines(&file.before, &file.after);
    let mut result = Vec::new();
    for op in ops {
        let mut deleted = Vec::new();
        let mut inserted = Vec::new();
        for change in diff.iter_inline_changes(op) {
            let segments = change
                .iter_strings_lossy()
                .map(|(emphasized, value)| {
                    (
                        emphasized,
                        value.trim_end_matches('\n').replace('\t', "    "),
                    )
                })
                .collect::<Vec<_>>();
            let highlighted = if let Some(index) = change.old_index() {
                &before_syntax[index]
            } else {
                &after_syntax[change.new_index().unwrap()]
            };
            let line = CodeLine {
                number: change.old_index().or(change.new_index()).unwrap_or(0) + 1,
                source_start: if let Some(index) = change.old_index() {
                    before_starts[index]
                } else {
                    after_starts[change.new_index().unwrap()]
                },
                job: syntax::diff_line(highlighted, &segments, change.tag()),
                kind: change.tag(),
            };
            match change.tag() {
                ChangeTag::Equal => {
                    let right = CodeLine {
                        number: change.new_index().unwrap_or(0) + 1,
                        source_start: after_starts[change.new_index().unwrap()],
                        job: after_syntax[change.new_index().unwrap()].clone(),
                        kind: ChangeTag::Equal,
                    };
                    result.push(DiffRow {
                        left: Some(line),
                        right: Some(right),
                    });
                }
                ChangeTag::Delete => deleted.push(line),
                ChangeTag::Insert => inserted.push(line),
            }
        }
        let count = deleted.len().max(inserted.len());
        let mut deleted = deleted.into_iter();
        let mut inserted = inserted.into_iter();
        for _ in 0..count {
            result.push(DiffRow {
                left: deleted.next(),
                right: inserted.next(),
            });
        }
    }
    result
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Source,
    Plan,
    Record,
}

pub struct Controller {
    logo: egui::TextureHandle,
    review: Review,
    clipboard_review: Result<String, String>,
    effects: Option<PlannedEffects>,
    selected: usize,
    tab: Tab,
    filter: String,
    filter_has_focus: bool,
    file_tree: FileTree,
    collapsed_folders: BTreeSet<String>,
    scroll_to_selection: bool,
    diff_rows: Vec<DiffRow>,
    diff_ops: Vec<Vec<similar::DiffOp>>,
    context_views: Vec<ContextView>,
    sections: Vec<(usize, Section)>,
    active_section: Option<usize>,
    scroll_to_section: Option<usize>,
    source_selection: SourceSelection,
    confirm: Option<Decision>,
    about_open: bool,
    approval: Option<Session>,
    receipt: Option<serde_json::Value>,
    copy_feedback: Option<(&'static str, f64)>,
    notice: Option<String>,
    decisions_dir: Option<PathBuf>,
    screenshot: Option<PathBuf>,
    frames: usize,
    screenshot_requested: bool,
}

impl Controller {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        review: Review,
        decisions_dir: Option<PathBuf>,
        screenshot: Option<PathBuf>,
        approval: Option<Session>,
    ) -> Self {
        theme::apply(&cc.egui_ctx);
        let clipboard_review = review
            .clipboard_report()
            .map_err(|error| format!("{error:#}"));
        let effects = review.plan().map(PlannedEffects::from_plan);
        let diff_ops: Vec<_> = review.files.iter().map(operations).collect();
        let diff_rows = review
            .files
            .first()
            .map(|file| rows(file, &diff_ops[0]))
            .unwrap_or_default();
        let mut sections = Vec::new();
        let context_views = diff_ops
            .iter()
            .enumerate()
            .map(|(file, ops)| {
                let flags: Vec<_> = ops
                    .iter()
                    .flat_map(|op| {
                        std::iter::repeat_n(
                            op.tag() != similar::DiffTag::Equal,
                            op.old_range().len().max(op.new_range().len()),
                        )
                    })
                    .collect();
                let view = ContextView::new(flags);
                sections.extend(view.sections.iter().map(|section| (file, *section)));
                view
            })
            .collect();
        let active_section = sections.iter().position(|(file, _)| *file == 0);
        let file_tree = FileTree::new(&review.files);
        let icon = crate::branding::logo()
            .expect("the embedded logo was checked before opening the window");
        let logo = cc.egui_ctx.load_texture(
            "airlock-logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        Self {
            logo,
            review,
            clipboard_review,
            effects,
            selected: 0,
            tab: Tab::Source,
            filter: String::new(),
            filter_has_focus: false,
            file_tree,
            collapsed_folders: BTreeSet::new(),
            scroll_to_selection: false,
            diff_rows,
            diff_ops,
            context_views,
            sections,
            active_section,
            scroll_to_section: None,
            source_selection: SourceSelection::default(),
            confirm: None,
            about_open: false,
            approval,
            receipt: None,
            copy_feedback: None,
            notice: None,
            decisions_dir,
            screenshot,
            frames: 0,
            screenshot_requested: false,
        }
    }

    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(20.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(44.0)));
                    ui.vertical(|ui| {
                        ui.heading(RichText::new("Airlock Control").color(BRAND));
                        ui.label(
                            RichText::new(
                                if self.approval.is_some() { "Approve the source and permitted impact before evaluation or building." }
                                else { "Compare configuration changes and inspect the deployment plan before approval." },
                            )
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icons::button(ui, Icon::Info, "About Airlock Control", true).clicked() {
                            self.about_open = true;
                        }
                        let label = if self.approval.is_some() { "APPROVAL REQUIRED" } else if self.review.mode == "demo" {
                            "LOCAL PREVIEW"
                        } else {
                            "LOCAL REVIEW"
                        };
                        egui::Frame::new()
                            .fill(ACCENT_TINT)
                            .stroke(Stroke::new(1.0_f32, ACCENT))
                            .inner_margin(8.0)
                            .corner_radius(5.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new(label).color(ACCENT).strong().size(12.0));
                            });
                    });
                });
            });
    }

    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("decision")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(18.0))
            .show(ctx, |ui| {
                if let Some(notice) = &self.notice {
                    ui.label(RichText::new(notice).color(ACCENT));
                }
                ui.horizontal(|ui| {
                    let effects_width = (ui.available_width() - 190.0).max(100.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(effects_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            if let Some(session) = &self.approval {
                                ui.label(
                                    RichText::new(format!(
                                        "Permitted VM changes: {}",
                                        session.allowed_vms
                                    ))
                                    .size(12.0),
                                );
                                ui.label(
                                    RichText::new(if session.allow_reboot {
                                        "System reboot: Permitted"
                                    } else {
                                        "System reboot: Not permitted"
                                    })
                                    .size(12.0)
                                    .color(MUTED),
                                );
                            } else {
                                effects_summary(ui, self.effects.as_ref());
                            }
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let enabled = self.receipt.is_none() && self.blocker().is_none();
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new(RichText::new("Approve").color(BUTTON_TEXT))
                                    .fill(ACCENT),
                            )
                            .clicked()
                        {
                            if let Some(session) = &self.approval {
                                session.finish(&self.review, "approve");
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else {
                                self.confirm = Some(Decision::Approve);
                            }
                        }
                        if ui
                            .add_enabled(self.receipt.is_none(), egui::Button::new("Reject"))
                            .clicked()
                        {
                            if let Some(session) = &self.approval {
                                session.finish(&self.review, "reject");
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else {
                                self.confirm = Some(Decision::Reject);
                            }
                        }
                    });
                });
                if self.receipt.is_none()
                    && let Some(reason) = self.blocker()
                {
                    ui.add(egui::Label::new(RichText::new(reason).size(12.0).color(ACCENT)).wrap());
                }
            });
    }

    fn blocker(&self) -> Option<&str> {
        if let Some(session) = &self.approval {
            if session.scope().is_none() {
                return Some("Enter valid permitted VM names in Deployment plan.");
            }
            if self.review.files.iter().any(|file| file.opaque.is_some()) {
                return Some("Some changed files cannot be displayed. Approval is disabled.");
            }
            None
        } else {
            self.review.approval_blocker()
        }
    }

    fn about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let frame = egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(10.0)
            .inner_margin(24.0)
            .shadow(ctx.style().visuals.window_shadow);
        let dialog = egui::Modal::new(egui::Id::new("about-airlock-control"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.vertical_centered(|ui| {
                    ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(64.0)));
                    ui.add_space(6.0);
                    ui.heading(RichText::new("Airlock Control").color(BRAND));
                    ui.label(
                        RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .size(12.0)
                            .color(MUTED),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Compare configuration changes and inspect deployment effects.")
                            .size(13.0)
                            .color(MUTED),
                    );
                });
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);
                for (title, description) in [
                    (
                        "Deployment effects",
                        if self.review.mode == "demo" {
                            "The deployment plan is an example."
                        } else if self.review.deployment.is_some() {
                            "Shown from the supplied deployment plan."
                        } else {
                            "No deployment plan is loaded."
                        },
                    ),
                    (
                        "Review decisions",
                        if self.approval.is_some() { "Approve permits evaluation, building and deployment of this source within the permitted impact. Reject returns feedback to the assistant." }
                        else { "No machine is connected. Approve and Reject record your decision locally without starting deployment." },
                    ),
                    (
                        "Decision storage",
                        if self.approval.is_some() { "The controller retains the request and outcome. Closing this window cancels approval." } else if self.review.mode == "demo" {
                            "Example decisions stay in memory for this session."
                        } else if self.decisions_dir.is_some() {
                            "Decision receipts are saved to the configured local directory."
                        } else {
                            "Decisions stay in memory for this session."
                        },
                    ),
                ] {
                    ui.label(RichText::new(title).strong().size(13.0));
                    ui.label(RichText::new(description).size(13.0).color(MUTED));
                    ui.add_space(10.0);
                }
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        ui.close();
                    }
                });
            });
        if dialog.should_close() {
            self.about_open = false;
        }
    }

    fn review_title(&self, ui: &mut egui::Ui) {
        ui.add(
            egui::Label::new(
                RichText::new(&self.review.candidate.subject)
                    .strong()
                    .size(15.0),
            )
            .truncate(),
        )
        .on_hover_text(&self.review.candidate.subject)
        .context_menu(|ui| {
            if ui.button("Copy change title").clicked() {
                ui.ctx().copy_text(self.review.candidate.subject.clone());
                ui.close();
            }
        });
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("files")
            .default_width(330.0)
            .min_width(270.0)
            .max_width(520.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(14.0))
            .show(ctx, |ui| {
                let all_rows =
                    self.file_tree
                        .rows(&self.review.files, &self.filter, &BTreeSet::new());
                let matching: Vec<_> = all_rows
                    .iter()
                    .filter_map(|row| match row.kind {
                        RowKind::File { index } => Some(index),
                        _ => None,
                    })
                    .collect();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Changed files").strong().size(15.0));
                    let count = if self.filter.trim().is_empty() {
                        self.review.files.len().to_string()
                    } else {
                        format!("{} / {}", matching.len(), self.review.files.len())
                    };
                    ui.label(RichText::new(count).size(12.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let removed: usize =
                            self.review.files.iter().map(|file| file.deletions).sum();
                        let added: usize =
                            self.review.files.iter().map(|file| file.additions).sum();
                        ui.label(
                            RichText::new(format!("−{removed}"))
                                .monospace()
                                .size(11.0)
                                .color(RED),
                        );
                        ui.label(
                            RichText::new(format!("+{added}"))
                                .monospace()
                                .size(11.0)
                                .color(GREEN),
                        );
                    });
                });
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 36.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let clear_width = icons::BUTTON_SIZE;
                    let margin = egui::Margin::symmetric(10, 8);
                    let text_width = (ui.available_width() - clear_width - ui.spacing().item_spacing.x - margin.sum().x).max(40.0);
                    let filter_id = ui.make_persistent_id("file-filter");
                    let previous_cursor = egui::TextEdit::load_state(ui.ctx(), filter_id)
                        .and_then(|state| state.cursor.char_range());
                    let mut filter = egui::TextEdit::singleline(&mut self.filter)
                            .id(filter_id)
                            .hint_text("Find a file or folder…")
                            .margin(margin)
                            .min_size(Vec2::new(0.0, 36.0))
                            .vertical_align(egui::Align::Center)
                            .desired_width(text_width)
                            .show(ui);
                    self.filter_has_focus = filter.response.has_focus();
                    if filter.response.contains_pointer() && ui.input(|input| input.pointer.secondary_pressed()) {
                        filter.state.cursor.set_char_range(previous_cursor);
                        filter.state.clone().store(ui.ctx(), filter_id);
                    }
                    selection::text_edit_menu(&mut filter, &self.filter, true);
                    if icons::button(ui, Icon::Clear, "Clear file filter", !self.filter.is_empty()).clicked()
                    {
                        self.filter.clear();
                    }
                });
                icons::row(ui, |ui| {
                    let searching = !self.filter.trim().is_empty();
                    if icons::button(ui, Icon::CollapseAll, "Collapse all folders", !searching).clicked()
                    {
                        self.collapsed_folders = all_rows
                            .iter()
                            .filter_map(|row| match row.kind {
                                RowKind::Folder { .. } => Some(row.path.clone()),
                                _ => None,
                            })
                            .collect();
                    }
                    if icons::button(ui, Icon::ExpandAll, "Expand all folders", !searching).clicked()
                    {
                        self.collapsed_folders.clear();
                    }
                });
                ui.add_space(6.0);
                let visible =
                    self.file_tree
                        .rows(&self.review.files, &self.filter, &self.collapsed_folders);
                let mut select = None;
                let mut navigate = None;
                let tree_height = (ui.available_height() - icons::BUTTON_SIZE - 28.0).max(100.0);
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if visible.is_empty() {
                        ui.add_space(18.0);
                        ui.label(
                            RichText::new(if self.review.files.is_empty() {
                                "No changed files."
                            } else {
                                "No files match this filter."
                            })
                            .color(MUTED),
                        );
                    }
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt("file-tree")
                        .auto_shrink([false, false])
                        .max_height(tree_height);
                    if self.scroll_to_selection {
                        if let Some(position) = visible.iter().position(|row| matches!(row.kind, RowKind::File { index } if index == self.selected)) {
                            scroll = scroll.vertical_scroll_offset((position as f32 * 29.0 - tree_height / 2.0).max(0.0));
                        }
                        self.scroll_to_selection = false;
                    }
                    scroll.show_rows(ui, 29.0, visible.len(), |ui, range| {
                            for index in range {
                                let row = &visible[index];
                                let response = tree_row(ui, row, &self.review.files, self.selected);
                                if response.clicked() {
                                    response.request_focus();
                                }
                                let activated = response.clicked()
                                    || response.has_focus()
                                        && ui.input(|input| {
                                            input.key_pressed(egui::Key::Enter)
                                                || input.key_pressed(egui::Key::Space)
                                        });
                                if response.has_focus() {
                                    if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
                                        navigate = Some(true);
                                    }
                                    if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
                                        navigate = Some(false);
                                    }
                                }
                                if activated {
                                    match row.kind {
                                        RowKind::File { index } => select = Some(index),
                                        RowKind::Folder { open, .. }
                                            if self.filter.trim().is_empty() =>
                                        {
                                            if open {
                                                self.collapsed_folders.insert(row.path.clone());
                                            } else {
                                                self.collapsed_folders.remove(&row.path);
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                            }
                        });
                });
                ui.separator();
                icons::row(ui, |ui| {
                    let position = matching.iter().position(|index| *index == self.selected);
                    ui.label(
                        RichText::new(
                            position
                                .map(|position| format!("{} of {}", position + 1, matching.len()))
                                .unwrap_or_else(|| "No selection in filter".into()),
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icons::button(ui, Icon::Next, "Next changed file",
                                !matching.is_empty()
                                    && position
                                        .is_none_or(|position| position + 1 < matching.len()),
                            ).clicked()
                        {
                            navigate = Some(true);
                        }
                        if icons::button(ui, Icon::Previous, "Previous changed file",
                                !matching.is_empty()
                                    && position.is_none_or(|position| position > 0),
                            ).clicked()
                        {
                            navigate = Some(false);
                        }
                    });
                    if let Some(forward) = navigate {
                        let position = match (forward, position) {
                            (true, Some(position)) => {
                                (position + 1).min(matching.len().saturating_sub(1))
                            }
                            (false, Some(position)) => position.saturating_sub(1),
                            (true, None) => 0,
                            (false, None) => matching.len().saturating_sub(1),
                        };
                        select = matching.get(position).copied();
                    }
                });
                if let Some(index) = select {
                    self.file_tree
                        .reveal(&self.review.files, index, &mut self.collapsed_folders);
                    self.select_file(index);
                    if navigate.is_some() {
                        self.scroll_to_selection = true;
                        ctx.memory_mut(|memory| memory.request_focus(tree_id(&self.review.files[index].path)));
                    }
                }
            });
    }

    fn select_file(&mut self, index: usize) {
        if self.selected != index {
            self.selected = index;
            self.source_selection = SourceSelection::default();
            self.diff_rows = rows(&self.review.files[index], &self.diff_ops[index]);
        }
        self.active_section = self.sections.iter().position(|(file, _)| *file == index);
        self.scroll_to_section = None;
        self.tab = Tab::Source;
    }

    fn navigate_section(&mut self, forward: bool) {
        let Some(index) =
            adjacent_section(&self.sections, self.active_section, self.selected, forward)
        else {
            return;
        };
        let (file, section) = self.sections[index];
        if file != self.selected {
            self.select_file(file);
            // Navigation covers the complete review, including filtered files.
            self.filter.clear();
            self.file_tree
                .reveal(&self.review.files, file, &mut self.collapsed_folders);
            self.scroll_to_selection = true;
        }
        self.active_section = Some(index);
        self.scroll_to_section = Some(section.first_change);
    }

    fn section_toolbar(&mut self, ui: &mut egui::Ui) {
        icons::row(ui, |ui| {
            let previous =
                adjacent_section(&self.sections, self.active_section, self.selected, false)
                    .is_some();
            let next = adjacent_section(&self.sections, self.active_section, self.selected, true)
                .is_some();
            if icons::button(
                ui,
                Icon::Previous,
                "Previous diff section (Shift+F7)",
                previous,
            )
            .clicked()
            {
                self.navigate_section(false);
            }
            if icons::button(ui, Icon::Next, "Next diff section (F7)", next).clicked() {
                self.navigate_section(true);
            }
            let counter = self.active_section.map_or_else(
                || format!("{} diff sections", self.sections.len()),
                |index| format!("Section {} of {}", index + 1, self.sections.len()),
            );
            ui.label(RichText::new(counter).size(12.0).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::CollapseAll, "Reset to 3 lines of context", true)
                    .clicked()
                {
                    self.context_views[self.selected].reset();
                    self.source_selection = SourceSelection::default();
                    self.scroll_to_section = self
                        .active_section
                        .map(|index| self.sections[index].1.first_change);
                }
                if icons::button(ui, Icon::ExpandAll, "Show entire file", true).clicked() {
                    self.context_views[self.selected].show_all();
                }
            });
        });
    }

    fn source(&mut self, ui: &mut egui::Ui) {
        if !self.review.files.is_empty() {
            self.section_toolbar(ui);
        }
        let Some(file) = self.review.files.get(self.selected) else {
            ui.centered_and_justified(|ui| {
                ui.label("No source changes between these commits.");
            });
            return;
        };
        ui.horizontal(|ui| {
            copyable_label(
                ui,
                RichText::new(visible_path(&file.path)).strong().monospace(),
                &file.path,
            );
            ui.label(RichText::new(format!("+{}", file.additions)).color(GREEN));
            ui.label(RichText::new(format!("−{}", file.deletions)).color(RED));
        });
        if file.old_mode != file.new_mode {
            ui.label(
                RichText::new(format!(
                    "File mode: {} -> {}",
                    file.old_mode.as_deref().unwrap_or("absent"),
                    file.new_mode.as_deref().unwrap_or("absent")
                ))
                .color(ACCENT),
            );
        }
        if let Some(reason) = &file.opaque {
            ui.label(RichText::new(reason).color(RED));
            ui.monospace(format!(
                "Before object: {}",
                file.old_oid.as_deref().unwrap_or("absent")
            ));
            ui.monospace(format!(
                "After object:  {}",
                file.new_oid.as_deref().unwrap_or("absent")
            ));
            return;
        }
        ui.separator();
        for (label, content) in [("Baseline", &file.before), ("Candidate", &file.after)] {
            if !content.is_empty() && !content.ends_with('\n') {
                ui.label(
                    RichText::new(format!("{label} has no final newline"))
                        .color(MUTED)
                        .size(12.0),
                );
            }
        }
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.columns(2, |columns| {
                column_header(
                    &mut columns[0],
                    "Baseline",
                    MUTED,
                    file,
                    0,
                    &self.source_selection,
                );
                column_header(
                    &mut columns[1],
                    "Candidate",
                    ACCENT,
                    file,
                    1,
                    &self.source_selection,
                );
            });
        });
        let available = ui.available_width();
        let max_chars = self.context_views[self.selected]
            .entries
            .iter()
            .filter_map(|entry| match entry {
                DisplayRow::Source(index) => Some(&self.diff_rows[*index]),
                DisplayRow::Gap { .. } => None,
            })
            .flat_map(|row| [&row.left, &row.right])
            .filter_map(|line| line.as_ref())
            .map(|line| line.job.text.chars().count())
            .max()
            .unwrap_or(0);
        let character_width =
            ui.fonts_mut(|fonts| fonts.glyph_width(&FontId::monospace(syntax::FONT_SIZE), ' '));
        let column_width =
            (available / 2.0).max(max_chars as f32 * character_width + syntax::TEXT_LEFT + 16.0);
        // Keep selection focus alive when its starting row scrolls out of the virtual list.
        let focus_ids = [0, 1].map(|side| ui.id().with(("source-selection", self.selected, side)));
        for id in focus_ids {
            ui.interact(
                egui::Rect::NOTHING,
                id,
                egui::Sense::focusable_noninteractive(),
            );
        }
        self.source_selection
            .keyboard(ui, [&file.before, &file.after]);
        let entries = &self.context_views[self.selected].entries;
        let active_range = self.active_section.map(|index| {
            let section = self.sections[index].1;
            section.start..section.end
        });
        let target = self.scroll_to_section.take().and_then(|row| {
            entries
                .iter()
                .position(|entry| *entry == DisplayRow::Source(row))
        });
        let mut expansion = None;
        let scroll = ui
            .scope(|ui| {
                // show_rows includes item spacing in its row stride. Code has no gaps.
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut area = egui::ScrollArea::both()
                    .id_salt(("source", self.selected))
                    .auto_shrink([false, false]);
                if let Some(index) = target {
                    area = area.vertical_scroll_offset(
                        (index as f32 * syntax::LINE_HEIGHT - ui.available_height() / 3.0).max(0.0),
                    );
                }
                area.show_rows(ui, syntax::LINE_HEIGHT, entries.len(), |ui, range| {
                    ui.set_min_width(column_width * 2.0);
                    for index in range {
                        let source_index = match entries[index] {
                            DisplayRow::Gap { start, end } => {
                                if let Some(all) = ui
                                    .push_id((self.selected, start, end), |ui| {
                                        context_gap(ui, start, end, column_width * 2.0)
                                    })
                                    .inner
                                {
                                    expansion = Some((start, end, all));
                                }
                                continue;
                            }
                            DisplayRow::Source(row) => row,
                        };
                        let row = &self.diff_rows[source_index];
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.push_id((self.selected, source_index, 0), |ui| {
                                code_cell(
                                    ui,
                                    row.left.as_ref(),
                                    column_width,
                                    0,
                                    (
                                        focus_ids[0],
                                        active_range
                                            .as_ref()
                                            .is_some_and(|range| range.contains(&source_index)),
                                    ),
                                    file,
                                    &mut self.source_selection,
                                );
                            });
                            ui.push_id((self.selected, source_index, 1), |ui| {
                                code_cell(
                                    ui,
                                    row.right.as_ref(),
                                    column_width,
                                    1,
                                    (
                                        focus_ids[1],
                                        active_range
                                            .as_ref()
                                            .is_some_and(|range| range.contains(&source_index)),
                                    ),
                                    file,
                                    &mut self.source_selection,
                                );
                            });
                        });
                    }
                })
            })
            .inner;
        if let Some((start, end, all)) = expansion {
            self.context_views[self.selected].expand(start, end, all);
            ui.ctx().request_repaint();
        }
        if self.source_selection.dragging
            && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
            && pointer.x >= scroll.inner_rect.left()
            && pointer.x <= scroll.inner_rect.right()
        {
            let edge = 18.0;
            let delta = if pointer.y < scroll.inner_rect.top() + edge {
                pointer.y - (scroll.inner_rect.top() + edge)
            } else if pointer.y > scroll.inner_rect.bottom() - edge {
                pointer.y - (scroll.inner_rect.bottom() - edge)
            } else {
                0.0
            };
            if delta != 0.0 {
                let mut state = scroll.state;
                let elapsed = ui.input(|input| input.stable_dt).min(0.05);
                state.offset.y = (state.offset.y + delta.clamp(-60.0, 60.0) * elapsed * 12.0)
                    .clamp(
                        0.0,
                        (scroll.content_size.y - scroll.inner_rect.height()).max(0.0),
                    );
                state.store(ui.ctx(), scroll.id);
                ui.ctx().request_repaint();
            }
        }
        if !ui.input(|input| input.pointer.primary_down()) {
            self.source_selection.dragging = false;
        }
    }

    fn plan(&mut self, ui: &mut egui::Ui) {
        if let Some(session) = &mut self.approval {
            ui.heading("Permitted impact");
            ui.label(RichText::new("The final deployment plan will be computed after building. Deployment stops if it exceeds this scope.").color(MUTED));
            ui.add_space(16.0);
            ui.label("VMs allowed to change, start, stop or restart");
            let mut output = egui::TextEdit::multiline(&mut session.allowed_vms)
                .desired_width(f32::INFINITY)
                .desired_rows(3)
                .show(ui);
            let text = session.allowed_vms.clone();
            selection::text_edit_menu(&mut output, &text, true);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label("System reboot");
                ui.radio_value(&mut session.allow_reboot, false, "Not permitted");
                ui.radio_value(&mut session.allow_reboot, true, "Permitted");
            });
            return;
        }
        egui::ScrollArea::vertical().id_salt("plan").auto_shrink([false, false]).show(ui, |ui| {
            ui.heading("Deployment effects");
            if self.review.mode == "demo" {
                ui.label(RichText::new("Example plan for this preview.").color(ACCENT));
            } else if self.review.deployment.is_some() {
                ui.label(RichText::new("Imported JSON · Machine state and candidate binding have not been verified.").color(MUTED));
            }
            ui.add_space(14.0);
            let Some(plan) = self.review.plan() else {
                reboot_badge(ui, None);
                ui.add_space(8.0);
                ui.label(RichText::new("No deployment plan loaded").strong());
                ui.label(RichText::new("Source changes alone do not establish which VMs will restart.").color(MUTED));
                return;
            };
            let effects = self.effects.as_ref().unwrap();
            if ui.available_width() >= 620.0 {
                ui.horizontal_top(|ui| {
                    let width = (ui.available_width() - 240.0).max(200.0);
                    ui.allocate_ui_with_layout(Vec2::new(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        plan_row(ui, "Restarting", |ui| vm_names(ui, effects.restarting.iter().map(String::as_str)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        reboot_badge(ui, effects.reboot);
                    });
                });
            } else {
                reboot_badge(ui, effects.reboot);
                ui.add_space(8.0);
                plan_row(ui, "Restarting", |ui| vm_names(ui, effects.restarting.iter().map(String::as_str)));
            }
            for (title, names) in [
                ("Starting", &effects.starting),
                ("Stopping", &effects.stopping),
            ] {
                if !names.is_empty() {
                    plan_row(ui, title, |ui| vm_names(ui, names.iter().map(String::as_str)));
                }
            }
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new("Configuration changes").strong().size(16.0));
            for (key, title) in [("changed", "Changed"), ("retire", "Retired"), ("restore", "Restored")] {
                plan_row(ui, title, |ui| {
                    if let Some(names) = plan.get(key).and_then(|value| value.as_array()) {
                        vm_names(ui, names.iter().filter_map(|name| name.as_str()));
                    } else {
                        ui.label(RichText::new("Not reported").size(13.0).color(MUTED));
                    }
                });
            }
            ui.add_space(8.0);
            ui.separator();
            egui::CollapsingHeader::new("Technical details").default_open(false).show(ui, |ui| {
                ui.label(RichText::new("Stop and start sequences, in the supplied order.").color(MUTED).size(12.0));
                if ui.available_width() >= 600.0 {
                    ui.columns(2, |columns| {
                        ordered_plan(&mut columns[0], plan, "stop", "Stop VMs", RED);
                        ordered_plan(&mut columns[1], plan, "start", "Start VMs", GREEN);
                    });
                } else {
                    ordered_plan(ui, plan, "stop", "Stop VMs", RED);
                    ordered_plan(ui, plan, "start", "Start VMs", GREEN);
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Complete supplied plan · JSON").strong());
                read_only_text(ui, "supplied-plan", &serde_json::to_string_pretty(self.review.deployment.as_ref().unwrap()).unwrap(), true);
            });
        });
    }

    fn record(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("review-record")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Review record");
                ui.add_space(12.0);
                if ui.available_width() >= 1200.0 {
                    ui.columns(2, |columns| {
                        record_source(&mut columns[0], &self.review, &mut self.copy_feedback);
                        record_identities(&mut columns[1], &self.review, &mut self.copy_feedback);
                    });
                } else {
                    record_source(ui, &self.review, &mut self.copy_feedback);
                    record_identities(ui, &self.review, &mut self.copy_feedback);
                }
            });
    }

    fn confirmation(&mut self, ctx: &egui::Context) {
        let Some(decision) = self.confirm else {
            return;
        };
        let mut finish = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("confirm-decision")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading(match decision {
                Decision::Approve => "Approve this local review?",
                Decision::Reject => "Reject this candidate?",
            });
            ui.label(&self.review.candidate.subject);
            ui.monospace(&self.review.candidate.commit);
            ui.add_space(10.0);
            ui.label(if self.review.mode == "demo" {
                "This example decision stays in memory. No machine is connected."
            } else if self.decisions_dir.is_some() {
                "A receipt will be saved locally. This does not authorize or start deployment."
            } else {
                "This decision stays in memory. Supply --decisions-dir to save local receipts."
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                cancel = ui.button("Back to review").clicked();
                finish = ui
                    .button(match decision {
                        Decision::Approve => "Confirm local approval",
                        Decision::Reject => "Confirm rejection",
                    })
                    .clicked();
            });
        });
        if cancel {
            self.confirm = None;
        }
        if finish {
            let result = self.review.receipt(decision).and_then(|receipt| {
                let path = if self.review.mode != "demo" {
                    self.decisions_dir
                        .as_ref()
                        .map(|directory| self.review.record_decision(decision, directory))
                        .transpose()?
                } else {
                    None
                };
                Ok((receipt, path))
            });
            match result {
                Ok((receipt, path)) => {
                    self.receipt = Some(receipt);
                    self.notice = Some(
                        path.map(|path| format!("Local decision saved: {}", path.display()))
                            .unwrap_or_else(|| {
                                "Local decision recorded in this window. No deployment started."
                                    .into()
                            }),
                    );
                    self.tab = Tab::Record;
                }
                Err(error) => self.notice = Some(format!("Decision was not recorded: {error:#}")),
            }
            self.confirm = None;
        }
    }
}

impl eframe::App for Controller {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.header(ctx);
        self.footer(ctx);
        self.sidebar(ctx);
        if self.tab == Tab::Source
            && self.confirm.is_none()
            && !self.about_open
            && !self.filter_has_focus
        {
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F7)) {
                self.navigate_section(false);
                ctx.request_repaint();
            } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F7))
            {
                self.navigate_section(true);
                ctx.request_repaint();
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(22.0))
            .show(ctx, |ui| {
                self.review_title(ui);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Source, "Source changes");
                    ui.selectable_value(&mut self.tab, Tab::Plan, "Deployment plan");
                    ui.selectable_value(&mut self.tab, Tab::Record, "Review record");
                    let show_copy_label = ui.available_width() >= 130.0;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let live_copy = self.approval.as_ref().map(|session| {
                            self.clipboard_review
                                .as_ref()
                                .map_err(Clone::clone)
                                .and_then(|base| {
                                    session.copy_text(base).map_err(|error| error.to_string())
                                })
                        });
                        let clipboard = live_copy.as_ref().unwrap_or(&self.clipboard_review);
                        let value = clipboard.as_ref().ok();
                        let response = copy_button(
                            ui,
                            "entire review",
                            value.map(String::as_str),
                            &mut self.copy_feedback,
                        );
                        if let Err(error) = clipboard {
                            response.on_disabled_hover_text(error);
                        }
                        if show_copy_label {
                            ui.label(RichText::new("Copy review").size(12.0).color(MUTED));
                        }
                    });
                });
                ui.add_space(12.0);
                match self.tab {
                    Tab::Source => self.source(ui),
                    Tab::Plan => self.plan(ui),
                    Tab::Record => self.record(ui),
                }
            });
        self.confirmation(ctx);
        self.about(ctx);
        self.frames += 1;
        if self.screenshot.is_some() && !self.screenshot_requested {
            if self.frames >= 4 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.screenshot_requested = true;
            }
            ctx.request_repaint();
        }
        for event in ctx.input(|input| input.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let bytes: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Err(error) = image::save_buffer(
                    &path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("airlock-ctrl: saving screenshot failed: {error}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

fn tree_row(
    ui: &mut egui::Ui,
    row: &TreeRow,
    files: &[FileChange],
    selected: usize,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 29.0), egui::Sense::hover());
    let response = ui
        .interact(
            rect,
            match row.kind {
                RowKind::Folder { .. } => egui::Id::new("ctrl-file-tree-folder").with(&row.path),
                RowKind::File { .. } => tree_id(&row.path),
            },
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let is_selected = matches!(row.kind, RowKind::File { index } if index == selected);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            is_selected,
            visible_path(&row.label),
        )
    });
    if is_selected {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 4.0, ACCENT_TINT);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 5.0), Vec2::new(2.0, 19.0)),
            1.0,
            ACCENT,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 1.0)),
            4.0,
            Color32::from_rgb(0x18, 0x26, 0x35),
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            4.0,
            Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );
    }
    let indent = row.depth as f32 * 14.0;
    let center = rect.left_center() + Vec2::new(10.0 + indent, 0.0);
    for depth in 0..row.depth {
        let x = rect.left() + 10.0 + depth as f32 * 14.0;
        ui.painter().line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1.0_f32, BORDER.linear_multiply(0.5)),
        );
    }
    let font = FontId::proportional(13.0);
    let text_left = center.x + 30.0;
    let reserved = match row.kind {
        RowKind::Folder { open, count } => {
            let offsets = if open {
                [
                    Vec2::new(-3.0, -2.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(3.0, -2.0),
                ]
            } else {
                [
                    Vec2::new(-1.0, -3.0),
                    Vec2::new(2.0, 0.0),
                    Vec2::new(-1.0, 3.0),
                ]
            };
            ui.painter().add(egui::Shape::line(
                offsets.into_iter().map(|offset| center + offset).collect(),
                Stroke::new(1.3_f32, MUTED),
            ));
            let origin = center + Vec2::new(10.0, -6.0);
            let outline = [
                (0.0, 1.0),
                (5.0, 1.0),
                (7.0, 3.0),
                (14.0, 3.0),
                (14.0, 12.0),
                (0.0, 12.0),
                (0.0, 1.0),
            ];
            ui.painter().add(egui::Shape::line(
                outline
                    .into_iter()
                    .map(|(x, y)| origin + Vec2::new(x, y))
                    .collect(),
                Stroke::new(1.1_f32, BRAND),
            ));
            ui.painter().text(
                rect.right_center() - Vec2::new(8.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                count.to_string(),
                FontId::proportional(11.0),
                MUTED,
            );
            34.0
        }
        RowKind::File { index } => {
            let file = &files[index];
            let origin = center + Vec2::new(12.0, -6.0);
            let outline = [
                (0.0, 0.0),
                (7.0, 0.0),
                (11.0, 4.0),
                (11.0, 13.0),
                (0.0, 13.0),
                (0.0, 0.0),
            ];
            ui.painter().add(egui::Shape::line(
                outline
                    .into_iter()
                    .map(|(x, y)| origin + Vec2::new(x, y))
                    .collect(),
                Stroke::new(1.0_f32, MUTED),
            ));
            ui.painter().add(egui::Shape::line(
                vec![
                    origin + Vec2::new(7.0, 0.0),
                    origin + Vec2::new(7.0, 4.0),
                    origin + Vec2::new(11.0, 4.0),
                ],
                Stroke::new(1.0_f32, MUTED),
            ));
            let (status, color) = match file.status.as_str() {
                "Added" => ("A", GREEN),
                "Deleted" => ("D", RED),
                _ => ("M", ACCENT),
            };
            let right = rect.right_center() - Vec2::new(8.0, 0.0);
            let font = FontId::monospace(11.0);
            if file.opaque.is_some() {
                ui.painter()
                    .text(right, egui::Align2::RIGHT_CENTER, "!", font, RED);
            } else if file.additions == 0 && file.deletions == 0 {
                ui.painter()
                    .text(right, egui::Align2::RIGHT_CENTER, "mode", font, MUTED);
            } else {
                if file.additions > 0 {
                    ui.painter().text(
                        right - Vec2::new(37.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        format!("+{}", compact_count(file.additions)),
                        font.clone(),
                        GREEN,
                    );
                }
                if file.deletions > 0 {
                    ui.painter().text(
                        right,
                        egui::Align2::RIGHT_CENTER,
                        format!("−{}", compact_count(file.deletions)),
                        font,
                        RED,
                    );
                }
            }
            ui.painter().text(
                right - Vec2::new(82.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                status,
                FontId::monospace(11.0),
                color,
            );
            108.0
        }
    };
    let mut job = egui::text::LayoutJob::simple(
        visible_path(&row.label),
        font,
        if is_selected { FOREGROUND } else { MUTED },
        (rect.right() - reserved - text_left).max(20.0),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(
        (rect.right() - reserved - text_left).max(20.0),
    );
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(text_left, rect.center().y - galley.size().y / 2.0),
        galley,
        FOREGROUND,
    );
    let tooltip = match row.kind {
        RowKind::Folder { count, .. } => {
            format!("{}\n{count} changed files", visible_path(&row.path))
        }
        RowKind::File { index } => {
            let file = &files[index];
            format!(
                "{}\n{} · +{} / −{}{}",
                visible_path(&row.path),
                file.status,
                file.additions,
                file.deletions,
                file.opaque
                    .as_ref()
                    .map(|reason| format!("\n{reason}"))
                    .unwrap_or_default()
            )
        }
    };
    response.context_menu(|ui| {
        if ui.button("Copy path").clicked() {
            ui.ctx().copy_text(row.path.clone());
            ui.close();
        }
        if let RowKind::File { index } = row.kind {
            if ui.button("Copy filename").clicked() {
                ui.ctx()
                    .copy_text(row.path.rsplit('/').next().unwrap_or(&row.path).to_owned());
                ui.close();
            }
            if files[index].opaque.is_none() {
                ui.separator();
                if ui.button("Copy baseline").clicked() {
                    ui.ctx().copy_text(files[index].before.clone());
                    ui.close();
                }
                if ui.button("Copy candidate").clicked() {
                    ui.ctx().copy_text(files[index].after.clone());
                    ui.close();
                }
            }
        }
    });
    response.on_hover_text(tooltip)
}

fn tree_id(path: &str) -> egui::Id {
    egui::Id::new("ctrl-file-tree-path").with(path)
}

fn compact_count(count: usize) -> String {
    if count >= 10_000 {
        format!("{}k", count / 1000)
    } else if count >= 1000 {
        format!("{:.1}k", count as f32 / 1000.0)
    } else {
        count.to_string()
    }
}

fn copy_button(
    ui: &mut egui::Ui,
    label: &'static str,
    value: Option<&str>,
    feedback: &mut Option<(&'static str, f64)>,
) -> egui::Response {
    let now = ui.input(|input| input.time);
    let remaining = feedback
        .filter(|(field, _)| *field == label)
        .map_or(0.0, |(_, time)| 1.6 - (now - time));
    let copied = remaining > 0.0;
    let tooltip = if copied {
        format!("{label} copied")
    } else {
        format!("Copy {label}")
    };
    let response = icons::button(
        ui,
        if copied { Icon::Check } else { Icon::Copy },
        &tooltip,
        value.is_some(),
    );
    if response.clicked() {
        ui.ctx().copy_text(value.unwrap().to_owned());
        *feedback = Some((label, now));
        ui.ctx().request_repaint();
    }
    if copied {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(remaining));
    }
    response
}

fn record_field(
    ui: &mut egui::Ui,
    label: &'static str,
    value: Option<&str>,
    placeholder: &str,
    feedback: &mut Option<(&'static str, f64)>,
) {
    ui.label(RichText::new(label).size(12.0).color(MUTED));
    let displayed = value.unwrap_or(placeholder);
    icons::row(ui, |ui| {
        let frame = egui::Frame::new()
            .fill(BACKGROUND)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 3));
        let width =
            (ui.available_width() - icons::BUTTON_SIZE - ui.spacing().item_spacing.x).max(40.0);
        frame
            .show(ui, |ui| {
                ui.set_width((width - frame.total_margin().sum().x).max(24.0));
                read_only_text(ui, label, displayed, false);
            })
            .response
            .on_hover_text(displayed);
        copy_button(ui, label, value, feedback);
    });
}

fn record_source(ui: &mut egui::Ui, review: &Review, feedback: &mut Option<(&'static str, f64)>) {
    plan_frame(ui, 210.0, |ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        ui.label(RichText::new("Source revisions").strong().size(16.0));
        ui.add_space(5.0);
        record_field(
            ui,
            "Repository",
            review.repository.as_deref(),
            "Built-in example",
            feedback,
        );
        ui.add_space(5.0);
        record_field(
            ui,
            "Baseline commit",
            Some(&review.baseline.commit),
            "",
            feedback,
        );
        ui.add_space(5.0);
        record_field(
            ui,
            "Candidate commit",
            Some(&review.candidate.commit),
            "",
            feedback,
        );
    });
}

fn record_identities(
    ui: &mut egui::Ui,
    review: &Review,
    feedback: &mut Option<(&'static str, f64)>,
) {
    plan_frame(ui, 210.0, |ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        ui.label(RichText::new("Content identities").strong().size(16.0));
        ui.label(
            RichText::new("Binds source changes and the supplied plan.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(5.0);
        record_field(
            ui,
            "Review SHA-256",
            Some(&review.fingerprint),
            "",
            feedback,
        );
        ui.add_space(5.0);
        record_field(
            ui,
            "Candidate lock SHA-256",
            review.lock_hash.as_deref(),
            "No flake.lock",
            feedback,
        );
    });
}

fn reboot_state(required: Option<bool>) -> (&'static str, Color32) {
    match required {
        Some(true) => ("Required", ACCENT),
        Some(false) => ("Not required", GREEN),
        None => ("Unknown", MUTED),
    }
}

fn effects_summary(ui: &mut egui::Ui, effects: Option<&PlannedEffects>) {
    ui.spacing_mut().item_spacing.y = 5.0;
    let row = |ui: &mut egui::Ui, label: &str, names: &str| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{label}:")).size(12.0).color(MUTED));
            ui.add(egui::Label::new(RichText::new(names).monospace().size(13.0)).truncate())
                .on_hover_text(names)
                .context_menu(|ui| {
                    if ui.button("Copy VM names").clicked() {
                        ui.ctx().copy_text(names.to_owned());
                        ui.close();
                    }
                });
        });
    };
    row(
        ui,
        "VMs restarting",
        &effects.map_or_else(
            || "Unknown (no plan)".to_owned(),
            |effects| names_text(&effects.restarting),
        ),
    );
    if let Some(effects) = effects {
        for (label, names) in [
            ("VMs starting", &effects.starting),
            ("VMs stopping", &effects.stopping),
        ] {
            if !names.is_empty() {
                row(ui, label, &names_text(names));
            }
        }
    }
    let (status, color) = reboot_state(effects.and_then(|effects| effects.reboot));
    ui.label(
        RichText::new(format!("System reboot: {status}"))
            .strong()
            .size(12.0)
            .color(color),
    )
    .context_menu(|ui| {
        if ui.button("Copy effects summary").clicked() {
            ui.ctx().copy_text(effects::summary(effects));
            ui.close();
        }
    });
}

fn plan_row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(120.0, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(120.0);
                ui.label(RichText::new(label).size(13.0).color(MUTED));
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 0.0),
            egui::Layout::top_down(egui::Align::Min),
            content,
        );
    });
}

fn reboot_badge(ui: &mut egui::Ui, required: Option<bool>) {
    let (status, color) = reboot_state(required);
    egui::Frame::new()
        .fill(color.linear_multiply(0.08))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("System reboot: {status}"))
                    .size(12.0)
                    .color(color),
            );
        });
}

fn vm_names<'a>(ui: &mut egui::Ui, names: impl IntoIterator<Item = &'a str>) {
    let mut names = names.into_iter().peekable();
    if names.peek().is_none() {
        ui.label(RichText::new("None").size(13.0).color(MUTED));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
        while let Some(name) = names.next() {
            let display = format!(
                "{}{}",
                visible_path(name),
                if names.peek().is_some() { "," } else { "" }
            );
            ui.add(
                egui::Label::new(RichText::new(display).monospace().size(13.0))
                    .selectable(true)
                    .wrap(),
            )
            .context_menu(|ui| {
                if ui.button("Copy VM name").clicked() {
                    ui.ctx().copy_text(name.to_owned());
                    ui.close();
                }
            });
        }
    });
}

fn plan_frame(ui: &mut egui::Ui, minimum_height: f32, content: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let frame = egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(14.0)
        .corner_radius(6.0);
    frame.show(ui, |ui| {
        ui.set_width((width - frame.total_margin().sum().x).max(0.0));
        ui.set_min_height(minimum_height);
        content(ui);
    });
}

fn ordered_plan(
    ui: &mut egui::Ui,
    plan: &serde_json::Value,
    key: &str,
    title: &str,
    color: Color32,
) {
    let names = plan.get(key).and_then(|value| value.as_array());
    ui.label(RichText::new(title).strong().color(color));
    let Some(names) = names else {
        ui.label(RichText::new("Not reported").color(MUTED));
        return;
    };
    if names.is_empty() {
        ui.label(RichText::new("None").color(MUTED));
    } else {
        for (index, name) in names.iter().enumerate() {
            let name = name.as_str().unwrap_or("[unexpected entry]");
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{:02}", index + 1))
                        .monospace()
                        .size(12.0)
                        .color(MUTED),
                );
                copyable_label(ui, RichText::new(visible_path(name)).monospace(), name);
            });
        }
    }
}

fn column_header(
    ui: &mut egui::Ui,
    label: &str,
    color: Color32,
    file: &FileChange,
    side: usize,
    selection: &SourceSelection,
) {
    let source = if side == 0 { &file.before } else { &file.after };
    icons::row(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            RichText::new(label.to_uppercase())
                .color(color)
                .size(11.0)
                .strong(),
        );
        let (response, _) =
            egui::containers::menu::MenuButton::from_button(icons::control()).ui(ui, |ui| {
                let selected = selection.text(source, side);
                if ui
                    .add_enabled(selected.is_some(), egui::Button::new("Copy selection"))
                    .clicked()
                {
                    ui.ctx().copy_text(selected.unwrap().to_owned());
                    ui.close();
                }
                if ui.button("Copy file contents").clicked() {
                    ui.ctx().copy_text(source.clone());
                    ui.close();
                }
                ui.separator();
                if ui.button("Copy file path").clicked() {
                    ui.ctx().copy_text(file.path.clone());
                    ui.close();
                }
            });
        icons::decorate(ui, response, Icon::More, &format!("{label} file actions"));
    });
}

fn context_gap(ui: &mut egui::Ui, start: usize, end: usize, width: f32) -> Option<bool> {
    let mut expand = None;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(width, syntax::LINE_HEIGHT), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, PANEL);
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0_f32, BORDER));
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0_f32, BORDER));
    // Keep controls inside the visible viewport even when long code scrolls horizontally.
    let visible = rect.intersect(ui.clip_rect());
    ui.scope_builder(egui::UiBuilder::new().max_rect(visible), |ui| {
        ui.spacing_mut().interact_size.y = syntax::LINE_HEIGHT;
        ui.spacing_mut().button_padding.y = 0.0;
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!(
                            "{} unchanged {}",
                            end - start,
                            if end - start == 1 { "line" } else { "lines" }
                        ))
                        .color(MUTED)
                        .size(12.0),
                    )
                    .frame(false),
                )
                .on_hover_text("Reveal up to 20 more lines next to each surrounding change")
                .clicked()
            {
                expand = Some(false);
            }
            let response = ui.add(icons::control().min_size(Vec2::splat(syntax::LINE_HEIGHT)));
            if icons::decorate(
                ui,
                response,
                Icon::ExpandAll,
                "Show this entire unchanged section",
            )
            .clicked()
            {
                expand = Some(true);
            }
        });
    });
    expand
}

fn code_cell(
    ui: &mut egui::Ui,
    line: Option<&CodeLine>,
    width: f32,
    side: usize,
    focus: (egui::Id, bool),
    file: &FileChange,
    selection: &mut SourceSelection,
) {
    let (focus_id, active) = focus;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, syntax::LINE_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    let background = match line.map(|line| line.kind) {
        Some(ChangeTag::Delete) => Color32::from_rgb(58, 31, 40),
        Some(ChangeTag::Insert) => Color32::from_rgb(23, 49, 40),
        _ => BACKGROUND,
    };
    ui.painter().rect_filled(rect, 0.0, background);
    if active {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
            0.0,
            ACCENT,
        );
    }
    if side == 1 {
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            Stroke::new(1.0_f32, BORDER),
        );
    }
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, BORDER),
    );
    let Some(line) = line else {
        return;
    };
    ui.painter().text(
        rect.left_center() + Vec2::new(34.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        line.number.to_string(),
        FontId::monospace(12.0),
        MUTED,
    );
    let (marker, marker_color) = match line.kind {
        ChangeTag::Delete => ("−", RED),
        ChangeTag::Insert => ("+", GREEN),
        ChangeTag::Equal => ("", MUTED),
    };
    ui.painter().text(
        rect.left_center() + Vec2::new(46.0, 0.0),
        egui::Align2::CENTER_CENTER,
        marker,
        FontId::monospace(12.0),
        marker_color,
    );
    let mut galley = ui.painter().layout_job(line.job.clone());
    let origin = rect.left_top() + Vec2::new(syntax::TEXT_LEFT, 0.0);
    let source = if side == 0 { &file.before } else { &file.after };
    let raw_line = source[line.source_start..].split('\n').next().unwrap_or("");
    let pointer = ui.input(|input| input.pointer.interact_pos());
    let pointer_byte = pointer.map(|position| {
        line.source_start
            + selection::source_byte(raw_line, galley.cursor_from_pos(position - origin).index)
    });
    let at_vertical_edge = pointer.is_some_and(|position| {
        position.x >= rect.left()
            && position.x <= rect.right()
            && ((position.y < ui.clip_rect().top()
                && rect.contains(egui::pos2(position.x, ui.clip_rect().top())))
                || (position.y > ui.clip_rect().bottom()
                    && rect.contains(egui::pos2(position.x, ui.clip_rect().bottom()))))
    });
    if response.contains_pointer() || selection.dragging && at_vertical_edge {
        if response.contains_pointer() && ui.input(|input| input.pointer.primary_pressed()) {
            let byte = pointer_byte.unwrap_or(line.source_start);
            if !ui.input(|input| input.modifiers.shift) || selection.side != side {
                selection.anchor = byte;
            }
            selection.side = side;
            selection.cursor = byte;
            selection.dragging = true;
            selection.focus = Some(focus_id);
            ui.memory_mut(|memory| memory.request_focus(focus_id));
        }
        if selection.dragging && selection.side == side {
            selection.cursor = pointer_byte.unwrap_or(selection.cursor);
            ui.ctx().request_repaint();
        }
    }
    if response.triple_clicked() {
        selection.anchor = line.source_start;
        selection.cursor = (line.source_start + raw_line.len() + 1).min(source.len());
    } else if response.double_clicked() {
        let range = selection::word_range(source, pointer_byte.unwrap_or(line.source_start));
        selection.anchor = range.start;
        selection.cursor = range.end;
    }
    response.clone().on_hover_cursor(egui::CursorIcon::Text);
    response.context_menu(|ui| {
        let selected = selection.text(source, side);
        if ui
            .add_enabled(selected.is_some(), egui::Button::new("Copy selection"))
            .clicked()
        {
            ui.ctx().copy_text(selected.unwrap().to_owned());
            ui.close();
        }
        if ui.button("Copy line").clicked() {
            ui.ctx().copy_text(raw_line.to_owned());
            ui.close();
        }
        if ui
            .button(if side == 0 {
                "Copy baseline"
            } else {
                "Copy candidate"
            })
            .clicked()
        {
            ui.ctx().copy_text(source.clone());
            ui.close();
        }
        if ui.button("Select all").clicked() {
            selection.select_all(source, side, focus_id);
            ui.memory_mut(|memory| memory.request_focus(focus_id));
            ui.close();
        }
        ui.separator();
        if ui.button("Copy file path").clicked() {
            ui.ctx().copy_text(file.path.clone());
            ui.close();
        }
    });
    if selection.side == side {
        let range = selection.range();
        let start = range
            .start
            .saturating_sub(line.source_start)
            .min(raw_line.len());
        let end = range
            .end
            .saturating_sub(line.source_start)
            .min(raw_line.len());
        if start < end {
            egui::text_selection::visuals::paint_text_selection(
                &mut galley,
                ui.visuals(),
                &egui::text::CCursorRange::two(
                    egui::text::CCursor::new(selection::displayed_cursor(raw_line, start)),
                    egui::text::CCursor::new(selection::displayed_cursor(raw_line, end)),
                ),
                None,
            );
        }
        if range.start <= line.source_start + raw_line.len()
            && range.end > line.source_start + raw_line.len()
        {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    origin + Vec2::new(galley.size().x, 0.0),
                    Vec2::new(7.9, galley.size().y.max(13.0)),
                ),
                0.0,
                ui.visuals().selection.bg_fill,
            );
        }
    }
    ui.painter().galley(origin, galley, Color32::WHITE);
}

fn visible_path(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character.is_control() || character == '\\' {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}
