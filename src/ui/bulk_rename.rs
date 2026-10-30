// src/ui/bulk_rename.rs
// Toplu yeniden adlandırma — pattern + canlı önizleme + çakışma koruması
use egui::*;
use chrono::Local;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use crate::backend::fs_ops::FileEntry;

#[derive(Clone)]
pub struct BulkRenameState {
    pub pattern:   String,
    pub pad:       u32,   // {n} sıfır doldurma uzunluğu
    pub start_n:   u32,   // {n} başlangıç sayısı
    pub preview:   Vec<(String, String)>, // (eski ad, yeni ad)
    pub has_conflict: bool,
}

impl BulkRenameState {
    pub fn new(entries: &[FileEntry]) -> Self {
        let mut s = Self {
            pattern: "{name}{ext}".to_string(),
            pad: 0, start_n: 1,
            preview: Vec::new(), has_conflict: false,
        };
        s.update_preview(entries);
        s
    }

    pub fn update_preview(&mut self, entries: &[FileEntry]) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let mut names: Vec<String> = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            let n = self.start_n + i as u32;
            let n_str = if self.pad == 0 {
                n.to_string()
            } else {
                format!("{:0>width$}", n, width = self.pad as usize)
            };

            let stem = std::path::Path::new(&entry.name)
                .file_stem().map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext = std::path::Path::new(&entry.name)
                .extension().map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();

            let new_name = self.pattern
                .replace("{name}",  &stem)
                .replace("{ext}",   &ext)
                .replace("{n}",     &n_str)
                .replace("{date}",  &today)
                .replace("{upper}", &stem.to_uppercase())
                .replace("{lower}", &stem.to_lowercase());

            names.push(new_name.clone());
            self.preview.push((entry.name.clone(), new_name));
        }

        // Çakışma kontrolü
        let mut seen = std::collections::HashSet::new();
        self.has_conflict = names.iter().any(|n| !seen.insert(n.clone()));
    }
}

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let entries = state.selected_entries();
    if entries.len() < 2 { state.panels.close(); return; }

    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("bulk_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0,0,0,140));
            if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
        });

    let pw = 680.0_f32; let ph = 520.0_f32;
    let state_id = Id::new("bulk_rename_state");
    let mut br: BulkRenameState = ctx.data_mut(|d|
        d.get_temp(state_id).unwrap_or_else(|| BulkRenameState::new(&entries))
    );
    let prev_pattern = br.pattern.clone();
    let prev_pad     = br.pad;
    let prev_start   = br.start_n;

    egui::Area::new(Id::new("bulk_rename_dialog"))
        .fixed_pos(Pos2::new(screen.center().x - pw/2.0, screen.center().y - ph/2.0))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow { offset: Vec2::new(0.0,8.0), blur: 24.0,
                    spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw - sp.lg * 2.0);
                ui.set_max_height(ph);

                // Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Bulk Rename")
                        .color(c.accent()).size(t.typography.size_lg).strong());
                    ui.label(RichText::new(format!("— {} files", entries.len()))
                        .color(c.text_hint()).size(t.typography.size_sm));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").clicked() { state.panels.close(); }
                    });
                });

                ui.add_space(sp.md);

                // ── Hazır şablonlar ──────────────────────────────────────
                ui.label(RichText::new("Quick Templates").color(c.text_hint())
                    .size(t.typography.size_xs).strong());
                ui.add_space(sp.xs);
                ui.horizontal_wrapped(|ui| {
                    for (label, pattern) in TEMPLATES {
                        if ui.add(Button::new(
                            RichText::new(*label).color(c.text_secondary()).size(t.typography.size_xs)
                        ).fill(c.bg_surface0()).rounding(Rounding::same(s.radius_sm))
                         .stroke(Stroke::new(1.0_f32, c.border()))).clicked() {
                            br.pattern = pattern.to_string();
                        }
                    }
                });

                ui.add_space(sp.sm);

                // ── Pattern input ────────────────────────────────────────
                ui.label(RichText::new("Pattern").color(c.text_hint()).size(t.typography.size_xs).strong());
                ui.add_space(sp.xs);
                ui.add(egui::TextEdit::singleline(&mut br.pattern)
                    .desired_width(f32::INFINITY)
                    .font(FontId::new(t.typography.size_base, FontFamily::Monospace))
                    .hint_text("{name}{ext}  or  {date}_{n}{ext}  or  foto_{n}{ext}")
                    .text_color(c.text_primary())
                );
                // Token helper
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    for tok in ["{name}", "{ext}", "{n}", "{date}", "{upper}", "{lower}"] {
                        if ui.add(Button::new(RichText::new(tok).size(t.typography.size_xs)
                            .color(c.accent()).monospace())
                            .fill(Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 15))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 60)))
                            .rounding(Rounding::same(s.radius_sm))).clicked() {
                            br.pattern.push_str(tok);
                        }
                    }
                });

                ui.add_space(sp.sm);

                // ── {n} ayarları ─────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Start #:").color(c.text_secondary()).size(t.typography.size_xs));
                    ui.add(egui::DragValue::new(&mut br.start_n).speed(1.0).range(0..=9999));
                    ui.add_space(sp.md);
                    ui.label(RichText::new("Pad zeros:").color(c.text_secondary()).size(t.typography.size_xs));
                    ui.add(egui::DragValue::new(&mut br.pad).speed(1.0).range(0..=6));
                    let ex = if br.pad == 0 { "1, 2, 3…".into() }
                             else { format!("{:0>width$}…", br.start_n, width = br.pad as usize) };
                    ui.label(RichText::new(format!("→ {}", ex)).color(c.text_hint()).size(t.typography.size_xs));
                });

                ui.add_space(sp.md);
                ui.separator();
                ui.add_space(sp.sm);

                // ── Canlı önizleme ───────────────────────────────────────
                if br.pattern != prev_pattern || br.pad != prev_pad || br.start_n != prev_start {
                    br.preview.clear();
                    br.update_preview(&entries);
                }

                // Conflict uyarısı
                if br.has_conflict {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠ Name conflict detected — apply disabled")
                            .color(c.danger()).size(t.typography.size_sm));
                    });
                    ui.add_space(sp.xs);
                }

                // Preview tablo header
                ui.horizontal(|ui| {
                    ui.add_sized(Vec2::new(280.0, 16.0), Label::new(
                        RichText::new("Before").color(c.text_hint()).size(t.typography.size_xs).strong()));
                    ui.label(RichText::new("→").color(c.text_hint()).size(t.typography.size_xs));
                    ui.label(RichText::new("After").color(c.text_hint()).size(t.typography.size_xs).strong());
                });

                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (old, new) in &br.preview {
                        let is_conflict = br.preview.iter()
                            .filter(|(_, n)| n == new).count() > 1;
                        ui.horizontal(|ui| {
                            ui.add_sized(Vec2::new(280.0, 18.0), Label::new(
                                RichText::new(old).color(c.text_secondary())
                                    .size(t.typography.size_xs).monospace()));
                            ui.label(RichText::new("→").color(c.text_hint()).size(t.typography.size_xs));
                            ui.label(RichText::new(new)
                                .color(if is_conflict { c.danger() } else { c.success() })
                                .size(t.typography.size_xs).monospace());
                        });
                    }
                });

                ui.add_space(sp.md);

                // ── Footer butonları ─────────────────────────────────────
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Cancel").color(c.text_secondary())
                        .size(t.typography.size_sm)).clicked() {
                        state.panels.close();
                        ctx.data_mut(|d| d.remove::<BulkRenameState>(state_id));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_apply = !br.has_conflict && !br.preview.is_empty();
                        if ui.add_enabled(can_apply, Button::new(
                            RichText::new("✓ Apply Rename").color(Color32::WHITE)
                                .size(t.typography.size_sm))
                            .fill(if can_apply { c.accent() } else { c.bg_surface1() })
                            .rounding(Rounding::same(s.radius_sm))
                            .stroke(Stroke::NONE)).clicked() {
                            // Gerçek rename
                            for (entry, (_, new_name)) in entries.iter().zip(br.preview.iter()) {
                                let parent = std::path::Path::new(&entry.path)
                                    .parent().map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                let new_path = format!("{}/{}", parent, new_name);
                                crate::backend::fs_ops::rename_item(
                                    entry.path.clone(), new_path).ok();
                            }
                            state.refresh();
                            state.panels.close();
                            ctx.data_mut(|d| d.remove::<BulkRenameState>(state_id));
                        }
                    });
                });
            });
        });

    ctx.data_mut(|d| d.insert_temp(state_id, br));
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        state.panels.close();
        ctx.data_mut(|d| d.remove::<BulkRenameState>(state_id));
    }
}

const TEMPLATES: &[(&str, &str)] = &[
    ("Numbered",      "{n}_{name}{ext}"),
    ("Name + #",      "{name}_{n}{ext}"),
    ("Date + #",      "{date}_{n}{ext}"),
    ("UPPERCASE",     "{upper}{ext}"),
    ("lowercase",     "{lower}{ext}"),
    ("Date + Name",   "{date}_{name}{ext}"),
];
