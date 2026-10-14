use crate::{
    Decision,
    model::{FileChange, Review},
    theme::{
        self, ACCENT, ACCENT_TINT, BACKGROUND, BORDER, BRAND, BUTTON_TEXT, FOREGROUND, GREEN,
        MUTED, PANEL, RED,
    },
};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use similar::{ChangeTag, TextDiff};
use std::path::PathBuf;

struct CodeLine {
    number: usize,
    segments: Vec<(bool, String)>,
    kind: ChangeTag,
}

struct DiffRow {
    left: Option<CodeLine>,
    right: Option<CodeLine>,
}

fn rows(file: &FileChange) -> Vec<DiffRow> {
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
    diff_rows: Vec<DiffRow>,
    reviewed: bool,
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
            diff_rows,
            reviewed: false,
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
                    ui.checkbox(
                        &mut self.reviewed,
                        "I reviewed the source and deployment plan",
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let enabled = self.receipt.is_none()
                            && self.reviewed
                            && self.review.approval_blocker().is_none();
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
            .default_width(280.0)
            .min_width(210.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(18.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("CANDIDATE").size(11.0).color(MUTED).strong());
                ui.label(RichText::new(&self.review.candidate.subject).strong());
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Baseline to candidate")
                        .size(12.0)
                        .color(MUTED),
                );
                ui.monospace(format!(
                    "{} -> {}",
                    short(&self.review.baseline.commit),
                    short(&self.review.candidate.commit)
                ));
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Changed files").strong());
                    ui.label(RichText::new(self.review.files.len().to_string()).color(MUTED));
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("Filter files…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("file-list")
                    .show(ui, |ui| {
                        for (index, file) in self.review.files.iter().enumerate() {
                            if !file
                                .path
                                .to_lowercase()
                                .contains(&self.filter.to_lowercase())
                            {
                                continue;
                            }
                            let text = format!(
                                "{}  {}\n     +{}  −{}",
                                match file.status.as_str() {
                                    "Added" => "A",
                                    "Deleted" => "D",
                                    _ => "M",
                                },
                                visible_path(&file.path),
                                file.additions,
                                file.deletions
                            );
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(text).size(12.0))
                                        .selected(index == self.selected)
                                        .min_size(Vec2::new(ui.available_width(), 50.0)),
                                )
                                .clicked()
                            {
                                self.selected = index;
                                self.diff_rows = rows(file);
                                self.tab = Tab::Source;
                            }
                            ui.add_space(3.0);
                        }
                    });
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
            ui.label(RichText::new(visible_path(&file.path)).strong().monospace());
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
                RichText::new("Read-only comparison · Includes unchanged context")
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
        egui::ScrollArea::both()
            .id_salt(("source", self.selected))
            .auto_shrink([false, false])
            .show_rows(ui, 23.0, self.diff_rows.len(), |ui, range| {
                ui.set_min_width(column_width * 2.0);
                for index in range {
                    let row = &self.diff_rows[index];
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        code_cell(ui, row.left.as_ref(), column_width);
                        code_cell(ui, row.right.as_ref(), column_width);
                    });
                }
            });
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
                ui.label(RichText::new(names.filter(|names| !names.is_empty()).unwrap_or_else(|| if plan.get(key).is_some() { "None" } else { "Not reported" }.into())).color(MUTED));
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
                ui.add(egui::Label::new(RichText::new(serde_json::to_string_pretty(self.review.deployment.as_ref().unwrap()).unwrap()).monospace().size(12.0)).selectable(true));
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
                    ui.add(egui::Label::new(RichText::new(value).monospace()).selectable(true));
                    ui.add_space(6.0);
                }
                ui.separator();
                if let Some(receipt) = &self.receipt {
                    ui.label(RichText::new("Local decision").color(ACCENT).strong());
                    ui.add(
                        egui::Label::new(
                            RichText::new(serde_json::to_string_pretty(receipt).unwrap())
                                .monospace()
                                .size(12.0),
                        )
                        .selectable(true),
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

fn code_cell(ui: &mut egui::Ui, line: Option<&CodeLine>, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 23.0), egui::Sense::hover());
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
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        rect.left_top() + Vec2::new(52.0, 4.0),
        galley,
        Color32::WHITE,
    );
}

fn short(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}

fn visible_path(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}
