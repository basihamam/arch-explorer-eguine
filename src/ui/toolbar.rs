// ─────────────────────────────────────────────────────────────────────────────

use egui::*;
use crate::theme::types::AexTheme;
use crate::state::{AppState, ViewMode};
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

            let icon_area_w = t.layout.toolbar_height * 4.5; // Kalan butonlar için temiz alan
            let addr_width = if state.search.active {
                (ui.available_width() * 0.5).max(80.0)
            } else {
                (ui.available_width() - icon_area_w).max(80.0)
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
            if state.search.active {
                search_bar(ui, t, state);
            } else {
                if nav_btn(ui, t, icon(t, "search"), true, || {}).on_hover_text("Search (Ctrl+F)").clicked() {
                    state.search.active = true;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // ── Preview Toggle (F3) ───────────────────────────────────
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

                // ── Dual Pane Toggle (Ctrl+P) ─────────────────────────────
                let dp_color = if state.dual_pane { c.accent() } else { c.text_hint() };
                let dp_resp  = icon_btn(ui, t, "\u{f0db}", dp_color).on_hover_text("Dual Pane (Ctrl+P)");
                if dp_resp.clicked() {
                    if state.dual_pane { state.close_dual_pane(); }
                    else               { state.open_dual_pane(); }
                }

                // ── Hidden toggle ─────────────────────────────────────────
                let hidden_color = if state.dir.show_hidden { c.accent() } else { c.text_hint() };
                if icon_btn(ui, t, icon(t, if state.dir.show_hidden { "visible" } else { "hidden" }), hidden_color)
                    .on_hover_text(if state.dir.show_hidden { "Hide Hidden Files (Ctrl+H)" } else { "Show Hidden Files (Ctrl+H)" })
                    .clicked()
                {
                    state.dir.show_hidden = !state.dir.show_hidden;
                    state.refresh();
                }

                ui.add_space(sp.sm);

                // ── View Mode Toggle ──────────────────────────────────────
                let grid_color = if state.view_mode == ViewMode::Grid {
                    c.accent()
                } else {
                    c.text_hint()
                };
                let list_color = if state.view_mode == ViewMode::List {
                    c.accent()
                } else {
                    c.text_hint()
                };

                if icon_btn(ui, t, icon(t, "list"), list_color).on_hover_text("List View (Ctrl+2)").clicked() {
                    state.set_view_mode_for_current_dir(ViewMode::List);
                }
                if icon_btn(ui, t, icon(t, "grid"), grid_color).on_hover_text("Grid View (Ctrl+1)").clicked() {
                    state.set_view_mode_for_current_dir(ViewMode::Grid);
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

                    if ell_resp.clicked() {
                        any_seg_clicked = true;
                        let jump_path = if is_adb_prefix {
                            format!("{}/{}", base_prefix, segments[0..start_idx].join("/"))
                        } else {
                            format!("/{}", segments[0..start_idx].join("/"))
                        };
                        state.navigate(jump_path);
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

                if seg_resp.clicked() {
                    any_seg_clicked = true;
                    if is_last {
                        wants_edit = true;
                    } else {
                        state.navigate(nav_path);
                    }
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

fn search_bar(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let c = &t.colors;

    let buf_id = ui.id().with("search_buf");
    let mut buf: String = ui.ctx().data_mut(|d|
        d.get_temp(buf_id).unwrap_or_else(|| state.search.query.clone())
    );

    let resp = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .hint_text(format!("{} Search…", super::icon(t, "search")))
            .desired_width(220.0)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional))
            .text_color(c.text_primary())
    );
    resp.request_focus();

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
