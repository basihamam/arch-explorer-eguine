// src/ui/devices_view.rs
// DevicesView — "Bu Bilgisayar" ekranı, disk kartları büyük görünümde
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use crate::backend::fs_ops::DriveInfo;
use super::{icon, human_size};

use std::sync::mpsc::Sender;
use crate::app::BackendMsg;

pub fn render(ui: &mut Ui, t: &AexTheme, state: &mut AppState, tx: &Sender<BackendMsg>) {
    let c  = &t.colors;
    let sp = &t.spacing;

    // Başlık
    ui.add_space(sp.lg);
    ui.horizontal(|ui| {
        ui.add_space(sp.xl);
        ui.label(
            RichText::new(format!("{} Devices", icon(t, "drive")))
                .color(c.text_primary())
                .size(t.typography.size_xl)
                .strong()
        );
    });
    ui.add_space(sp.md);

    let mut drives = state.sidebar.drives.clone();
    // Sort mounted drives: Root "/" first, then by mount_point / name
    drives.sort_by(|a, b| {
        if a.mount_point == "/" {
            std::cmp::Ordering::Less
        } else if b.mount_point == "/" {
            std::cmp::Ordering::Greater
        } else {
            a.mount_point.cmp(&b.mount_point)
        }
    });

    let mut blocks = state.sidebar.block_devices.clone();
    blocks.sort_by(|a, b| {
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

    let adb_devices = state.sidebar.adb_devices.clone();

    if drives.is_empty() && blocks.is_empty() && adb_devices.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.label(RichText::new(icon(t, "drive")).color(c.text_disabled()).size(48.0));
            ui.add_space(sp.md);
            ui.label(RichText::new("No drives or devices detected").color(c.text_hint()).size(t.typography.size_base));
        });
        return;
    }

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        egui::Frame {
            inner_margin: Margin::symmetric(sp.xl, sp.xs),
            ..Default::default()
        }.show(ui, |ui| {
            // ── Android / ADB Devices ─────────────────────────────────────────
            if !adb_devices.is_empty() {
                section_label(ui, t, "Android & Portable Devices");
                ui.add_space(sp.sm);

                let avail_w = ui.available_width();
                let card_w  = ((avail_w - sp.md * 2.0) / 3.0).max(180.0);
                let card_h  = 120.0_f32;

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(sp.md);
                    for adb_dev in &adb_devices {
                        adb_card(ui, t, state, adb_dev, card_w, card_h);
                    }
                });
                ui.add_space(sp.lg);
            }

            // ── Mounted Drives ────────────────────────────────────────────────
            if !drives.is_empty() {
                section_label(ui, t, "Mounted");
                ui.add_space(sp.sm);

                // Grid layout: 3 kart yan yana
                let avail_w = ui.available_width();
                let card_w  = ((avail_w - sp.md * 2.0) / 3.0).max(180.0);
                let card_h  = 120.0_f32;

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(sp.md);
                    for drive in &drives {
                        drive_card(ui, t, state, drive, card_w, card_h, tx);
                    }
                });
                ui.add_space(sp.lg);
            }

            // ── Unmounted Devices ─────────────────────────────────────────────
            let unmounted: Vec<_> = blocks.iter().filter(|b| !b.is_mounted).collect();
            if !unmounted.is_empty() {
                section_label(ui, t, "Unmounted");
                ui.add_space(sp.sm);

                let avail_w = ui.available_width();
                let card_w  = ((avail_w - sp.md * 2.0) / 3.0).max(180.0);
                let card_h  = 100.0_f32;

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(sp.md);
                    for dev in &unmounted {
                        unmounted_card(ui, t, state, dev, card_w, card_h);
                    }
                });
            }

            ui.add_space(sp.xl);
        });
    });
}

// ── Drive card (mounted) ──────────────────────────────────────────────────────

fn drive_card(
    ui: &mut Ui, t: &AexTheme, state: &mut AppState,
    drive: &DriveInfo, w: f32, h: f32,
    tx: &Sender<BackendMsg>,
) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let is_ntfs = drive.fs_type.to_lowercase().contains("ntfs");

    if ui.is_rect_visible(rect) {
        // Kart arka planı
        let bg = if t.effects.glass_panels {
            if resp.hovered() {
                Color32::from_rgba_unmultiplied(
                    c.bg_surface1().r(), c.bg_surface1().g(), c.bg_surface1().b(),
                    (t.effects.card_opacity * 255.0) as u8,
                )
            } else {
                Color32::from_rgba_unmultiplied(
                    c.bg_surface0().r(), c.bg_surface0().g(), c.bg_surface0().b(),
                    (t.effects.card_opacity * 180.0) as u8,
                )
            }
        } else {
            if resp.hovered() { c.bg_surface0() } else { c.bg_mantle() }
        };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_lg), bg);
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_lg),
            Stroke::new(1.0_f32, if resp.hovered() { c.accent() } else { c.border() }));

        let pad = sp.md;

        // İkon
        let ico = if is_ntfs { icon(t, "ntfs") } else { icon(t, "drive") };
        let ico_color = if is_ntfs { Color32::from_rgb(0, 120, 215) } else { c.accent() };
        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 4.0), Align2::LEFT_TOP, ico,
            FontId::new(22.0, FontFamily::Monospace), ico_color,
        );

        // NTFS badge
        if is_ntfs {
            let bw = 42.0_f32; let bh = 16.0_f32;
            let br = Rect::from_min_size(
                rect.min + Vec2::new(pad + 26.0, pad + 6.0), Vec2::new(bw, bh));
            ui.painter().rect_filled(br, Rounding::same(4.0),
                Color32::from_rgba_unmultiplied(0, 120, 215, 40));
            ui.painter().text(br.center(), Align2::CENTER_CENTER, "NTFS",
                FontId::new(10.0, FontFamily::Proportional),
                Color32::from_rgb(0, 120, 215));
        }

        // İsim
        let name = truncate(&drive.name, 22);
        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 30.0), Align2::LEFT_TOP, &name,
            FontId::new(t.typography.size_sm, FontFamily::Proportional), c.text_primary(),
        );

        // Device path
        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 44.0), Align2::LEFT_TOP, &drive.device,
            FontId::new(t.typography.size_xs, FontFamily::Monospace), c.text_hint(),
        );

        // Kullanım bar
        let bar_y  = rect.max.y - pad - 22.0;
        let bar_w  = rect.width() - pad * 2.0;
        let bar_h2 = 4.0_f32;
        let bar_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + pad, bar_y), Vec2::new(bar_w, bar_h2));

        // Arka plan
        ui.painter().rect_filled(bar_rect, Rounding::same(2.0), c.bg_surface1());

        // Dolu kısım
        let used_pct = if drive.total > 0 { drive.used as f32 / drive.total as f32 } else { 0.0 };
        let used_pct = used_pct.clamp(0.0, 1.0);
        let used_color = if used_pct > 0.9 { c.danger() }
                         else if used_pct > 0.75 { c.warning() }
                         else { c.accent() };
        if used_pct > 0.0 {
            let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_w * used_pct, bar_h2));
            ui.painter().rect_filled(fill_rect, Rounding::same(2.0), used_color);
        }

        // Boyut yazısı
        let free = human_size(drive.available);
        let total = human_size(drive.total);
        ui.painter().text(
            Pos2::new(rect.min.x + pad, bar_y + bar_h2 + 4.0), Align2::LEFT_TOP,
            format!("{} free of {}", free, total),
            FontId::new(t.typography.size_xs, FontFamily::Proportional), c.text_hint(),
        );
    }

    // Tıklama — içine gir
    if resp.double_clicked() || resp.clicked() {
        state.panels.close();
        state.navigate(drive.mount_point.clone());
    }

    // Sağ tık menüsü
    resp.context_menu(|ui| {
        if ui.button(format!("{} Open", icon(t, "folder_open"))).clicked() {
            state.panels.close();
            state.navigate(drive.mount_point.clone());
            ui.close_menu();
        }
        if ui.button(format!("{} Open in New Tab", icon(t, "folder_open"))).clicked() {
            state.open_in_new_tab(drive.mount_point.clone());
            ui.close_menu();
        }
        ui.separator();
        if ui.button(format!("{} Unmount", icon(t, "drive"))).clicked() {
            let mp  = drive.mount_point.clone();
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                let r = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(crate::backend::mounts::unmount_device(mp.clone()));
                match r {
                    Ok(m) => {
                        tx2.send(BackendMsg::UnmountDone { success: true, message: m, target: mp }).ok();
                    }
                    Err(e) => {
                        tx2.send(BackendMsg::UnmountDone { success: false, message: e, target: mp }).ok();
                    }
                }
            });
            ui.close_menu();
        }
    });
}

// ── Unmounted card ────────────────────────────────────────────────────────────

fn unmounted_card(
    ui: &mut Ui, t: &AexTheme, state: &mut AppState,
    dev: &crate::backend::mounts::BlockDevice, w: f32, h: f32,
) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if t.effects.glass_panels {
            if resp.hovered() {
                Color32::from_rgba_unmultiplied(
                    c.bg_surface1().r(), c.bg_surface1().g(), c.bg_surface1().b(),
                    (t.effects.card_opacity * 255.0) as u8,
                )
            } else {
                Color32::from_rgba_unmultiplied(
                    c.bg_surface0().r(), c.bg_surface0().g(), c.bg_surface0().b(),
                    (t.effects.card_opacity * 160.0) as u8,
                )
            }
        } else {
            if resp.hovered() { c.bg_surface0() } else { c.bg_mantle() }
        };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_lg), bg);
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_lg),
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(c.border().r(), c.border().g(), c.border().b(), 80)));

        let pad = sp.md;

        let is_ntfs = dev.fs_type.to_lowercase().contains("ntfs");
        let ico = if is_ntfs { icon(t, "ntfs") } else { icon(t, "drive") };
        let ico_color = if is_ntfs { Color32::from_rgb(0, 120, 215) } else { c.text_disabled() };

        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 4.0), Align2::LEFT_TOP,
            ico,
            FontId::new(22.0, FontFamily::Monospace), ico_color,
        );

        let label = if dev.label.is_empty() { &dev.device } else { &dev.label };
        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 30.0), Align2::LEFT_TOP,
            truncate(label, 22),
            FontId::new(t.typography.size_sm, FontFamily::Proportional), c.text_secondary(),
        );

        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 44.0), Align2::LEFT_TOP,
            format!("{} · {}", dev.device, dev.fs_type),
            FontId::new(t.typography.size_xs, FontFamily::Monospace), c.text_hint(),
        );

        // Mount Et butonu
        let btn_w = w - pad * 2.0; let btn_h = 22.0_f32;
        let btn_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + pad, rect.max.y - pad - btn_h),
            Vec2::new(btn_w, btn_h),
        );
        let btn_resp = ui.interact(btn_rect, ui.id().with(&dev.device), Sense::click());
        let btn_color = if btn_resp.hovered() { c.accent() } else { c.text_hint() };
        ui.painter().rect_filled(btn_rect, Rounding::same(s.radius_sm),
            Color32::from_rgba_unmultiplied(btn_color.r(), btn_color.g(), btn_color.b(), 20));
        ui.painter().text(btn_rect.center(), Align2::CENTER_CENTER,
            format!("{} Mount", icon(t, "mount")),
            FontId::new(t.typography.size_xs, FontFamily::Proportional), btn_color);

        if btn_resp.clicked() || resp.clicked() || resp.double_clicked() {
            state.panels.open_mount();
        }
    }
}

// ── ADB Device Card ──────────────────────────────────────────────────────────

fn adb_card(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    adb_dev: &crate::backend::adb::AdbDevice,
    w: f32,
    h: f32,
) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let target_path = crate::backend::adb::make_adb_path(&adb_dev.serial, "/sdcard");
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() { c.bg_surface1() } else { c.bg_surface0() };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_lg), bg);
        ui.painter().rect_stroke(
            rect,
            Rounding::same(s.radius_lg),
            Stroke::new(1.0_f32, if resp.hovered() { Color32::from_rgb(164, 198, 57) } else { c.border() }),
        );

        let pad = sp.md;

        // Telefon İkonu
        let ico = icon(t, "phone");
        let ico_color = if adb_dev.is_authorized {
            Color32::from_rgb(164, 198, 57) // Android green
        } else {
            c.warning()
        };

        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 4.0),
            Align2::LEFT_TOP,
            ico,
            FontId::new(24.0, FontFamily::Monospace),
            ico_color,
        );

        // ADB Badge
        let bw = 36.0_f32;
        let bh = 14.0_f32;
        let br = Rect::from_min_size(rect.min + Vec2::new(pad + 28.0, pad + 6.0), Vec2::new(bw, bh));
        ui.painter().rect_filled(br, Rounding::same(4.0), Color32::from_rgba_unmultiplied(164, 198, 57, 40));
        ui.painter().text(
            br.center(),
            Align2::CENTER_CENTER,
            "ADB",
            FontId::new(8.0, FontFamily::Proportional),
            Color32::from_rgb(164, 198, 57),
        );

        // Cihaz Adı
        let name = truncate(&adb_dev.display_name(), 22);
        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 32.0),
            Align2::LEFT_TOP,
            &name,
            FontId::new(t.typography.size_sm, FontFamily::Proportional),
            c.text_primary(),
        );

        // Seri No / Durum
        let status_desc = if adb_dev.is_authorized {
            format!("{} · Internal Storage", adb_dev.serial)
        } else {
            format!("{} · Unauthorized (check screen)", adb_dev.serial)
        };
        ui.painter().text(
            rect.min + Vec2::new(pad, pad + 48.0),
            Align2::LEFT_TOP,
            &status_desc,
            FontId::new(t.typography.size_xs, FontFamily::Monospace),
            if adb_dev.is_authorized { c.text_hint() } else { c.warning() },
        );

        // Kullanım barı
        if adb_dev.is_authorized && adb_dev.total_storage > 0 {
            let bar_y = rect.max.y - pad - 22.0;
            let bar_w = rect.width() - pad * 2.0;
            let bar_h2 = 4.0_f32;
            let bar_rect = Rect::from_min_size(Pos2::new(rect.min.x + pad, bar_y), Vec2::new(bar_w, bar_h2));

            ui.painter().rect_filled(bar_rect, Rounding::same(2.0), c.bg_surface1());

            let used_pct = (adb_dev.used_storage as f32 / adb_dev.total_storage as f32).clamp(0.0, 1.0);
            if used_pct > 0.0 {
                let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_w * used_pct, bar_h2));
                ui.painter().rect_filled(fill_rect, Rounding::same(2.0), Color32::from_rgb(164, 198, 57));
            }

            let free_bytes = adb_dev.total_storage.saturating_sub(adb_dev.used_storage);
            let stat_str = format!("{} boş / {} toplam", human_size(free_bytes), human_size(adb_dev.total_storage));
            ui.painter().text(
                rect.min + Vec2::new(pad, rect.height() - pad - 6.0),
                Align2::LEFT_TOP,
                &stat_str,
                FontId::new(t.typography.size_xs, FontFamily::Proportional),
                c.text_hint(),
            );
        } else if !adb_dev.is_authorized {
            ui.painter().text(
                rect.min + Vec2::new(pad, rect.height() - pad - 12.0),
                Align2::LEFT_TOP,
                "Telefondan izin verin",
                FontId::new(t.typography.size_xs, FontFamily::Proportional),
                c.warning(),
            );
        }
    }

    if resp.clicked() || resp.double_clicked() {
        if adb_dev.is_authorized {
            state.panels.close();
            state.navigate(target_path);
        } else {
            state.notify.warning("Lütfen telefondan USB Hata Ayıklama iznini onaylayın!".to_string());
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_label(ui: &mut Ui, t: &AexTheme, label: &str) {
    let c = &t.colors;
    ui.label(RichText::new(label).color(c.text_hint()).size(t.typography.size_xs).strong());
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() }
    else { format!("{}…", s.chars().take(max - 1).collect::<String>()) }
}
