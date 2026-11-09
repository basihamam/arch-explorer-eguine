// src/ui/sidebar.rs
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use crate::backend::fs_ops::Bookmark;
use super::{icon, human_size};

pub fn render(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    tx: &std::sync::mpsc::Sender<crate::app::BackendMsg>,
    rt: &tokio::runtime::Runtime,
) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    // Sidebar arka planı
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(
        rect,
        Rounding::ZERO,
        if t.effects.glass_panels {
            Color32::from_rgba_unmultiplied(
                c.bg_mantle().r(), c.bg_mantle().g(), c.bg_mantle().b(),
                (t.effects.sidebar_opacity * 255.0) as u8,
            )
        } else {
            c.bg_mantle()
        },
    );

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.add_space(sp.sm);

            // ── Bookmarks ─────────────────────────────────────────────────
            if t.sidebar.show_bookmarks && !state.sidebar.bookmarks.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(t.spacing.md);
                    ui.label(
                        RichText::new("BOOKMARKS")
                            .color(c.text_hint())
                            .size(t.typography.size_xs)
                            .strong()
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("+").color(c.text_hint()).size(13.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).on_hover_text("Mevcut klasörü yer imlerine ekle").clicked() {
                            let cur_path = state.active_context_path();
                            let name = std::path::Path::new(&cur_path)
                                .file_name().map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| cur_path.clone());
                            let _ = crate::backend::fs_ops::add_bookmark_full(crate::backend::fs_ops::Bookmark {
                                path: cur_path.clone(),
                                name,
                                icon: "folder".to_string(),
                                pinned: false,
                                order: 99,
                                color: String::new(),
                            });
                            state.sidebar.bookmarks = crate::backend::fs_ops::get_bookmarks();
                            state.notify.success("Yer imlerine eklendi".into());
                        }
                    });
                });
                ui.add_space(sp.xs);

                let bms = state.sidebar.bookmarks.clone();
                for bm in &bms {
                    bookmark_item(ui, t, state, bm);
                }

                ui.add_space(sp.md);
            }

            // ── Devices ───────────────────────────────────────────────────
            if t.sidebar.show_drives {
                let dev_count = state.sidebar.block_devices.len();
                ui.horizontal(|ui| {
                    ui.add_space(t.spacing.md);
                    ui.label(
                        RichText::new("DEVICES")
                            .color(c.text_hint())
                            .size(t.typography.size_xs)
                            .strong()
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let is_dev_open = state.panels.active == crate::state::ActivePanel::DevicesView;
                        let icon_color = if is_dev_open { c.accent() } else { c.text_hint() };
                        if ui.add(egui::Button::new(RichText::new("◉").color(icon_color).size(12.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                            state.panels.open_devices_view();
                        }
                    });
                });
                ui.add_space(sp.xs);

                // Devices View shortcut entry ("Bu Bilgisayar")
                let is_dev_active = state.panels.active == crate::state::ActivePanel::DevicesView;
                let resp = sidebar_row(ui, t, is_dev_active);
                let rect = resp.rect;
                ui.painter().text(
                    rect.min + Vec2::new(sp.md + 2.0, rect.height() / 2.0),
                    Align2::LEFT_CENTER,
                    "◉",
                    FontId::new(t.icons.size_sm, FontFamily::Monospace),
                    if is_dev_active { c.accent() } else { c.text_secondary() },
                );
                ui.painter().text(
                    rect.min + Vec2::new(sp.md + 22.0, rect.height() / 2.0),
                    Align2::LEFT_CENTER,
                    "Devices",
                    FontId::new(t.typography.size_sm, FontFamily::Proportional),
                    if is_dev_active { c.text_primary() } else { c.text_secondary() },
                );
                if resp.clicked() {
                    state.panels.open_devices_view();
                }

                ui.add_space(2.0);

                let mut all_devices = state.sidebar.block_devices.clone();
                all_devices.sort_by(|a, b| {
                    let a_root = a.mount_point.as_deref() == Some("/");
                    let b_root = b.mount_point.as_deref() == Some("/");
                    if a_root != b_root {
                        return if a_root { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
                    }
                    if a.is_mounted != b.is_mounted {
                        return if a.is_mounted { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
                    }
                    a.label.cmp(&b.label)
                });
                let visible_devices: Vec<_> = if state.sidebar.devices_expanded || all_devices.len() <= 5 {
                    all_devices.clone()
                } else {
                    all_devices.iter().take(5).cloned().collect()
                };

                // ADB Android Cihazları
                let adb_devs = state.sidebar.adb_devices.clone();
                for adb_dev in &adb_devs {
                    adb_sidebar_item(ui, t, state, adb_dev);
                }

                for dev in &visible_devices {
                    device_sidebar_item(ui, t, state, dev);
                }

                if all_devices.len() > 5 {
                    let text = if state.sidebar.devices_expanded {
                        "▲ Daha az".to_string()
                    } else {
                        format!("▼ +{} daha", all_devices.len() - 5)
                    };
                    let expand_btn = ui.add_sized(
                        Vec2::new(ui.available_width() - t.spacing.sm * 2.0, 20.0),
                        egui::Button::new(RichText::new(text).color(c.text_hint()).size(10.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    );
                    if expand_btn.clicked() {
                        state.sidebar.devices_expanded = !state.sidebar.devices_expanded;
                    }
                }

                ui.add_space(sp.md);
            }

            // ── Quick Actions ─────────────────────────────────────────────
            section_header(ui, t, "QUICK");
            ui.add_space(sp.xs);

            quick_item(ui, t, state, icon(t, "drive"), "Devices", None, |s| {
                s.panels.open_devices_view();
            });
            let home_dir = dirs::home_dir().map(|h| h.to_string_lossy().to_string());
            quick_item(ui, t, state, icon(t, "home"), "Home", home_dir.clone(), move |s| {
                if let Some(h) = home_dir {
                    s.navigate(h);
                }
            });
            let trash_dir = crate::backend::fs_ops::resolve_trash_dir().to_string_lossy().to_string();
            quick_item(ui, t, state, icon(t, "trash"), "Trash", Some(trash_dir.clone()), move |s| {
                s.navigate(trash_dir);
            });
            quick_item(ui, t, state, icon(t, "terminal"), "Terminal", None, |s| {
                let cur = s.active_context_path();
                s.open_terminal(Some(cur));
            });

            // Sidebar sağ border
            let rect = ui.min_rect();
            ui.painter().vline(
                rect.right(),
                rect.top()..=rect.bottom(),
                Stroke::new(1.0_f32, c.border()),
            );
        });
}

// ── Section header ─────────────────────────────────────────────────────────

fn section_header(ui: &mut Ui, t: &AexTheme, label: &str) {
    let c = &t.colors;
    ui.horizontal(|ui| {
        ui.add_space(t.spacing.md);
        ui.label(
            RichText::new(label)
                .color(c.text_hint())
                .size(t.typography.size_xs)
                .strong()
        );
    });
}

// ── Bookmark item ──────────────────────────────────────────────────────────

fn bookmark_item(ui: &mut Ui, t: &AexTheme, state: &mut AppState, bm: &Bookmark) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let bm_path  = bm.path.clone();
    let bm_name  = bm.name.clone();
    let bm_color = bm.color.clone();
    let bm_icon  = bm.icon.clone();

    let resp = ui.add_sized(
        Vec2::new(ui.available_width(), 28.0),
        egui::Button::new("") // invisible button for full-width click
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
    );

    let is_active = state.nav.current_path == bm.path;
    let item_rect = resp.rect;
    let pointer_over = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .map(|p| item_rect.contains(p))
        .unwrap_or(false);
    let is_drag_over = state.drag.dragging && pointer_over;

    // Smooth hover animation
    let bm_hov_id = Id::new(format!("bm_hov_{}", bm.path));
    let bm_hov_factor = ui.ctx().animate_bool(bm_hov_id, resp.hovered() && !is_active && !is_drag_over);

    let bg = if is_active {
        Color32::from_rgba_unmultiplied(
            c.bg_surface2().r(), c.bg_surface2().g(), c.bg_surface2().b(),
            (t.effects.card_opacity * 255.0) as u8,
        )
    } else if is_drag_over {
        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 45)
    } else if bm_hov_factor > 0.001 {
        let alpha = (255.0 * bm_hov_factor) as u8;
        Color32::from_rgba_unmultiplied(c.bg_surface0().r(), c.bg_surface0().g(), c.bg_surface0().b(), alpha)
    } else {
        Color32::TRANSPARENT
    };

    ui.painter().rect_filled(item_rect, Rounding::same(s.radius_sm), bg);

    if is_active {
        // Sol indicator çizgi + subtle border
        ui.painter().rect_filled(
            Rect::from_min_size(
                item_rect.min,
                Vec2::new(2.0, item_rect.height()),
            ),
            Rounding::same(1.0),
            c.accent(),
        );
        ui.painter().rect_stroke(
            item_rect,
            Rounding::same(s.radius_sm),
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 120)),
        );
    }

    // Drag-over border
    if is_drag_over {
        ui.painter().rect_stroke(item_rect, Rounding::same(s.radius_sm), Stroke::new(1.5_f32, c.accent()));
    }

    // Icon
    let icon_str = sidebar_icon(t, &bm_icon);
    let icon_color = if bm_color.is_empty() {
        if is_active || is_drag_over { c.accent() } else { c.text_secondary() }
    } else {
        crate::theme::types::hex_to_color32(&bm_color)
    };

    ui.painter().text(
        item_rect.min + Vec2::new(sp.md + 2.0, item_rect.height() / 2.0),
        Align2::LEFT_CENTER,
        icon_str,
        FontId::new(t.icons.size_sm, FontFamily::Monospace),
        icon_color,
    );

    // Label
    let text_color = if is_active || is_drag_over { c.text_primary() } else { c.text_secondary() };
    ui.painter().text(
        item_rect.min + Vec2::new(sp.md + 22.0, item_rect.height() / 2.0),
        Align2::LEFT_CENTER,
        &bm_name,
        FontId::new(t.typography.size_sm, FontFamily::Proportional),
        text_color,
    );

    if resp.clicked() {
        state.panels.close();
        if bm_name.to_lowercase() == "terminal" || bm_icon == "terminal" {
            let cur = state.active_context_path();
            state.open_terminal(Some(cur));
        } else {
            state.navigate(bm_path.clone());
        }
    }

    // Drag & Drop — dosya bu bookmark'a bırakıldı mı?
    if is_drag_over && ui.input(|i| i.pointer.any_released()) {
        let dest = bm.path.clone();
        let sources = state.drag.sources.clone();
        let copy = state.drag.copy_mode;
        state.drag.cancel();
        state.start_transfer(sources, dest, copy);
    }

    // Context menu
    resp.context_menu(|ui| {
        if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
            state.panels.close();
            state.open_in_new_tab(bm_path.clone());
            ui.close_menu();
        }
        if ui.button("Open").clicked() {
            state.panels.close();
            state.navigate(bm_path.clone());
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Remove Bookmark").clicked() {
            crate::backend::fs_ops::remove_bookmark(bm_path).ok();
            state.sidebar.bookmarks = crate::backend::fs_ops::get_bookmarks();
            ui.close_menu();
        }
    });
}

// ── Device item (mounted or unmounted) matching screenshot ───────────────────

fn device_sidebar_item(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    dev: &crate::backend::mounts::BlockDevice,
) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let is_active = dev.is_mounted
        && dev.mount_point.as_ref().map(|mp| state.nav.current_path.starts_with(mp)).unwrap_or(false);

    // Eşleşen DriveInfo bul (doluluk oranı için)
    let drive_info = dev.mount_point.as_ref().and_then(|mp| {
        state.sidebar.drives.iter().find(|d| &d.mount_point == mp || &d.device == &dev.device)
    });

    let w = ui.available_width() - t.spacing.sm * 2.0;
    let has_usage = drive_info.is_some() && dev.is_mounted;
    let h = if has_usage { 38.0_f32 } else { 34.0_f32 };

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if is_active {
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 25)
        } else if resp.hovered() {
            c.bg_surface0()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);

        if is_active {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                Rounding::same(1.0),
                c.accent(),
            );
        }

        // Disk icon
        let is_ntfs = dev.is_windows || dev.fs_type.to_lowercase().contains("ntfs");
        let dev_icon = if is_ntfs {
            icon(t, "ntfs")
        } else if dev.mount_point.as_deref() == Some("/") {
            icon(t, "server")
        } else if dev.is_mounted {
            icon(t, "drive")
        } else {
            "○"
        };

        let icon_color = if is_active {
            c.accent()
        } else if dev.is_mounted {
            c.text_secondary()
        } else {
            c.text_disabled()
        };

        ui.painter().text(
            rect.min + Vec2::new(sp.md + 2.0, if has_usage { 15.0 } else { rect.height() / 2.0 }),
            Align2::LEFT_CENTER,
            dev_icon,
            FontId::new(if dev.is_windows { 13.0 } else { t.icons.size_sm }, FontFamily::Monospace),
            icon_color,
        );

        // Label (name)
        let display_name = if !dev.label.is_empty() && dev.label != dev.name {
            &dev.label
        } else {
            &dev.name
        };

        let name_color = if is_active {
            c.text_primary()
        } else if dev.is_mounted {
            c.text_secondary()
        } else {
            c.text_hint()
        };

        ui.painter().text(
            rect.min + Vec2::new(sp.md + 22.0, 9.0),
            Align2::LEFT_CENTER,
            display_name,
            FontId::new(t.typography.size_sm, FontFamily::Proportional),
            name_color,
        );

        // Subtitle (size · fstype)
        let fs_str = if dev.fs_type.is_empty() { "?" } else { &dev.fs_type };
        let sub_text = if dev.is_mounted {
            format!("{} · {}", dev.size, fs_str)
        } else {
            format!("{} · {} · click to mount", dev.size, fs_str)
        };

        ui.painter().text(
            rect.min + Vec2::new(sp.md + 22.0, 22.0),
            Align2::LEFT_CENTER,
            &sub_text,
            FontId::new(9.0, FontFamily::Proportional),
            c.text_disabled(),
        );

        // Usage bar (eğer mounted ise ve drive_info varsa)
        if let Some(d) = drive_info {
            let bar_y = rect.min.y + 32.0;
            let bar_w = (rect.width() - (sp.md + 24.0)).max(30.0);
            let bar_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + sp.md + 22.0, bar_y),
                Vec2::new(bar_w, 2.5),
            );
            let used_frac = if d.total > 0 { (d.used as f32 / d.total as f32).clamp(0.0, 1.0) } else { 0.0 };
            let bar_color = if used_frac > 0.90 {
                c.danger()
            } else if used_frac > 0.75 {
                c.warning()
            } else {
                c.accent()
            };

            ui.painter().rect_filled(bar_rect, Rounding::same(1.5), c.bg_surface1());
            if used_frac > 0.0 {
                let fill_w = bar_rect.width() * used_frac;
                ui.painter().rect_filled(
                    Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, bar_rect.height())),
                    Rounding::same(1.5),
                    bar_color,
                );
            }
        }
    }

    if resp.clicked() {
        if dev.is_mounted {
            if let Some(ref mp) = dev.mount_point {
                state.panels.close();
                state.navigate(mp.clone());
            }
        } else {
            state.panels.open_mount();
        }
    }

    if dev.is_mounted {
        if let Some(ref mp) = dev.mount_point {
            let mp_path = mp.clone();
            resp.context_menu(|ui| {
                if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
                    state.panels.close();
                    state.open_in_new_tab(mp_path.clone());
                    ui.close_menu();
                }
                if ui.button("Open").clicked() {
                    state.panels.close();
                    state.navigate(mp_path.clone());
                    ui.close_menu();
                }
            });
        }
    }

    // Drop files onto mounted block device
    let pointer_over_dev = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .map(|p| rect.contains(p))
        .unwrap_or(false);

    if dev.is_mounted && state.drag.dragging && pointer_over_dev {
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm),
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 40));
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(1.5, c.accent()));
        if ui.input(|i| i.pointer.any_released()) {
            if let Some(ref mp) = dev.mount_point {
                let dest = mp.clone();
                let sources = state.drag.sources.clone();
                let copy = state.drag.copy_mode;
                state.drag.cancel();
                state.start_transfer(sources, dest, copy);
            }
        }
    }
}

// ── ADB Device item ──────────────────────────────────────────────────────────

fn adb_sidebar_item(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    adb_dev: &crate::backend::adb::AdbDevice,
) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let target_path = crate::backend::adb::make_adb_path(&adb_dev.serial, "/sdcard");
    let is_active = state.nav.current_path.starts_with(&format!("adb://{}", adb_dev.serial));

    let w = ui.available_width() - t.spacing.sm * 2.0;
    let has_usage = adb_dev.is_authorized && adb_dev.total_storage > 0;
    let h = if has_usage { 38.0_f32 } else { 34.0_f32 };

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if is_active {
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 25)
        } else if resp.hovered() {
            c.bg_surface0()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);

        if is_active {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                Rounding::same(1.0),
                c.accent(),
            );
        }

        // Android / Telefon ikonu
        let dev_icon = icon(t, "phone");
        let icon_color = if is_active {
            c.accent()
        } else if adb_dev.is_authorized {
            Color32::from_rgb(164, 198, 57) // Android green
        } else {
            c.warning()
        };

        ui.painter().text(
            rect.min + Vec2::new(sp.md + 2.0, if has_usage { 15.0 } else { rect.height() / 2.0 }),
            Align2::LEFT_CENTER,
            dev_icon,
            FontId::new(t.icons.size_sm + 1.0, FontFamily::Monospace),
            icon_color,
        );

        // Display Name
        let display_name = adb_dev.display_name();
        let name_color = if is_active {
            c.text_primary()
        } else if adb_dev.is_authorized {
            c.text_secondary()
        } else {
            c.text_hint()
        };

        ui.painter().text(
            rect.min + Vec2::new(sp.md + 22.0, 9.0),
            Align2::LEFT_CENTER,
            &display_name,
            FontId::new(t.typography.size_sm, FontFamily::Proportional),
            name_color,
        );

        // Subtitle (Depolama veya Yetki durumu)
        let sub_text = if adb_dev.is_authorized {
            if adb_dev.total_storage > 0 {
                let free_bytes = adb_dev.total_storage.saturating_sub(adb_dev.used_storage);
                format!("{} free · ADB", human_size(free_bytes))
            } else {
                "Connected · ADB".to_string()
            }
        } else {
            format!("{} (check phone)", adb_dev.state)
        };

        ui.painter().text(
            rect.min + Vec2::new(sp.md + 22.0, 22.0),
            Align2::LEFT_CENTER,
            &sub_text,
            FontId::new(9.0, FontFamily::Proportional),
            if adb_dev.is_authorized { c.text_disabled() } else { c.warning() },
        );

        // Usage bar (eğer yetkili ve alan bilgisi varsa)
        if has_usage {
            let bar_y = rect.min.y + 32.0;
            let bar_w = (rect.width() - (sp.md + 24.0)).max(30.0);
            let bar_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + sp.md + 22.0, bar_y),
                Vec2::new(bar_w, 2.5),
            );
            let used_frac = (adb_dev.used_storage as f32 / adb_dev.total_storage as f32).clamp(0.0, 1.0);
            let bar_color = if used_frac > 0.90 {
                c.danger()
            } else if used_frac > 0.75 {
                c.warning()
            } else {
                Color32::from_rgb(164, 198, 57)
            };

            ui.painter().rect_filled(bar_rect, Rounding::same(1.5), c.bg_surface1());
            if used_frac > 0.0 {
                let fill_w = bar_rect.width() * used_frac;
                ui.painter().rect_filled(
                    Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, bar_rect.height())),
                    Rounding::same(1.5),
                    bar_color,
                );
            }
        }
    }

    if resp.clicked() {
        if adb_dev.is_authorized {
            state.panels.close();
            state.navigate(target_path.clone());
        } else {
            state.notify.warning("Lütfen telefondan USB Hata Ayıklama iznini onaylayın!".to_string());
        }
    }

    let adb_new_tab_path = format!("adb://{}/sdcard", adb_dev.serial);
    resp.context_menu(|ui| {
        if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
            if adb_dev.is_authorized {
                state.panels.close();
                state.open_in_new_tab(adb_new_tab_path.clone());
            } else {
                state.notify.warning("Lütfen telefondan USB Hata Ayıklama iznini onaylayın!".to_string());
            }
            ui.close_menu();
        }
        if ui.button("Open").clicked() {
            if adb_dev.is_authorized {
                state.panels.close();
                state.navigate(adb_new_tab_path.clone());
            } else {
                state.notify.warning("Lütfen telefondan USB Hata Ayıklama iznini onaylayın!".to_string());
            }
            ui.close_menu();
        }
    });

    // Drop files onto phone in sidebar (adb push!)
    let pointer_over_adb = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .map(|p| rect.contains(p))
        .unwrap_or(false);

    if adb_dev.is_authorized && state.drag.dragging && pointer_over_adb {
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm),
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 40));
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(1.5, c.accent()));
        if ui.input(|i| i.pointer.any_released()) {
            let dest = target_path.clone();
            let sources = state.drag.sources.clone();
            let copy = state.drag.copy_mode;
            state.drag.cancel();
            state.start_transfer(sources, dest, copy);
        }
    }
}

// ── Quick item ─────────────────────────────────────────────────────────────

fn quick_item(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    icon_str: &str,
    label: &str,
    drop_target: Option<String>,
    on_click: impl FnOnce(&mut AppState),
) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let resp = sidebar_row(ui, t, false);
    let rect = resp.rect;

    let is_dragging = state.drag.dragging;
    let pointer_over_quick = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .map(|p| rect.contains(p))
        .unwrap_or(false);
    let is_drag_over = is_dragging && drop_target.is_some() && pointer_over_quick;

    if is_drag_over {
        ui.painter().rect_filled(
            rect,
            Rounding::same(s.radius_sm),
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 45),
        );
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(1.5, c.accent()));
    }

    ui.painter().text(
        rect.min + Vec2::new(sp.md + 2.0, rect.height() / 2.0),
        Align2::LEFT_CENTER,
        icon_str,
        FontId::new(t.icons.size_sm, FontFamily::Monospace),
        if is_drag_over { c.accent() } else { c.text_secondary() },
    );
    ui.painter().text(
        rect.min + Vec2::new(sp.md + 22.0, rect.height() / 2.0),
        Align2::LEFT_CENTER,
        label,
        FontId::new(t.typography.size_sm, FontFamily::Proportional),
        if is_drag_over { c.text_primary() } else { c.text_secondary() },
    );

    if resp.clicked() { on_click(state); }

    if let Some(ref target_path) = drop_target {
        let tp = target_path.clone();
        resp.context_menu(|ui| {
            if ui.button("Yeni Sekmede Aç / Open in New Tab").clicked() {
                state.panels.close();
                state.open_in_new_tab(tp.clone());
                ui.close_menu();
            }
            if ui.button("Open").clicked() {
                state.panels.close();
                state.navigate(tp.clone());
                ui.close_menu();
            }
        });
    }

    // Drop files onto quick item (e.g. Home, Trash)
    if is_drag_over && ui.input(|i| i.pointer.any_released()) {
        if let Some(dest) = drop_target {
            let is_trash = dest == "trash:" || dest == "trash:///"
                || dest == crate::backend::fs_ops::resolve_trash_dir().to_string_lossy().as_ref()
                || dest.contains("/.local/share/Trash");
            if is_trash {
                let paths = state.drag.sources.clone();
                state.drag.cancel();
                for p in paths {
                    crate::backend::fs_ops::delete_item(p, false).ok();
                }
                state.refresh();
            } else {
                let sources = state.drag.sources.clone();
                let copy = state.drag.copy_mode;
                state.drag.cancel();
                state.start_transfer(sources, dest, copy);
            }
        }
    }
}

// ── Shared row widget ──────────────────────────────────────────────────────

fn sidebar_row(ui: &mut Ui, t: &AexTheme, is_active: bool) -> Response {
    let c = &t.colors;
    let s = &t.shape;
    let w = ui.available_width() - t.spacing.sm * 2.0;
    let h = t.layout.item_height;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if is_active {
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 25)
        } else if resp.hovered() {
            c.bg_surface0()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);

        if is_active {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                Rounding::same(1.0),
                c.accent(),
            );
        }
    }

    resp
}

// ── Icon mapper ───────────────────────────────────────────────────────────

fn sidebar_icon<'a>(t: &AexTheme, key: &str) -> &'static str {
    match key {
        "home"      => icon(t, "home"),
        "downloads" => icon(t, "downloads"),
        "documents" => icon(t, "documents"),
        "pictures"  => icon(t, "pictures"),
        "music"     => icon(t, "music"),
        "videos"    => icon(t, "videos"),
        "desktop"   => icon(t, "desktop"),
        "hdd"       => icon(t, "drive"),
        "server"    => icon(t, "server"),
        "star"      => icon(t, "star"),
        _           => icon(t, "folder"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_open_in_new_tab() {
        let mut state = AppState::new();
        state.nav.current_path = "/home/aiuser".to_string();
        let initial_tabs = state.tabs.len();

        // 1. Bookmark open in new tab
        let bm_path = "/home/aiuser/Downloads".to_string();
        state.open_in_new_tab(bm_path.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 1);
        assert_eq!(state.nav.current_path, bm_path);

        // 2. Quick item open in new tab
        let quick_target = "/home/aiuser/Documents".to_string();
        state.open_in_new_tab(quick_target.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 2);
        assert_eq!(state.nav.current_path, quick_target);

        // 3. Mounted device open in new tab
        let mount_point = "/media/usb".to_string();
        state.open_in_new_tab(mount_point.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 3);
        assert_eq!(state.nav.current_path, mount_point);

        // 4. ADB device open in new tab
        let adb_path = "adb://device123/sdcard".to_string();
        state.open_in_new_tab(adb_path.clone());
        assert_eq!(state.tabs.len(), initial_tabs + 4);
        assert_eq!(state.nav.current_path, adb_path);
    }
}
