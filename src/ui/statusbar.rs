// ─────────────────────────────────────────────────────────────────────────────

use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use super::human_size;

pub fn render_bottom_panel(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    let c = &t.colors;
    let sp = &t.spacing;
    let h = t.layout.statusbar_height.max(24.0);

    let panel_fill = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_mantle().r(),
            c.bg_mantle().g(),
            c.bg_mantle().b(),
            (t.effects.panel_opacity * 255.0) as u8,
        )
    } else {
        c.bg_mantle()
    };

    egui::TopBottomPanel::bottom("app_status_bar")
        .exact_height(h)
        .resizable(false)
        .frame(egui::Frame {
            fill: panel_fill,
            stroke: Stroke::new(1.0_f32, c.border()),
            inner_margin: Margin::symmetric(sp.md, 0.0),
            ..Default::default()
        })
        .show(ctx, |ui| {
            render_content(ui, t, state);
        });
}

fn render_content(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let c  = &t.colors;
    let sp = &t.spacing;

    ui.horizontal_centered(|ui| {
        let total = state.dir.entries.len();
        let sel_count = state.selection.count();
        let sel_size: u64 = state.selected_files_size();

        // Search mode indicator
        if state.search.active && !state.search.query.is_empty() {
            ui.label(RichText::new(format!("Arama: \"{}\"  —  {} sonuç",
                state.search.query, total))
                .color(c.accent()).size(t.typography.size_xs));
        } else if state.dir.is_loading {
            ui.spinner();
            ui.label(RichText::new("Yükleniyor…")
                .color(c.accent()).size(t.typography.size_xs));
        } else {
            // Item count & Selected count clearly shown at bottom-left
            if sel_count > 0 {
                ui.label(RichText::new(format!("{} / {} seçildi", sel_count, total))
                    .color(c.accent())
                    .size(t.typography.size_xs)
                    .strong());
            } else {
                ui.label(RichText::new(format!("{} öğe", total))
                    .color(c.text_hint())
                    .size(t.typography.size_xs));
            }

            // Selected size
            if sel_count > 0 && sel_size > 0 {
                ui.label(RichText::new(format!("  •  {}", human_size(sel_size)))
                    .color(c.text_hint()).size(t.typography.size_xs));
            }

            // Git status (eğer repo içindeyse)
            if state.git_status.is_repo {
                ui.add_space(sp.sm);
                ui.label(RichText::new(format!("⎇ {}", state.git_status.branch))
                    .color(c.accent()).font(FontId::new(10.0, FontFamily::Monospace)).strong());

                let counts = state.git_status.counts;
                if counts.modified > 0 {
                    ui.label(RichText::new(format!("{}M", counts.modified))
                        .color(Color32::from_rgb(255, 209, 102)).size(10.0).strong());
                }
                if counts.added > 0 {
                    ui.label(RichText::new(format!("{}A", counts.added))
                        .color(Color32::from_rgb(90, 255, 142)).size(10.0).strong());
                }
                if counts.untracked > 0 {
                    ui.label(RichText::new(format!("{}?", counts.untracked))
                        .color(c.text_hint()).size(10.0).strong());
                }
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Loading
            if state.dir.is_loading {
                ui.label(RichText::new("Yükleniyor…")
                    .color(c.accent()).size(t.typography.size_xs));
                ui.add_space(sp.sm);
            }
            // Drag mode
            if state.drag.dragging {
                let mode = if state.drag.copy_mode { "Kopyala" } else { "Taşı" };
                ui.label(RichText::new(format!("{} {} öğe…",
                    mode, state.drag.sources.len()))
                    .color(c.accent()).size(t.typography.size_xs));
                ui.add_space(sp.sm);
            }
            // Zoom level (≠ 1.0 olunca göster)
            if (state.view_scale - 1.0).abs() > 0.05 {
                ui.label(RichText::new(format!("{:.0}%", state.view_scale * 100.0))
                    .color(c.text_hint()).size(t.typography.size_xs));
                ui.add_space(sp.sm);
            }
            // Tab count
            if state.tabs.len() > 1 {
                ui.label(RichText::new(format!("Sekme {}/{}",
                    state.tabs.active_idx + 1, state.tabs.len()))
                    .color(c.text_hint()).size(t.typography.size_xs));
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
