use crate::{
    Decision,
    file_tree::{FileTree, RowKind, TreeRow},
    model::{FileChange, Review},
    selection::{self, SourceSelection, copyable_label, read_only_text},
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
    segments: Vec<(bool, String)>,
    kind: ChangeTag,
}

struct DiffRow {
    left: Option<CodeLine>,
    right: Option<CodeLine>,
}

fn rows(file: &FileChange) -> Vec<DiffRow> {
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
    let diff = TextDiff::configure()
        .timeout(std::time::Duration::from_millis(250))
        .diff_lines(&file.before, &file.after);
    let mut result = Vec::new();
    for op in diff.ops() {
        let mut deleted = Vec::new();
        let mut inserted = Vec::new();
        for change in diff.iter_inline_changes(op) {
            let line = CodeLine {
                number: change.old_index().or(change.new_index()).unwrap_or(0) + 1,
                source_start: if let Some(index) = change.old_index() {
                    before_starts[index]
                } else {
                    after_starts[change.new_index().unwrap()]
                },
                segments: change
                    .iter_strings_lossy()
                    .map(|(emphasized, value)| {
                        (
                            emphasized,
                            value.trim_end_matches('\n').replace('\t', "    "),
                        )
                    })
                    .collect(),
                kind: change.tag(),
            };
            match change.tag() {
                ChangeTag::Equal => {
                    let right = CodeLine {
                        number: change.new_index().unwrap_or(0) + 1,
                        source_start: after_starts[change.new_index().unwrap()],
                        segments: line.segments.clone(),
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
    selected: usize,
    tab: Tab,
    filter: String,
    file_tree: FileTree,
    collapsed_folders: BTreeSet<String>,
    scroll_to_selection: bool,
    diff_rows: Vec<DiffRow>,
    source_selection: SourceSelection,
    confirm: Option<Decision>,
    receipt: Option<serde_json::Value>,
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
    ) -> Self {
        theme::apply(&cc.egui_ctx);
        let diff_rows = review.files.first().map(rows).unwrap_or_default();
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
            selected: 0,
            tab: Tab::Source,
            filter: String::new(),
            file_tree,
            collapsed_folders: BTreeSet::new(),
            scroll_to_selection: false,
            diff_rows,
            source_selection: SourceSelection::default(),
            confirm: None,
            receipt: None,
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
                                "Review source. Understand the effects. Make the decision.",
                            )
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.review.mode == "demo" {
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
                if let Some(reason) = self.review.approval_blocker() {
                    ui.label(RichText::new(reason).color(MUTED));
                }
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let enabled =
                            self.receipt.is_none() && self.review.approval_blocker().is_none();
                        let label = if self.review.mode == "demo" {
                            "Approve preview"
                        } else {
                            "Approve local review"
                        };
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new(RichText::new(label).color(BUTTON_TEXT))
                                    .fill(ACCENT),
                            )
                            .clicked()
                        {
                            self.confirm = Some(Decision::Approve);
                        }
                        if ui
                            .add_enabled(self.receipt.is_none(), egui::Button::new("Reject"))
                            .clicked()
                        {
                            self.confirm = Some(Decision::Reject);
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "No deployment connection · Decisions are local review records only",
                    )
                    .size(12.0)
                    .color(MUTED),
                );
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
                ui.label(RichText::new("CANDIDATE").size(11.0).color(MUTED).strong());
                ui.add(
                    egui::Label::new(RichText::new(&self.review.candidate.subject).strong())
                        .truncate(),
                )
                .on_hover_text(&self.review.candidate.subject);
                ui.add(egui::Label::new(
                    RichText::new(format!(
                        "{} -> {}",
                        short(&self.review.baseline.commit),
                        short(&self.review.candidate.commit)
                    ))
                    .monospace()
                    .size(11.0)
                    .color(MUTED),
                ))
                .on_hover_text(format!(
                    "Baseline: {}\nCandidate: {}",
                    self.review.baseline.commit, self.review.candidate.commit,
                ));
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
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
                ui.horizontal(|ui| {
                    // Reserve the full button width, including its themed padding.
                    let clear_width = 40.0;
                    let margin = egui::Margin::symmetric(4, 2);
                    let text_width = (ui.available_width() - clear_width - ui.spacing().item_spacing.x - margin.sum().x).max(40.0);
                    let filter_id = ui.make_persistent_id("file-filter");
                    let previous_cursor = egui::TextEdit::load_state(ui.ctx(), filter_id)
                        .and_then(|state| state.cursor.char_range());
                    let mut filter = egui::TextEdit::singleline(&mut self.filter)
                            .id(filter_id)
                            .hint_text("Find a file or folder…")
                            .margin(margin)
                            .desired_width(text_width)
                            .show(ui);
                    if filter.response.contains_pointer() && ui.input(|input| input.pointer.secondary_pressed()) {
                        filter.state.cursor.set_char_range(previous_cursor);
                        filter.state.clone().store(ui.ctx(), filter_id);
                    }
                    selection::text_edit_menu(&mut filter, &self.filter, true);
                    if ui.add_enabled(!self.filter.is_empty(), egui::Button::new("×").small().min_size(Vec2::new(clear_width, 20.0)))
                            .on_hover_text("Clear file filter")
                            .clicked()
                    {
                        self.filter.clear();
                    }
                });
                ui.horizontal(|ui| {
                    let searching = !self.filter.trim().is_empty();
                    if ui
                        .add_enabled(
                            !searching,
                            egui::Button::new(RichText::new("Collapse all").size(11.0)).small(),
                        )
                        .clicked()
                    {
                        self.collapsed_folders = all_rows
                            .iter()
                            .filter_map(|row| match row.kind {
                                RowKind::Folder { .. } => Some(row.path.clone()),
                                _ => None,
                            })
                            .collect();
                    }
                    if ui
                        .add_enabled(
                            !searching,
                            egui::Button::new(RichText::new("Expand all").size(11.0)).small(),
                        )
                        .clicked()
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
                let tree_height = (ui.available_height() - 46.0).max(100.0);
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
                ui.horizontal(|ui| {
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
                        if ui
                            .add_enabled(
                                !matching.is_empty()
                                    && position
                                        .is_none_or(|position| position + 1 < matching.len()),
                                egui::Button::new("Next").small(),
                            )
                            .clicked()
                        {
                            navigate = Some(true);
                        }
                        if ui
                            .add_enabled(
                                !matching.is_empty()
                                    && position.is_none_or(|position| position > 0),
                                egui::Button::new("Previous").small(),
                            )
                            .clicked()
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
                    self.selected = index;
                    self.source_selection = SourceSelection::default();
                    self.diff_rows = rows(&self.review.files[index]);
                    self.tab = Tab::Source;
                    if navigate.is_some() {
                        self.scroll_to_selection = true;
                        ctx.memory_mut(|memory| memory.request_focus(tree_id(&self.review.files[index].path)));
                    }
                }
            });
    }

    fn source(&mut self, ui: &mut egui::Ui) {
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
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Drag to select · Right-click to copy · Ctrl+C")
                    .size(12.0)
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Copy candidate").clicked() {
                    ui.ctx().copy_text(file.after.clone());
                }
                if ui.small_button("Copy baseline").clicked() {
                    ui.ctx().copy_text(file.before.clone());
                }
            });
        });
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
        ui.columns(2, |columns| {
            columns[0].label(RichText::new("BASELINE").color(MUTED).size(11.0).strong());
            columns[1].label(RichText::new("CANDIDATE").color(ACCENT).size(11.0).strong());
        });
        let available = ui.available_width();
        let max_chars = self
            .diff_rows
            .iter()
            .flat_map(|row| [&row.left, &row.right])
            .filter_map(|line| line.as_ref())
            .map(|line| {
                line.segments
                    .iter()
                    .map(|(_, text)| text.chars().count())
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        let column_width = (available / 2.0).max(max_chars as f32 * 7.9 + 75.0);
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
        let scroll = egui::ScrollArea::both()
            .id_salt(("source", self.selected))
            .auto_shrink([false, false])
            .show_rows(ui, 23.0, self.diff_rows.len(), |ui, range| {
                ui.set_min_width(column_width * 2.0);
                for index in range {
                    let row = &self.diff_rows[index];
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.push_id((self.selected, index, 0), |ui| {
                            code_cell(
                                ui,
                                row.left.as_ref(),
                                column_width,
                                0,
                                focus_ids[0],
                                file,
                                &mut self.source_selection,
                            );
                        });
                        ui.push_id((self.selected, index, 1), |ui| {
                            code_cell(
                                ui,
                                row.right.as_ref(),
                                column_width,
                                1,
                                focus_ids[1],
                                file,
                                &mut self.source_selection,
                            );
                        });
                    });
                }
            });
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

    fn plan(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("plan").show(ui, |ui| {
            ui.heading("Deployment effects");
            if self.review.mode == "demo" {
                ui.label(RichText::new("Example plan for this preview.").color(ACCENT));
            } else {
                ui.label(RichText::new("Imported JSON · Machine state and candidate binding have not been verified.").color(MUTED));
            }
            ui.add_space(14.0);
            let Some(plan) = self.review.plan() else {
                ui.label("No deployment plan loaded. Source changes alone do not establish which VMs will restart.");
                return;
            };
            for (key, label) in [("changed", "Changed VMs"), ("stop", "VMs stopped, in order"), ("start", "VMs started, in order"), ("retire", "Retired VMs"), ("restore", "Restored VMs")] {
                ui.label(RichText::new(label).strong());
                let names = plan.get(key).and_then(|value| value.as_array()).map(|items| items.iter().map(|value| value.as_str().unwrap_or("[unexpected entry]")).collect::<Vec<_>>().join(", "));
                let names = names.filter(|names| !names.is_empty()).unwrap_or_else(|| if plan.get(key).is_some() { "None" } else { "Not reported" }.into());
                copyable_label(ui, RichText::new(&names).color(MUTED), &names);
                ui.add_space(6.0);
            }
            ui.separator();
            let reboot = match plan.get("host_reboot_required").and_then(|value| value.as_bool()) {
                Some(true) => "Host changed · Reboot required for host activation",
                Some(false) => "No host reboot required according to the supplied plan",
                None => "Host reboot requirement is not reported in this plan",
            };
            ui.label(RichText::new(reboot).color(ACCENT));
            ui.add_space(12.0);
            egui::CollapsingHeader::new("Complete supplied plan").default_open(true).show(ui, |ui| {
                read_only_text(ui, "supplied-plan", &serde_json::to_string_pretty(self.review.deployment.as_ref().unwrap()).unwrap(), true);
            });
        });
    }

    fn record(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .id_salt("review-record")
            .show(ui, |ui| {
                ui.heading("Review record");
                ui.label(
                    RichText::new("This fingerprint binds the displayed source and supplied plan.")
                        .color(MUTED),
                );
                ui.add_space(12.0);
                for (label, value) in [
                    ("Baseline commit", self.review.baseline.commit.as_str()),
                    ("Candidate commit", self.review.candidate.commit.as_str()),
                    (
                        "Candidate lock SHA-256",
                        self.review.lock_hash.as_deref().unwrap_or("No flake.lock"),
                    ),
                    ("Review SHA-256", self.review.fingerprint.as_str()),
                    (
                        "Repository",
                        self.review
                            .repository
                            .as_deref()
                            .unwrap_or("Built-in example"),
                    ),
                ] {
                    ui.label(RichText::new(label).strong());
                    read_only_text(ui, label, value, false);
                    ui.add_space(6.0);
                }
                ui.separator();
                if let Some(receipt) = &self.receipt {
                    ui.label(RichText::new("Local decision").color(ACCENT).strong());
                    read_only_text(
                        ui,
                        "local-decision",
                        &serde_json::to_string_pretty(receipt).unwrap(),
                        true,
                    );
                } else {
                    ui.label("Awaiting your review decision.");
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
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(22.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Source, "Source changes");
                    ui.selectable_value(&mut self.tab, Tab::Plan, "Deployment plan");
                    ui.selectable_value(&mut self.tab, Tab::Record, "Review record");
                });
                ui.add_space(12.0);
                match self.tab {
                    Tab::Source => self.source(ui),
                    Tab::Plan => self.plan(ui),
                    Tab::Record => self.record(ui),
                }
            });
        self.confirmation(ctx);
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

fn code_cell(
    ui: &mut egui::Ui,
    line: Option<&CodeLine>,
    width: f32,
    side: usize,
    focus_id: egui::Id,
    file: &FileChange,
    selection: &mut SourceSelection,
) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, 23.0), egui::Sense::click_and_drag());
    let background = match line.map(|line| line.kind) {
        Some(ChangeTag::Delete) => Color32::from_rgb(58, 31, 40),
        Some(ChangeTag::Insert) => Color32::from_rgb(23, 49, 40),
        _ => BACKGROUND,
    };
    ui.painter().rect_filled(rect, 0.0, background);
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, BORDER),
    );
    let Some(line) = line else {
        return;
    };
    ui.painter().text(
        rect.left_top() + Vec2::new(36.0, 4.0),
        egui::Align2::RIGHT_TOP,
        line.number.to_string(),
        FontId::monospace(12.0),
        MUTED,
    );
    let mut job = egui::text::LayoutJob::default();
    for (emphasized, text) in &line.segments {
        let color = match line.kind {
            ChangeTag::Delete => RED,
            ChangeTag::Insert => GREEN,
            ChangeTag::Equal => FOREGROUND,
        };
        let background = if *emphasized {
            match line.kind {
                ChangeTag::Delete => Color32::from_rgb(100, 43, 53),
                ChangeTag::Insert => Color32::from_rgb(35, 87, 62),
                _ => Color32::TRANSPARENT,
            }
        } else {
            Color32::TRANSPARENT
        };
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: FontId::monospace(13.0),
                color,
                background,
                ..Default::default()
            },
        );
    }
    let mut galley = ui.painter().layout_job(job);
    let origin = rect.left_top() + Vec2::new(52.0, 4.0);
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

fn short(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
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
