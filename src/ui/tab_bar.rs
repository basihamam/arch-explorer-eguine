// src/ui/tab_bar.rs
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::{AppState, TabState, PaneId};

/// Action triggered by interacting with the tab bar
pub enum TabBarAction {
    SwitchTab { pane_is_left: bool, tab_idx: usize },
    CloseTab { pane_is_left: bool, tab_idx: usize },
    NewTab { pane_is_left: bool },
    FocusPane { pane_is_left: bool },
    DropFiles { dest_path: String },
    OpenFoldersInNewTabs { pane_is_left: bool, paths: Vec<String> },
}

pub fn apply_tab_action(state: &mut AppState, action: TabBarAction) {
    match action {
        TabBarAction::DropFiles { dest_path } => {
            state.drop_files_into_dir(dest_path);
        }
        TabBarAction::OpenFoldersInNewTabs { pane_is_left, paths } => {
            let left_active = state.active_pane == PaneId::Left;
            if pane_is_left != left_active {
                state.switch_active_pane();
            }
            state.drag.cancel();
            for folder_path in paths {
                state.open_in_new_tab(folder_path);
            }
        }
        TabBarAction::FocusPane { pane_is_left } => {
            let left_active = state.active_pane == PaneId::Left;
            if pane_is_left != left_active {
                state.switch_active_pane();
            }
        }
        TabBarAction::SwitchTab { pane_is_left, tab_idx } => {
            let left_active = state.active_pane == PaneId::Left;
            if pane_is_left != left_active {
                state.switch_active_pane();
            }
            state.switch_tab(tab_idx);
        }
        TabBarAction::CloseTab { pane_is_left, tab_idx } => {
            let left_active = state.active_pane == PaneId::Left;
            if pane_is_left != left_active {
                state.switch_active_pane();
            }
            state.tabs.close_tab(tab_idx);
            let path = state.tabs.active().path.clone();
            state.nav.current_path = path;
            state.needs_reload = true;
        }
        TabBarAction::NewTab { pane_is_left } => {
            let left_active = state.active_pane == PaneId::Left;
            if pane_is_left != left_active {
                state.switch_active_pane();
            }
            let path = state.nav.current_path.clone();
            state.tabs.new_tab(path);
            let new_idx = state.tabs.active_idx;
            state.switch_tab(new_idx);
        }
    }
}

/// Renders tab bar as a top panel right beneath titlebar across the full window width
pub fn render_top_panel(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    ctx.data_mut(|d| {
        d.insert_temp(Id::new("is_file_dragging"), state.drag.dragging);
        d.insert_temp(Id::new("drag_sources"), state.drag.sources.clone());
    });

    let tab_h = 34.0_f32;
    let c = &t.colors;

    let fill_color = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_crust().r(), c.bg_crust().g(), c.bg_crust().b(),
            (t.effects.toolbar_opacity * 255.0) as u8,
        )
    } else {
        c.bg_crust()
    };

    egui::TopBottomPanel::top("top_tab_bar")
        .exact_height(tab_h)
        .show_separator_line(false)
        .frame(egui::Frame {
            fill: fill_color,
            inner_margin: egui::Margin::ZERO,
            outer_margin: egui::Margin::ZERO,
            stroke: egui::Stroke::new(1.0, c.border()), // bottom border line
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.set_height(tab_h);
            render_tab_bar_ui(ui, t, state);
        });
}

/// Renders either dual pane or single pane tab bar inside a given Ui
pub fn render_tab_bar_ui(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    ui.ctx().data_mut(|d| {
        d.insert_temp(Id::new("is_file_dragging"), state.drag.dragging);
        d.insert_temp(Id::new("drag_sources"), state.drag.sources.clone());
    });

    if state.dual_pane {
        render_dual_pane_tab_bar(ui, t, state);
    } else {
        render_single_pane_tab_bar(ui, t, state);
    }
}

/// Main fallback render function for embedding in a Ui
pub fn render(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    render_tab_bar_ui(ui, t, state);
}

/// Dual pane tab bar — Sol pane tabs on left, 4px divider in middle, Sağ pane tabs on right
pub fn render_dual_pane_tab_bar(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let tab_h = 34.0_f32;
    let avail_w = ui.available_width();
    let divider_w = 4.0_f32;
    let half_w = ((avail_w - divider_w) / 2.0).max(50.0);

    let left_active = state.active_pane == PaneId::Left;
    let snap_clone = state.right_pane.clone();
    let mut action: Option<TabBarAction> = None;

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        // ── Sol Pane Tabs (Left Half) ──
        let (left_rect, _) = ui.allocate_exact_size(Vec2::new(half_w, tab_h), Sense::hover());
        let mut left_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(left_rect)
                .layout(Layout::left_to_right(Align::Min)),
        );

        if left_active {
            render_tab_half(
                &mut left_ui,
                t,
                &state.tabs,
                true,
                true,
                half_w,
                tab_h,
                &mut action,
            );
        } else if let Some(ref snap) = snap_clone {
            render_tab_half(
                &mut left_ui,
                t,
                &snap.tabs,
                true,
                false,
                half_w,
                tab_h,
                &mut action,
            );
        }

        // ── Ortadaki ayraç (Center Divider) ──
        let (div_rect, _) = ui.allocate_exact_size(Vec2::new(divider_w, tab_h), Sense::hover());
        if ui.is_rect_visible(div_rect) {
            ui.painter().rect_filled(div_rect, Rounding::ZERO, t.colors.border());
        }

        // ── Sağ Pane Tabs (Right Half) ──
        let (right_rect, _) = ui.allocate_exact_size(Vec2::new(half_w, tab_h), Sense::hover());
        let mut right_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(right_rect)
                .layout(Layout::left_to_right(Align::Min)),
        );

        if !left_active {
            render_tab_half(
                &mut right_ui,
                t,
                &state.tabs,
                false,
                true,
                half_w,
                tab_h,
                &mut action,
            );
        } else if let Some(ref snap) = snap_clone {
            render_tab_half(
                &mut right_ui,
                t,
                &snap.tabs,
                false,
                false,
                half_w,
                tab_h,
                &mut action,
            );
        }
    });

    if let Some(act) = action {
        apply_tab_action(state, act);
    }
}

/// Single pane tab bar (full width)
pub fn render_single_pane_tab_bar(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let tab_h = 34.0_f32;
    let avail_w = ui.available_width();
    let mut action: Option<TabBarAction> = None;

    render_tab_half(
        ui,
        t,
        &state.tabs,
        true,
        true,
        avail_w,
        tab_h,
        &mut action,
    );

    if let Some(act) = action {
        apply_tab_action(state, act);
    }
}

/// Helper to render one half of the dual pane tabs (or single pane)
pub fn render_tab_half(
    ui: &mut Ui,
    t: &AexTheme,
    tabs: &TabState,
    is_left: bool,
    is_pane_active: bool,
    half_w: f32,
    tab_h: f32,
    action: &mut Option<TabBarAction>,
) {
    let c = &t.colors;
    let plus_w = 34.0_f32;
    let n = tabs.len();
    let active_idx = tabs.active_idx;

    let is_dragging = ui.ctx().data_mut(|d| d.get_temp::<bool>(Id::new("is_file_dragging")).unwrap_or(false));
    let drag_sources: Vec<String> = ui.ctx().data_mut(|d| d.get_temp(Id::new("drag_sources")).unwrap_or_default());
    let folder_sources: Vec<String> = if is_dragging {
        drag_sources.into_iter().filter(|s| {
            let p = std::path::Path::new(s);
            p.is_dir() || (s.starts_with("adb://") && (s.ends_with('/') || !s.split('/').last().unwrap_or("").contains('.')))
        }).collect()
    } else {
        Vec::new()
    };
    let has_folders = !folder_sources.is_empty();

    let pointer_pos = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .or_else(|| ui.ctx().pointer_latest_pos())
        .or_else(|| ui.input(|i| i.pointer.latest_pos()));

    let tabs_avail = (half_w - plus_w).max(0.0);

    // Calculate desired widths based on label text length (natural sizing clamped between 100 and 200)
    let mut desired_widths: Vec<f32> = tabs.tabs.iter()
        .map(|tab| (40.0 + tab.label.chars().count() as f32 * 8.0).clamp(100.0, 200.0))
        .collect();

    let total_desired: f32 = desired_widths.iter().sum();
    if total_desired > tabs_avail && total_desired > 0.0 {
        let scale = tabs_avail / total_desired;
        for w in &mut desired_widths {
            *w = (*w * scale).max(48.0);
        }
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        for i in 0..n {
            let tab = &tabs.tabs[i];
            let is_active_tab = i == active_idx;
            let tab_w = desired_widths.get(i).copied().unwrap_or(100.0);

            let (rect, resp) = ui.allocate_exact_size(Vec2::new(tab_w, tab_h), Sense::click());
            if !ui.is_rect_visible(rect) { continue; }

            // Drag & Drop onto Tab: Highlight and Drop
            let is_pointer_in_tab = pointer_pos.map_or(false, |p| rect.contains(p));
            let is_tab_hovered = resp.hovered() || is_pointer_in_tab;
            let is_drag_over = is_dragging && is_pointer_in_tab;

            // 1. Tab Background
            let bg = if is_drag_over {
                Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 80)
            } else if is_active_tab && is_pane_active {
                c.bg_surface0()
            } else if is_tab_hovered && !is_dragging {
                Color32::from_rgba_unmultiplied(c.bg_surface0().r(), c.bg_surface0().g(), c.bg_surface0().b(), 60)
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, Rounding::ZERO, bg);

            if is_drag_over {
                ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(1.5, c.accent()));
            }

            // Drop release on tab
            if is_dragging && is_pointer_in_tab && ui.input(|i| i.pointer.any_released()) && action.is_none() {
                *action = Some(TabBarAction::DropFiles { dest_path: tab.path.clone() });
            }

            // 2. Bottom accent line (Only active tab in active pane has the 2px accent underline!)
            if is_active_tab && is_pane_active {
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    rect.bottom() - 1.0,
                    Stroke::new(2.0, c.accent()),
                );
            }

            // 3. Right border (separator between tabs)
            ui.painter().vline(
                rect.right() - 0.5,
                rect.top()..=rect.bottom(),
                Stroke::new(1.0, c.border()),
            );

            // 4. Diamond icon ◈ (\u{25C8})
            // Sol pane: accent (pink), Sağ pane: warning (gold/yellow)
            let icon_color = if is_left {
                if is_pane_active {
                    c.accent()
                } else {
                    Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 120)
                }
            } else {
                if is_pane_active {
                    c.warning()
                } else {
                    Color32::from_rgba_unmultiplied(c.warning().r(), c.warning().g(), c.warning().b(), 120)
                }
            };

            ui.painter().text(
                rect.min + Vec2::new(10.0, tab_h / 2.0),
                Align2::LEFT_CENTER,
                "\u{25C8}",
                FontId::new(11.0, FontFamily::Proportional),
                icon_color,
            );

            // 5. Label
            let text_color = if is_active_tab && is_pane_active {
                c.text_primary()
            } else if is_pane_active {
                c.text_secondary()
            } else {
                c.text_disabled()
            };

            let max_chars = ((tab_w - 36.0) / 7.2).max(3.0) as usize;
            let label_text = truncate_tab_label(&tab.label, max_chars);
            ui.painter().text(
                rect.min + Vec2::new(24.0, tab_h / 2.0),
                Align2::LEFT_CENTER,
                &label_text,
                FontId::new(11.5, FontFamily::Proportional),
                text_color,
            );

            // 6. Close button ✕
            if tabs.len() > 1 && !is_dragging && (is_tab_hovered || (is_active_tab && is_pane_active)) {
                let close_size = 20.0_f32;
                let close_rect = Rect::from_center_size(
                    Pos2::new(rect.right() - 14.0, rect.center().y),
                    Vec2::splat(close_size),
                );
                let close_resp = ui.interact(
                    close_rect,
                    ui.id().with(format!("close_{}_{}", if is_left { "l" } else { "r" }, i)),
                    Sense::click(),
                ).on_hover_text("Close Tab (Ctrl+W)");
                if close_resp.hovered() {
                    ui.painter().rect_filled(
                        close_rect.shrink(2.0),
                        Rounding::same(t.shape.radius_sm),
                        Color32::from_rgba_unmultiplied(c.danger().r(), c.danger().g(), c.danger().b(), 45),
                    );
                }
                let close_color = if close_resp.hovered() {
                    c.danger()
                } else {
                    c.text_disabled()
                };
                ui.painter().text(
                    close_rect.center(),
                    Align2::CENTER_CENTER,
                    "✕",
                    FontId::new(10.0, FontFamily::Proportional),
                    close_color,
                );
                if close_resp.clicked() && action.is_none() {
                    *action = Some(TabBarAction::CloseTab { pane_is_left: is_left, tab_idx: i });
                }
            }

            // 7. Click interactions
            if resp.clicked() && action.is_none() {
                *action = Some(TabBarAction::SwitchTab { pane_is_left: is_left, tab_idx: i });
            }
            if resp.middle_clicked() && action.is_none() {
                *action = Some(TabBarAction::CloseTab { pane_is_left: is_left, tab_idx: i });
            }
        }

        // ── Empty space between tabs and + button ──
        let used_w: f32 = desired_widths.iter().sum::<f32>() + plus_w;
        let flex_w = (half_w - used_w).max(0.0);
        if flex_w > 0.0 {
            let (empty_rect, empty_resp) = ui.allocate_exact_size(Vec2::new(flex_w, tab_h), Sense::click());
            let is_empty_hovered = pointer_pos.map_or(false, |p| empty_rect.contains(p));
            let is_empty_drag_over = is_dragging && has_folders && is_empty_hovered;

            if is_empty_drag_over {
                let bg = Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 45);
                ui.painter().rect_filled(empty_rect.shrink(2.0), Rounding::same(4.0), bg);
                ui.painter().rect_stroke(
                    empty_rect.shrink(2.0),
                    Rounding::same(4.0),
                    Stroke::new(1.5, c.accent()),
                );

                let folder_count = folder_sources.len();
                let hint_text = if folder_count > 1 {
                    format!("+ {} Klasörü Yeni Sekmede Aç", folder_count)
                } else {
                    "+ Yeni Sekme Olarak Aç".to_string()
                };
                ui.painter().text(
                    empty_rect.center(),
                    Align2::CENTER_CENTER,
                    &hint_text,
                    FontId::new(11.0, FontFamily::Proportional),
                    c.accent(),
                );

                if ui.input(|i| i.pointer.any_released()) && action.is_none() {
                    *action = Some(TabBarAction::OpenFoldersInNewTabs {
                        pane_is_left: is_left,
                        paths: folder_sources.clone(),
                    });
                }
            }

            if empty_resp.clicked() && action.is_none() && !is_dragging {
                *action = Some(TabBarAction::FocusPane { pane_is_left: is_left });
            }
        }

        // ── + (New tab) Button at the FAR RIGHT of this half ──
        let (plus_rect, plus_resp) = ui.allocate_exact_size(Vec2::new(plus_w, tab_h), Sense::click());
        let plus_resp = plus_resp.on_hover_text("New Tab (Ctrl+T)");
        let is_plus_hovered = pointer_pos.map_or(false, |p| plus_rect.contains(p));
        let is_plus_drag_over = is_dragging && has_folders && is_plus_hovered;

        if ui.is_rect_visible(plus_rect) {
            // Left border for + button
            ui.painter().vline(
                plus_rect.left() - 0.5,
                plus_rect.top()..=plus_rect.bottom(),
                Stroke::new(1.0, c.border()),
            );

            let plus_bg = if is_plus_drag_over {
                Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 80)
            } else if plus_resp.hovered() && !is_dragging {
                c.bg_surface0()
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(plus_rect, Rounding::ZERO, plus_bg);

            if is_plus_drag_over {
                ui.painter().rect_stroke(plus_rect, Rounding::ZERO, Stroke::new(1.5, c.accent()));
            }

            let plus_color = if is_plus_drag_over {
                c.accent()
            } else if plus_resp.hovered() {
                c.text_primary()
            } else if is_pane_active {
                c.text_hint()
            } else {
                c.text_disabled()
            };

            ui.painter().text(
                plus_rect.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::new(17.0, FontFamily::Proportional),
                plus_color,
            );
        }

        if is_plus_drag_over && ui.input(|i| i.pointer.any_released()) && action.is_none() {
            *action = Some(TabBarAction::OpenFoldersInNewTabs {
                pane_is_left: is_left,
                paths: folder_sources.clone(),
            });
        }

        if plus_resp.clicked() && action.is_none() && !is_dragging {
            *action = Some(TabBarAction::NewTab { pane_is_left: is_left });
        }
    });
}

// ── Legacy / compatibility helpers for tests ──────────────────────────────────
pub fn render_pane(ui: &mut Ui, t: &AexTheme, state: &mut AppState, is_left: bool, pane_is_active: bool) {
    let half_w = ui.available_width();
    let tab_h = ui.available_height().max(32.0);
    let mut action = None;
    render_tab_half(
        ui,
        t,
        &state.tabs,
        is_left,
        pane_is_active,
        half_w,
        tab_h,
        &mut action,
    );
    if let Some(act) = action {
        apply_tab_action(state, act);
    }
}

pub fn render_snapshot_tabs(
    ui: &mut Ui,
    t: &AexTheme,
    snap: &crate::state::PaneSnapshot,
    is_left: bool,
    mut on_activate: impl FnMut(),
) {
    let half_w = ui.available_width();
    let tab_h = ui.available_height().max(32.0);
    let mut action = None;
    render_tab_half(
        ui,
        t,
        &snap.tabs,
        is_left,
        false,
        half_w,
        tab_h,
        &mut action,
    );
    if action.is_some() {
        on_activate();
    }
}

fn truncate_tab_label(s: &str, max_chars: usize) -> String {
    if max_chars < 3 { return "…".to_string(); }
    if s.chars().count() <= max_chars { return s.to_string(); }
    let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_bar_rects() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let t = AexTheme::default();
                let mut state = AppState::new();
                state.open_dual_pane();

                let make_entry = |name: &str| crate::backend::fs_ops::FileEntry {
                    name: name.to_string(),
                    path: format!("/tmp/{}", name),
                    is_dir: false,
                    is_symlink: false,
                    size: 1024,
                    modified: "now".to_string(),
                    created: "now".to_string(),
                    permissions: "rwx".to_string(),
                    mime_type: "text/plain".to_string(),
                    extension: "txt".to_string(),
                    hidden: false,
                    is_executable: false,
                };

                state.dir.entries = std::sync::Arc::new(vec![make_entry("file1.txt"), make_entry("file2.txt")]);
                if let Some(ref mut snap) = state.right_pane {
                    snap.dir.entries = std::sync::Arc::new(vec![make_entry("file1.txt"), make_entry("file2.txt")]);
                }

                let avail_w = 800.0;
                let splitter = 1.0;
                let pane_w = (avail_w - splitter) / 2.0;
                let tab_h = 32.0;
                let body_h = 400.0;

                // When Left is active:
                let left_active = true;
                let snap = state.right_pane.clone().unwrap();

                let tabs_row = ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                    let (left_rect, _) = ui.allocate_exact_size(egui::Vec2::new(pane_w, tab_h), Sense::hover());
                    let mut left_ui = ui.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::left_to_right(Align::Min)));
                    render_pane(&mut left_ui, &t, &mut state, true, true);

                    ui.add_space(splitter);

                    let (right_rect, _) = ui.allocate_exact_size(egui::Vec2::new(pane_w, tab_h), Sense::hover());
                    let mut right_ui = ui.new_child(UiBuilder::new().max_rect(right_rect).layout(Layout::left_to_right(Align::Min)));
                    render_snapshot_tabs(&mut right_ui, &t, &snap, false, || {});

                    assert_eq!(left_rect.min.x, 8.0);
                    assert_eq!(left_rect.max.x, 8.0 + pane_w);
                    assert_eq!(right_rect.min.x, 8.0 + pane_w + splitter);
                    assert_eq!(right_rect.max.x, 8.0 + 2.0 * pane_w + splitter);
                });
                assert_eq!(tabs_row.response.rect.height(), tab_h);

                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                    let (active_rect, _) = ui.allocate_exact_size(egui::Vec2::new(pane_w, body_h), Sense::hover());
                    let mut active_ui = ui.new_child(UiBuilder::new().max_rect(active_rect).layout(Layout::top_down(Align::Min)));
                    let (dummy_tx, _) = std::sync::mpsc::channel();
                    crate::app::render_active_pane(&mut active_ui, &t, &mut state, "left", &dummy_tx);

                    ui.add_space(splitter);

                    let (inactive_rect, _) = ui.allocate_exact_size(egui::Vec2::new(pane_w, body_h), Sense::hover());
                    let mut inactive_ui = ui.new_child(UiBuilder::new().max_rect(inactive_rect).layout(Layout::top_down(Align::Min)));
                    crate::app::render_inactive_pane(&mut inactive_ui, &t, &snap, "right", &mut state, &dummy_tx);

                    assert_eq!(active_rect.min.x, 8.0);
                    assert_eq!(active_rect.max.x, 8.0 + pane_w);
                    assert_eq!(inactive_rect.min.x, 8.0 + pane_w + splitter);
                    assert_eq!(inactive_rect.max.x, 8.0 + 2.0 * pane_w + splitter);
                    assert_eq!(active_rect.min.y, inactive_rect.min.y);
                });

                // When Right is active:
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                    let (left_rect, _) = ui.allocate_exact_size(egui::Vec2::new(pane_w, tab_h), Sense::hover());
                    let mut left_ui = ui.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::left_to_right(Align::Min)));
                    render_snapshot_tabs(&mut left_ui, &t, &snap, true, || {});

                    ui.add_space(splitter);

                    let (right_rect, _) = ui.allocate_exact_size(egui::Vec2::new(pane_w, tab_h), Sense::hover());
                    let mut right_ui = ui.new_child(UiBuilder::new().max_rect(right_rect).layout(Layout::left_to_right(Align::Min)));
                    render_pane(&mut right_ui, &t, &mut state, false, true);

                    assert_eq!(left_rect.min.x, 8.0);
                    assert_eq!(left_rect.max.x, 8.0 + pane_w);
                    assert_eq!(right_rect.min.x, 8.0 + pane_w + splitter);
                    assert_eq!(right_rect.max.x, 8.0 + 2.0 * pane_w + splitter);
                });
            });
        });
    }

    #[test]
    fn test_dual_pane_tab_bar_full() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let t = AexTheme::default();
            let mut state = AppState::new();
            state.open_dual_pane();

            // Add tabs to left and right
            state.tabs.new_tab("/home/lordblue".to_string());
            if let Some(ref mut snap) = state.right_pane {
                snap.tabs.new_tab("/home/lordblue/Android".to_string());
            }

            render_top_panel(ctx, &t, &mut state);
        });
    }

    #[test]
    fn test_tab_actions() {
        let mut state = AppState::new();
        state.open_dual_pane();
        assert_eq!(state.active_pane, PaneId::Left);

        // Add tabs to right pane
        if let Some(ref mut snap) = state.right_pane {
            snap.tabs.new_tab("/home/lordblue/Android".to_string());
            assert_eq!(snap.tabs.len(), 2);
        }

        // Action 1: SwitchTab on inactive pane (Right) -> should switch pane and select tab
        apply_tab_action(&mut state, TabBarAction::SwitchTab { pane_is_left: false, tab_idx: 1 });
        assert_eq!(state.active_pane, PaneId::Right);
        assert_eq!(state.tabs.active_idx, 1);

        // Action 2: NewTab on inactive pane (Left) -> should switch pane and add tab
        apply_tab_action(&mut state, TabBarAction::NewTab { pane_is_left: true });
        assert_eq!(state.active_pane, PaneId::Left);
        assert_eq!(state.tabs.len(), 2);

        // Action 3: CloseTab
        apply_tab_action(&mut state, TabBarAction::CloseTab { pane_is_left: true, tab_idx: 1 });
        assert_eq!(state.tabs.len(), 1);

        // Action 4: FocusPane
        apply_tab_action(&mut state, TabBarAction::FocusPane { pane_is_left: false });
        assert_eq!(state.active_pane, PaneId::Right);
    }

    #[test]
    fn test_single_pane_tab_bar() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let t = AexTheme::default();
                let mut state = AppState::new();
                state.tabs.new_tab("/home/user/docs".to_string());
                render_single_pane_tab_bar(ui, &t, &mut state);
            });
        });
    }

    #[test]
    fn test_tab_drop_action() {
        let mut state = AppState::new();
        state.drag.start(vec!["/tmp/test.txt".into()], true);
        assert!(state.drag.dragging);
        apply_tab_action(&mut state, TabBarAction::DropFiles { dest_path: "/home/user".into() });
        assert!(!state.drag.dragging);
    }

    #[test]
    fn test_inactive_tab_close_hover_no_flicker() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let t = AexTheme::default();
                let mut state = AppState::new();
                state.tabs.new_tab("/home/user/tab2".to_string());
                assert_eq!(state.tabs.len(), 2);
                assert_eq!(state.tabs.active_idx, 1);

                // Render with inactive tab (idx 0)
                let mut action = None;
                render_tab_half(ui, &t, &state.tabs, true, true, 400.0, 34.0, &mut action);
            });
        });
    }

    #[test]
    fn test_open_folders_in_new_tabs_action() {
        let mut state = AppState::new();
        state.open_dual_pane();
        assert_eq!(state.active_pane, PaneId::Left);
        let initial_tabs = state.tabs.len();

        // Drag a folder
        let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
        state.drag.start(vec![temp_dir.clone()], false);
        assert!(state.drag.dragging);

        // Drop on right pane empty area -> should switch to right pane and open tab
        apply_tab_action(&mut state, TabBarAction::OpenFoldersInNewTabs {
            pane_is_left: false,
            paths: vec![temp_dir.clone()],
        });

        assert!(!state.drag.dragging);
        assert_eq!(state.active_pane, PaneId::Right);
        assert_eq!(state.tabs.active().path, temp_dir);
    }

    #[test]
    fn test_tab_drag_drop_simulation() {
        let ctx = egui::Context::default();
        let t = AexTheme::default();
        let mut state = AppState::new();
        state.tabs.new_tab("/home/user/tab2".to_string());
        assert_eq!(state.tabs.len(), 2);

        // Start dragging a file
        state.drag.start(vec!["/tmp/test.txt".to_string()], false);

        // Sync ctx data
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("is_file_dragging"), true);
            d.insert_temp(Id::new("drag_sources"), vec!["/tmp/test.txt".to_string()]);
        });

        // Frame 1: Pointer inside tab 0 (x=50, y=17) with button released
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::PointerMoved(egui::pos2(50.0, 17.0)));
        raw_input.events.push(egui::Event::PointerButton {
            pos: egui::pos2(50.0, 17.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });

        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut action = None;
                // Render tab bar with pointer over first tab
                render_tab_half(ui, &t, &state.tabs, true, true, 400.0, 34.0, &mut action);
                assert!(action.is_some(), "DropFiles action should be triggered on drop release over tab");
                if let Some(TabBarAction::DropFiles { dest_path }) = action {
                    assert_eq!(dest_path, state.tabs.tabs[0].path);
                } else {
                    panic!("Expected TabBarAction::DropFiles");
                }
            });
        });
    }

    #[test]
    fn test_empty_tab_bar_folder_drop_simulation() {
        let ctx = egui::Context::default();
        let t = AexTheme::default();
        let mut state = AppState::new();
        let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
        state.drag.start(vec![temp_dir.clone()], false);

        // Sync ctx data
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("is_file_dragging"), true);
            d.insert_temp(Id::new("drag_sources"), vec![temp_dir.clone()]);
        });

        // Pointer released at x=300 (empty space after the tabs), y=17
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::PointerMoved(egui::pos2(300.0, 17.0)));
        raw_input.events.push(egui::Event::PointerButton {
            pos: egui::pos2(300.0, 17.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });

        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut action = None;
                render_tab_half(ui, &t, &state.tabs, true, true, 400.0, 34.0, &mut action);
                assert!(action.is_some(), "OpenFoldersInNewTabs action should be triggered on empty area");
                if let Some(TabBarAction::OpenFoldersInNewTabs { pane_is_left, paths }) = action {
                    assert!(pane_is_left);
                    assert_eq!(paths, vec![temp_dir.clone()]);
                } else {
                    panic!("Expected TabBarAction::OpenFoldersInNewTabs");
                }
            });
        });
    }
}
