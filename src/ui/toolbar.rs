// ─────────────────────────────────────────────────────────────────────────────

use egui::*;
use crate::theme::types::AexTheme;
use crate::state::{AppState, ViewMode, MediaGridStyle};
use super::icon;

pub fn render(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;
    let h  = t.layout.toolbar_height;

    let tb_fill = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_mantle().r(), c.bg_mantle().g(), c.bg_mantle().b(),
            (t.effects.toolbar_opacity * 255.0) as u8,
        )
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame {
        inner_margin: Margin::symmetric(sp.md, sp.sm),
        fill: tb_fill,
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.set_height(h);
        ui.horizontal(|ui| {
            // ── Nav Buttons ───────────────────────────────────────────────
            nav_btn(ui, t, icon(t, "back"), state.nav.can_go_back(), || {
                state.go_back();
            }).on_hover_text("Back (Alt+Left)");
            nav_btn(ui, t, icon(t, "forward"), state.nav.can_go_forward(), || {
                state.go_forward();
            }).on_hover_text("Forward (Alt+Right)");
            nav_btn(ui, t, icon(t, "up"), state.nav.can_go_up(), || {
                state.go_up();
            }).on_hover_text("Parent Directory (Alt+Up)");
            if nav_btn(ui, t, icon(t, "refresh"), true, || {}).on_hover_text("Refresh (F5)").clicked() {
                state.refresh();
            }

            ui.add_space(sp.sm);

            // ── Separator ─────────────────────────────────────────────────
            ui.painter().vline(
                ui.cursor().left(),
                ui.cursor().top()..=ui.cursor().bottom() - sp.sm,
                Stroke::new(1.0_f32, c.border()),
            );
            ui.add_space(sp.sm);

            // ── Address Bar / Breadcrumb ─────────────────────────────────
            let edit_mode_id = ui.id().with("addr_edit_mode");
            let mut edit_mode: bool = ui.ctx().data_mut(|d|
                d.get_temp(edit_mode_id).unwrap_or(false)
            );

            // ── Responsive Breakpoints ────────────────────────────────────
            let total_avail_w = ui.available_width();
            let show_view_modes = total_avail_w >= 640.0;
            let show_dual_pane = total_avail_w >= 540.0;
            let show_hidden_toggle = total_avail_w >= 460.0;
            let show_preview = total_avail_w >= 390.0;
            let show_search_btn = total_avail_w >= 320.0 || state.search.active;

            // Accurate calculation of right cluster width
            let mut right_cluster_w = 32.0; // Folder Options button (28px + 4px spacing)
            if show_view_modes {
                right_cluster_w += 96.0 + 4.0; // Grid, List, Compact (3 * 32px) + spacer
            }
            if show_hidden_toggle { right_cluster_w += 32.0; }
            if show_dual_pane { right_cluster_w += 32.0; }
            if show_preview { right_cluster_w += 32.0; }
            right_cluster_w += 12.0; // Safety & padding margin

            // Address bar & search width allocation — prevents overlap with right cluster
            let (addr_width, search_bar_w) = if state.search.active {
                let avail_for_nav = (total_avail_w - right_cluster_w - sp.sm).max(180.0);
                let search_w = (avail_for_nav * 0.45).clamp(130.0, 240.0);
                let addr_w = (avail_for_nav - search_w - sp.sm).max(60.0);
                (addr_w, Some(search_w))
            } else if show_search_btn {
                let addr_w = (total_avail_w - right_cluster_w - 36.0).max(80.0);
                (addr_w, None)
            } else {
                let addr_w = (total_avail_w - right_cluster_w).max(80.0);
                (addr_w, None)
            };

            let just_entered_id = ui.id().with("addr_just_entered");
            let show_breadcrumb = t.layout.show_breadcrumb;

            if edit_mode || !show_breadcrumb {
                // Düzenleme modu veya breadcrumb kapalıysa — TextEdit
                let te_id = ui.id().with("addr_textedit");

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.nav.current_path)
                        .id(te_id)
                        .frame(true)
                        .font(FontId::new(t.typography.size_sm, FontFamily::Monospace))
                        .text_color(c.text_primary())
                        .hint_text("Yol girin…")
                        .desired_width(addr_width),
                );

                // request_focus SADECE edit_mode'a geçildiğinde ilk frame'de
                let just_entered: bool = ui.ctx()
                    .data_mut(|d| d.get_temp(just_entered_id).unwrap_or(true));
                if show_breadcrumb && just_entered {
                    resp.request_focus();
                    let char_len = state.nav.current_path.chars().count();
                    let mut te_state = egui::text_edit::TextEditState::load(ui.ctx(), te_id).unwrap_or_default();
                    te_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(char_len),
                    )));
                    te_state.store(ui.ctx(), te_id);
                    ui.ctx().data_mut(|d| d.insert_temp(just_entered_id, false));
                }

                if resp.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        let path = state.nav.current_path.clone();
                        state.navigate(path);
                    }
                    if show_breadcrumb {
                        edit_mode = false;
                        ui.ctx().data_mut(|d| d.insert_temp::<bool>(just_entered_id, true));
                    }
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) && show_breadcrumb {
                    edit_mode = false;
                    ui.ctx().data_mut(|d| d.insert_temp::<bool>(just_entered_id, true));
                }
            } else {
                // Breadcrumb modu — segmentler halinde göster
                let (_resp, wants_edit) = breadcrumb(ui, t, state, addr_width);
                if wants_edit {
                    edit_mode = true;
                    ui.ctx().data_mut(|d| d.insert_temp::<bool>(just_entered_id, true));
                }
            }

            ui.ctx().data_mut(|d| d.insert_temp(edit_mode_id, edit_mode));

            ui.add_space(sp.sm);

            // ── Search ────────────────────────────────────────────────────
            if let Some(sw) = search_bar_w {
                search_bar(ui, t, state, sw);
            } else if show_search_btn {
                if nav_btn(ui, t, icon(t, "search"), true, || {}).on_hover_text("Search (Ctrl+F)").clicked() {
                    state.search.active = true;
                    state.search.request_focus = true;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // ── Preview Toggle (F3) ───────────────────────────────────
                if show_preview {
                    let preview_active = state.panels.active == crate::state::ActivePanel::Preview;
                    let preview_color = if preview_active { c.accent() } else { c.text_hint() };
                    let preview_resp = icon_btn(ui, t, icon(t, "preview"), preview_color).on_hover_text("Preview (F3)");
                    if preview_resp.clicked() {
                        if preview_active {
                            state.media_player.stop();
                            state.panels.close();
                        } else {
                            state.panels.open_preview();
                        }
                    }
                }

                // ── Dual Pane Toggle (Ctrl+P) ─────────────────────────────
                if show_dual_pane {
                    let dp_color = if state.dual_pane { c.accent() } else { c.text_hint() };
                    let dp_resp  = icon_btn(ui, t, "\u{f0db}", dp_color).on_hover_text("Dual Pane (Ctrl+P)");
                    if dp_resp.clicked() {
                        if state.dual_pane { state.close_dual_pane(); }
                        else               { state.open_dual_pane(); }
                    }
                }

                // ── Hidden toggle ─────────────────────────────────────────
                if show_hidden_toggle {
                    let hidden_color = if state.dir.show_hidden { c.accent() } else { c.text_hint() };
                    if icon_btn(ui, t, icon(t, if state.dir.show_hidden { "visible" } else { "hidden" }), hidden_color)
                        .on_hover_text(if state.dir.show_hidden { "Hide Hidden Files (Ctrl+H)" } else { "Show Hidden Files (Ctrl+H)" })
                        .clicked()
                    {
                        state.dir.show_hidden = !state.dir.show_hidden;
                        state.refresh();
                    }
                }

                // ── Separator / space before view modes ──────────────────
                if show_view_modes && (show_hidden_toggle || show_dual_pane || show_preview) {
                    ui.add_space(sp.xs);
                }

                // ── View Mode Toggles (Compact, List, Grid) ───────────────
                // Added right-to-left: Compact (right), List (mid), Grid (left)
                if show_view_modes {
                    let compact_color = if state.view_mode == ViewMode::Compact { c.accent() } else { c.text_hint() };
                    let list_color = if state.view_mode == ViewMode::List { c.accent() } else { c.text_hint() };
                    let grid_color = if state.view_mode == ViewMode::Grid { c.accent() } else { c.text_hint() };

                    if icon_btn(ui, t, icon(t, "compact"), compact_color).on_hover_text("Compact View (Ctrl+3)").clicked() {
                        state.set_view_mode_for_current_dir(ViewMode::Compact);
                    }
                    if icon_btn(ui, t, icon(t, "list"), list_color).on_hover_text("List View (Ctrl+2)").clicked() {
                        state.set_view_mode_for_current_dir(ViewMode::List);
                    }
                    if icon_btn(ui, t, icon(t, "grid"), grid_color).on_hover_text("Grid View (Ctrl+1)").clicked() {
                        state.set_view_mode_for_current_dir(ViewMode::Grid);
                    }

                    ui.add_space(sp.xs);
                }

                // ── Folder Options / Settings icon (FAR LEFT of right cluster!) ──
                let view_opts_id = Id::new("toolbar_view_options_popup_open");
                let mut popup_open: bool = ui.ctx().data_mut(|d| d.get_temp(view_opts_id).unwrap_or(false));
                let opts_color = if popup_open { c.accent() } else { c.text_hint() };
                let opts_btn = icon_btn(ui, t, icon(t, "settings"), opts_color)
                    .on_hover_text("Folder Options & View Settings");
                if opts_btn.clicked() {
                    popup_open = !popup_open;
                    ui.ctx().data_mut(|d| d.insert_temp(view_opts_id, popup_open));
                }

                if popup_open {
                    let popup_w = 230.0;
                    let screen_r = ui.ctx().screen_rect().right();
                    let popup_x = opts_btn.rect.left().clamp(4.0, (screen_r - popup_w - 4.0).max(4.0));
                    let popup_pos = Pos2::new(popup_x, opts_btn.rect.bottom() + 4.0);

                    let menu_fill = if t.effects.glass_panels {
                        Color32::from_rgba_unmultiplied(
                            c.bg_mantle().r(),
                            c.bg_mantle().g(),
                            c.bg_mantle().b(),
                            (t.effects.panel_opacity * 255.0) as u8,
                        )
                    } else {
                        c.bg_mantle()
                    };

                    let mut close_popup = false;
                    let mut new_style: Option<MediaGridStyle> = None;

                    egui::Area::new(Id::new("toolbar_view_options_popup_area"))
                        .fixed_pos(popup_pos)
                        .order(Order::Tooltip)
                        .show(ui.ctx(), |ui| {
                            egui::Frame {
                                fill: menu_fill,
                                rounding: Rounding::same(s.radius_md),
                                stroke: Stroke::new(1.0_f32, c.border()),
                                shadow: epaint::Shadow {
                                    offset: Vec2::new(0.0, 4.0),
                                    blur: 12.0,
                                    spread: 0.0,
                                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
                                },
                                inner_margin: Margin::same(sp.xs + 3.0),
                                ..Default::default()
                            }
                            .show(ui, |ui| {
                                ui.set_width(popup_w - (sp.xs + 3.0) * 2.0);

                                let has_hidden = !show_view_modes
                                    || !show_hidden_toggle
                                    || !show_dual_pane
                                    || !show_preview
                                    || (!show_search_btn && !state.search.active);

                                if has_hidden {
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new("GÖRÜNÜM & ARAÇLAR / TOOLBAR")
                                            .size(9.0)
                                            .color(c.text_hint())
                                            .strong(),
                                    );
                                    ui.add_space(3.0);

                                    // Hidden Search
                                    if !show_search_btn && !state.search.active {
                                        if menu_action_item(ui, t, icon(t, "search"), "Search (Ctrl+F)", false, false).clicked() {
                                            state.search.active = true;
                                            state.search.request_focus = true;
                                            close_popup = true;
                                        }
                                    }

                                    // Hidden View Modes
                                    if !show_view_modes {
                                        if menu_action_item(ui, t, icon(t, "grid"), "Grid View (Ctrl+1)", state.view_mode == ViewMode::Grid, true).clicked() {
                                            state.set_view_mode_for_current_dir(ViewMode::Grid);
                                            close_popup = true;
                                        }
                                        if menu_action_item(ui, t, icon(t, "list"), "List View (Ctrl+2)", state.view_mode == ViewMode::List, true).clicked() {
                                            state.set_view_mode_for_current_dir(ViewMode::List);
                                            close_popup = true;
                                        }
                                        if menu_action_item(ui, t, icon(t, "compact"), "Compact View (Ctrl+3)", state.view_mode == ViewMode::Compact, true).clicked() {
                                            state.set_view_mode_for_current_dir(ViewMode::Compact);
                                            close_popup = true;
                                        }
                                    }

                                    // Hidden toggle
                                    if !show_hidden_toggle {
                                        let is_hid = state.dir.show_hidden;
                                        let hid_lbl = if is_hid { "Hide Hidden Files (Ctrl+H)" } else { "Show Hidden Files (Ctrl+H)" };
                                        let hid_ico = icon(t, if is_hid { "visible" } else { "hidden" });
                                        if menu_action_item(ui, t, hid_ico, hid_lbl, is_hid, true).clicked() {
                                            state.dir.show_hidden = !state.dir.show_hidden;
                                            state.refresh();
                                            close_popup = true;
                                        }
                                    }

                                    // Dual Pane
                                    if !show_dual_pane {
                                        if menu_action_item(ui, t, "\u{f0db}", "Dual Pane (Ctrl+P)", state.dual_pane, true).clicked() {
                                            if state.dual_pane { state.close_dual_pane(); }
                                            else               { state.open_dual_pane(); }
                                            close_popup = true;
                                        }
                                    }

                                    // Preview Panel
                                    if !show_preview {
                                        let is_prev = state.panels.active == crate::state::ActivePanel::Preview;
                                        if menu_action_item(ui, t, icon(t, "preview"), "Preview Panel (F3)", is_prev, true).clicked() {
                                            if is_prev {
                                                state.media_player.stop();
                                                state.panels.close();
                                            } else {
                                                state.panels.open_preview();
                                            }
                                            close_popup = true;
                                        }
                                    }

                                    ui.add_space(4.0);
                                    ui.painter().hline(
                                        ui.cursor().left()..=ui.cursor().right(),
                                        ui.cursor().top(),
                                        Stroke::new(1.0_f32, c.border()),
                                    );
                                    ui.add_space(4.0);
                                }

                                // ── Daha Fazla / More Options ────────────────
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new("DAHA FAZLA / MORE OPTIONS")
                                        .size(9.0)
                                        .color(c.text_hint())
                                        .strong(),
                                );
                                ui.add_space(3.0);

                                // Option 1: Nemo Uniform Grid
                                let is_nemo = state.media_grid_style == MediaGridStyle::NemoGrid;
                                if menu_action_item(ui, t, icon(t, "grid"), "Nemo Uniform Grid", is_nemo, true).clicked() {
                                    new_style = Some(MediaGridStyle::NemoGrid);
                                    close_popup = true;
                                }

                                // Option 2: Masonry (Mansion) Flow
                                let is_masonry = state.media_grid_style == MediaGridStyle::Masonry;
                                if menu_action_item(ui, t, "\u{f009}", "Masonry (Mansion) Flow", is_masonry, true).clicked() {
                                    new_style = Some(MediaGridStyle::Masonry);
                                    close_popup = true;
                                }

                                // Check click outside or Escape
                                let popup_rect = ui.min_rect();
                                if ui.input(|i| i.pointer.any_pressed()) {
                                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                        if !popup_rect.contains(pos) && !opts_btn.rect.contains(pos) {
                                            close_popup = true;
                                        }
                                    }
                                }
                                if ui.input(|i| i.key_pressed(Key::Escape)) {
                                    close_popup = true;
                                }
                            });
                        });

                    if let Some(style) = new_style {
                        state.set_media_grid_style(style);
                    }
                    if close_popup {
                        ui.ctx().data_mut(|d| d.insert_temp(view_opts_id, false));
                    }
                }
            });
        });
    });

    // Toolbar altına ince çizgi
    let rect = ui.min_rect();
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.border()),
    );
}

/// Breadcrumb — path'i / ile ayır, her segmente tıklanabilir buton koy
fn breadcrumb(ui: &mut Ui, t: &AexTheme, state: &mut AppState, width: f32) -> (Response, bool) {
    let c  = &t.colors;
    let s  = &t.shape;
    let path = state.nav.current_path.clone();
    let mut wants_edit = false;

    let (outer_rect, outer_resp) = ui.allocate_exact_size(
        Vec2::new(width, 28.0), Sense::click(),
    );

    if ui.is_rect_visible(outer_rect) {
        // Arka plan
        ui.painter().rect_filled(outer_rect, Rounding::same(s.radius_sm), c.bg_surface0());

        let mut x = outer_rect.left() + 8.0;
        let y_center = outer_rect.center().y;

        // Pink ◈ icon matching screenshot design
        let is_adb = path.starts_with("adb://");
        let icon_w = 16.0_f32;
        if x + icon_w < outer_rect.right() {
            let icon_rect = Rect::from_min_size(Pos2::new(x, outer_rect.top()), Vec2::new(icon_w, 28.0));
            let icon_resp = ui.interact(icon_rect, ui.id().with("bc_icon"), Sense::click());
            if icon_resp.clicked() {
                wants_edit = true;
            }
            let icon_str = if is_adb { icon(t, "phone") } else { "◈" };
            ui.painter().text(
                Pos2::new(x + icon_w / 2.0, y_center),
                Align2::CENTER_CENTER,
                icon_str,
                FontId::new(12.0, FontFamily::Proportional),
                c.accent(),
            );
            x += icon_w;
        }

        let is_adb_prefix = path.starts_with("adb://");
        let (root_label, segments, base_prefix) = if is_adb_prefix {
            let without = path.strip_prefix("adb://").unwrap_or("");
            let mut parts = without.splitn(2, '/');
            let serial = parts.next().unwrap_or("").to_string();
            let sub = parts.next().unwrap_or("");
            let segs: Vec<&str> = sub.split('/').filter(|s| !s.is_empty()).collect();
            let dev_label = state.sidebar.adb_devices.iter()
                .find(|d| d.serial == serial)
                .map(|d| d.display_name())
                .unwrap_or_else(|| serial.clone());
            (dev_label, segs, format!("adb://{}", serial))
        } else {
            let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            ("/".to_string(), segs, "".to_string())
        };

        let mut any_seg_clicked = false;

        // Root / veya Telefon Adı
        let root_w = if is_adb_prefix { (root_label.len() as f32 * 7.5 + 16.0).clamp(50.0, 160.0) } else { 16.0_f32 };
        if x + root_w < outer_rect.right() {
            let root_rect = Rect::from_min_size(
                Pos2::new(x, outer_rect.top()),
                Vec2::new(root_w, 28.0),
            );
            let root_resp = ui.interact(root_rect, ui.id().with("bc_root"), Sense::click());
            let root_pointer_over = ui.ctx().pointer_interact_pos()
                .or_else(|| ui.ctx().pointer_hover_pos())
                .map(|p| root_rect.contains(p))
                .unwrap_or(false);

            let root_target = if is_adb_prefix {
                format!("{}/sdcard", base_prefix)
            } else {
                "/".to_string()
            };

            root_resp.context_menu(|ui| {
                if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
                    state.open_in_new_tab(root_target.clone());
                    ui.close_menu();
                }
            });

            if state.drag.dragging && root_pointer_over {
                ui.painter().rect_filled(root_rect.shrink(1.0), Rounding::same(s.radius_sm),
                    Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 40));
                ui.painter().rect_stroke(root_rect.shrink(1.0), Rounding::same(s.radius_sm), Stroke::new(1.5, c.accent()));
                if ui.input(|i| i.pointer.any_released()) {
                    let sources = state.drag.sources.clone();
                    let copy = state.drag.copy_mode;
                    state.drag.cancel();
                    state.start_transfer(sources, root_target.clone(), copy);
                }
            }

            let color = if root_resp.hovered() || (state.drag.dragging && root_pointer_over) { c.accent() } else { c.text_hint() };
            ui.painter().text(Pos2::new(x + root_w/2.0, y_center), Align2::CENTER_CENTER,
                &root_label, FontId::new(t.typography.size_sm, FontFamily::Proportional), color);
            if root_resp.clicked() {
                any_seg_clicked = true;
                if segments.is_empty() {
                    wants_edit = true;
                } else {
                    state.navigate(root_target);
                }
            }
            if root_resp.secondary_clicked() {
                any_seg_clicked = true;
            }
            x += root_w;
        }

        let n = segments.len();
        if n > 0 {
            let avail_w = (outer_rect.right() - x - 8.0).max(30.0);
            let seg_widths: Vec<f32> = segments.iter()
                .map(|s| (s.len() as f32 * 7.5 + 16.0).clamp(32.0, 140.0))
                .collect();
            let total_needed: f32 = seg_widths.iter().map(|w| w + 10.0).sum();

            let mut start_idx = 0;
            if total_needed > avail_w {
                // Sığmıyor: Son klasörü ve sığabildiği kadar gerisindeki ebeveynleri koru,
                // daha önceki kısımları "..." olarak daralt
                let ellipsis_w = 26.0_f32 + 10.0_f32; // "..." butonu ve ayırıcı
                let mut used = seg_widths[n - 1] + 10.0;
                start_idx = n - 1;
                for idx in (0..n - 1).rev() {
                    let w = seg_widths[idx] + 10.0;
                    if used + w + ellipsis_w <= avail_w {
                        used += w;
                        start_idx = idx;
                    } else {
                        break;
                    }
                }
            }

            // Eğer baştaki segmentler daraltıldıysa "..." göster
            if start_idx > 0 {
                // Separator ›
                if x + 10.0 < outer_rect.right() {
                    ui.painter().text(Pos2::new(x + 5.0, y_center), Align2::CENTER_CENTER,
                        "›", FontId::new(t.typography.size_sm, FontFamily::Proportional), c.text_disabled());
                    x += 10.0;
                }

                let ell_w = 24.0_f32;
                if x + ell_w < outer_rect.right() - 4.0 {
                    let ell_rect = Rect::from_min_size(Pos2::new(x, outer_rect.top() + 2.0), Vec2::new(ell_w, 24.0));
                    let ell_resp = ui.interact(ell_rect, ui.id().with("bc_ellipsis"), Sense::click());
                    let ell_hover = ell_resp.hovered();
                    if ell_hover {
                        ui.painter().rect_filled(ell_rect, Rounding::same(s.radius_sm), c.bg_surface1());
                        let hidden_path = if is_adb_prefix {
                            format!("{}/{}", base_prefix, segments[0..start_idx].join("/"))
                        } else {
                            format!("/{}", segments[0..start_idx].join("/"))
                        };
                        egui::show_tooltip_text(ui.ctx(), ui.layer_id(), ui.id().with("bc_ell_tt"),
                            format!("{}\n(Click to jump to parent)", hidden_path));
                    }
                    ui.painter().text(
                        Pos2::new(x + ell_w / 2.0, y_center),
                        Align2::CENTER_CENTER,
                        "...",
                        FontId::new(t.typography.size_sm, FontFamily::Monospace),
                        if ell_hover { c.accent() } else { c.text_hint() },
                    );

                    let jump_path = if is_adb_prefix {
                        format!("{}/{}", base_prefix, segments[0..start_idx].join("/"))
                    } else {
                        format!("/{}", segments[0..start_idx].join("/"))
                    };

                    ell_resp.context_menu(|ui| {
                        if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
                            state.open_in_new_tab(jump_path.clone());
                            ui.close_menu();
                        }
                    });

                    if ell_resp.clicked() {
                        any_seg_clicked = true;
                        state.navigate(jump_path);
                    }
                    if ell_resp.secondary_clicked() {
                        any_seg_clicked = true;
                    }
                    if ell_resp.double_clicked() {
                        any_seg_clicked = true;
                        wants_edit = true;
                    }
                    x += ell_w;
                }
            }

            // Görünür segmentleri çiz
            for i in start_idx..n {
                let seg = segments[i];
                let seg_w = seg_widths[i];

                // Separator ›
                if x + 10.0 < outer_rect.right() {
                    ui.painter().text(Pos2::new(x + 5.0, y_center), Align2::CENTER_CENTER,
                        "›", FontId::new(t.typography.size_sm, FontFamily::Proportional), c.text_disabled());
                    x += 10.0;
                }

                let is_last = i == n - 1;
                let actual_w = if x + seg_w > outer_rect.right() - 4.0 {
                    (outer_rect.right() - 4.0 - x).max(24.0)
                } else {
                    seg_w
                };

                let seg_rect = Rect::from_min_size(Pos2::new(x, outer_rect.top()), Vec2::new(actual_w, 28.0));
                let seg_resp = ui.interact(seg_rect, ui.id().with(format!("bc_{}", i)), Sense::click());

                // Prevent text overflow across segments by truncating with ellipsis
                let max_chars = ((actual_w - 12.0) / 7.2).floor().max(3.0) as usize;
                let display_seg = if seg.chars().count() > max_chars {
                    let cut: String = seg.chars().take(max_chars.saturating_sub(1)).collect();
                    format!("{}…", cut)
                } else {
                    seg.to_string()
                };

                let seg_pointer_over = ui.ctx().pointer_interact_pos()
                    .or_else(|| ui.ctx().pointer_hover_pos())
                    .map(|p| seg_rect.contains(p))
                    .unwrap_or(false);

                let nav_path = if is_adb_prefix {
                    format!("{}/{}", base_prefix, segments[0..=i].join("/"))
                } else {
                    format!("/{}", segments[0..=i].join("/"))
                };

                // Drag & Drop — breadcrumb segmentine bırak
                if state.drag.dragging && seg_pointer_over {
                    ui.painter().rect_filled(seg_rect.shrink(1.0), Rounding::same(s.radius_sm),
                        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 40));
                    ui.painter().rect_stroke(seg_rect.shrink(1.0), Rounding::same(s.radius_sm),
                        Stroke::new(1.5, c.accent()));

                    if ui.input(|i| i.pointer.any_released()) {
                        let sources = state.drag.sources.clone();
                        let copy = state.drag.copy_mode;
                        state.drag.cancel();
                        state.start_transfer(sources, nav_path.clone(), copy);
                    }
                } else if seg_resp.hovered() && !is_last {
                    ui.painter().rect_filled(seg_rect.shrink(2.0), Rounding::same(s.radius_sm), c.bg_surface1());
                }

                let color = if is_last {
                    c.text_primary()
                } else if seg_resp.hovered() || (state.drag.dragging && seg_pointer_over) {
                    c.accent()
                } else {
                    c.text_secondary()
                };

                ui.painter().text(
                    Pos2::new(x + actual_w / 2.0, y_center),
                    Align2::CENTER_CENTER,
                    &display_seg,
                    FontId::new(t.typography.size_sm, FontFamily::Proportional),
                    color,
                );

                seg_resp.context_menu(|ui| {
                    if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
                        state.open_in_new_tab(nav_path.clone());
                        ui.close_menu();
                    }
                });

                if seg_resp.clicked() {
                    any_seg_clicked = true;
                    if is_last {
                        wants_edit = true;
                    } else {
                        state.navigate(nav_path);
                    }
                }
                if seg_resp.secondary_clicked() {
                    any_seg_clicked = true;
                }
                if seg_resp.double_clicked() {
                    any_seg_clicked = true;
                    wants_edit = true;
                }

                x += actual_w;
                if x >= outer_rect.right() - 4.0 {
                    break;
                }
            }
        }

        // Eğer boş alana tıklandıysa veya çift tıklandıysa metin düzenleme moduna geç
        if (outer_resp.clicked() && !any_seg_clicked) || outer_resp.double_clicked() {
            wants_edit = true;
        }
    }
    (outer_resp, wants_edit)
}


/// Nav button — devre dışıyken soluk gösterilir, tıklanamaz
fn nav_btn(ui: &mut Ui, t: &AexTheme, label: &str, enabled: bool, on_click: impl FnOnce()) -> Response {
    let c = &t.colors;
    let color = if enabled { c.text_primary() } else { c.text_disabled() };

    let resp = icon_btn(ui, t, label, color);
    if enabled && resp.clicked() {
        on_click();
    }
    resp
}

fn icon_btn(ui: &mut Ui, t: &AexTheme, label: &str, color: Color32) -> Response {
    let size = Vec2::splat(28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if resp.is_pointer_button_down_on() {
            t.colors.bg_surface1()
        } else if resp.hovered() {
            t.colors.bg_surface0()
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, Rounding::same(t.shape.radius_sm), bg);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(t.icons.size_md, FontFamily::Monospace),
            color,
        );
    }
    resp
}

fn menu_action_item(
    ui: &mut Ui,
    t: &AexTheme,
    icon_str: &str,
    text: &str,
    checked: bool,
    show_check: bool,
) -> Response {
    let c = &t.colors;
    let s = &t.shape;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), c.bg_surface0());
    }
    let text_color = if checked { c.accent() } else { c.text_primary() };
    let icon_color = if checked { c.accent() } else { c.text_secondary() };

    let mut x = rect.left() + 6.0;
    let y = rect.center().y;

    if show_check {
        let check_str = if checked { "✓" } else { " " };
        ui.painter().text(
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            check_str,
            FontId::new(t.typography.size_sm, FontFamily::Monospace),
            c.accent(),
        );
        x += 16.0;
    }

    if !icon_str.is_empty() {
        ui.painter().text(
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            icon_str,
            FontId::new(t.icons.size_sm, FontFamily::Monospace),
            icon_color,
        );
        x += 20.0;
    }

    ui.painter().text(
        Pos2::new(x, y),
        Align2::LEFT_CENTER,
        text,
        FontId::new(t.typography.size_sm, FontFamily::Proportional),
        text_color,
    );

    resp
}

fn search_bar(ui: &mut Ui, t: &AexTheme, state: &mut AppState, width: f32) {
    let c = &t.colors;

    let buf_id = ui.id().with("search_buf");
    let mut buf: String = ui.ctx().data_mut(|d|
        d.get_temp(buf_id).unwrap_or_else(|| state.search.query.clone())
    );

    let edit_w = (width - 28.0).max(60.0);
    let resp = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .hint_text(format!("{} Search…", super::icon(t, "search")))
            .desired_width(edit_w)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional))
            .text_color(c.text_primary())
    );
    if state.search.request_focus {
        resp.request_focus();
        state.search.request_focus = false;
    }
    state.search.is_focused = resp.has_focus();

    if resp.changed() {
        // Debounce — fire pending search after 280ms (handled in app.rs)
        state.pending_search_query = Some(buf.clone());
        state.pending_search_changed = Some(std::time::Instant::now());
    }

    ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));

    if ui.small_button("✕").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
        state.search.clear();
        state.needs_reload = true;
        ui.ctx().data_mut(|d| {
            d.remove::<String>(buf_id);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::defaults::default_theme;

    #[test]
    fn test_compact_icon_distinct_from_dual_pane() {
        let theme = default_theme();
        let compact_icon = super::super::icon(&theme, "compact");
        let grid_icon = super::super::icon(&theme, "grid");
        let list_icon = super::super::icon(&theme, "list");
        let settings_icon = super::super::icon(&theme, "settings");

        assert_eq!(compact_icon, "\u{f00b}", "Compact view should use th-list Nerd Font glyph");
        assert_ne!(compact_icon, "\u{f0db}", "Compact view must NOT share icon with dual pane (fa-columns)");
        assert_eq!(grid_icon, "\u{f00a}");
        assert_eq!(list_icon, "\u{f03a}");
        assert_eq!(settings_icon, "\u{f013}");
    }

    #[test]
    fn test_toolbar_reserve_width_prevents_overlap() {
        // Test responsive calculations
        let total_avail_w = 700.0_f32;
        let show_view_modes = total_avail_w >= 640.0;
        let show_dual_pane = total_avail_w >= 540.0;
        let show_hidden_toggle = total_avail_w >= 460.0;
        let show_preview = total_avail_w >= 390.0;
        let show_search_btn = total_avail_w >= 320.0;

        assert!(show_view_modes);
        assert!(show_dual_pane);
        assert!(show_hidden_toggle);
        assert!(show_preview);
        assert!(show_search_btn);

        let mut right_cluster_w = 32.0;
        if show_view_modes { right_cluster_w += 100.0; }
        if show_hidden_toggle { right_cluster_w += 32.0; }
        if show_dual_pane { right_cluster_w += 32.0; }
        if show_preview { right_cluster_w += 32.0; }
        right_cluster_w += 12.0;

        // With search button:
        let addr_w = (total_avail_w - right_cluster_w - 36.0).max(80.0);
        let used_left = addr_w + 36.0;
        let remaining_for_right = total_avail_w - used_left;
        assert!(remaining_for_right >= right_cluster_w,
            "Remaining width ({}) must accommodate right cluster ({})", remaining_for_right, right_cluster_w);

        // When search is active:
        let avail_for_nav = (total_avail_w - right_cluster_w - 4.0).max(180.0);
        let search_w = (avail_for_nav * 0.45).clamp(130.0, 240.0);
        let search_addr_w = (avail_for_nav - search_w - 4.0).max(60.0);
        let total_nav_used = search_addr_w + 4.0 + search_w;
        assert!(total_avail_w - total_nav_used >= right_cluster_w,
            "When search is active, right cluster space must be preserved without collision");
    }

    #[test]
    fn test_view_modes_and_media_grid_style_state() {
        let mut state = AppState::new();
        state.nav.current_path = "/home/aiuser".to_string();

        state.set_view_mode_for_current_dir(ViewMode::Grid);
        assert_eq!(state.view_mode, ViewMode::Grid);

        state.set_view_mode_for_current_dir(ViewMode::List);
        assert_eq!(state.view_mode, ViewMode::List);

        state.set_view_mode_for_current_dir(ViewMode::Compact);
        assert_eq!(state.view_mode, ViewMode::Compact);

        state.set_media_grid_style(MediaGridStyle::NemoGrid);
        assert_eq!(state.media_grid_style, MediaGridStyle::NemoGrid);

        state.set_media_grid_style(MediaGridStyle::Masonry);
        assert_eq!(state.media_grid_style, MediaGridStyle::Masonry);
    }

    #[test]
    fn test_breadcrumb_open_in_new_tab() {
        let mut state = AppState::new();
        state.nav.current_path = "/home/aiuser/Documents".to_string();
        let initial_tabs = state.tabs.len();

        // 1. Root context menu action
        let root_target = "/".to_string();
        state.open_in_new_tab(root_target.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 1);
        assert_eq!(state.nav.current_path, "/");

        // 2. Segment context menu action
        let seg_target = "/home/aiuser".to_string();
        state.open_in_new_tab(seg_target.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 2);
        assert_eq!(state.nav.current_path, "/home/aiuser");

        // 3. Ellipsis jump context menu action
        let jump_target = "/home".to_string();
        state.open_in_new_tab(jump_target.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 3);
        assert_eq!(state.nav.current_path, "/home");
    }
}
