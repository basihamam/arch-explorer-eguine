// src/ui/context_menu.rs
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use super::icon;

pub trait CtxResponseExt {
    fn ctx_act(&self) -> bool;
}

impl CtxResponseExt for Response {
    fn ctx_act(&self) -> bool {
        self.clicked() || self.secondary_clicked()
    }
}

pub fn render(
    ctx: &Context,
    t: &AexTheme,
    state: &mut AppState,
    tx: &std::sync::mpsc::Sender<crate::app::BackendMsg>,
    _rt: &tokio::runtime::Runtime,
) {
    let Some(ref cm) = state.panels.context_menu.clone() else { return };

    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;
    let ih = t.context_menu.item_height;

    let screen  = ctx.screen_rect();
    let menu_w  = 230.0_f32;

    // item sayısına göre yükseklik
    let item_count = if cm.target.is_some() { 14.0 } else { 5.0 };
    let menu_h  = ih * item_count + sp.sm * 2.0;
    let x = cm.x.min(screen.right() - menu_w - 8.0);
    let y = cm.y.min(screen.bottom() - menu_h - 8.0);
    let menu_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(menu_w, menu_h));

    // Open With hover state — submenu kontrolü
    let ow_hover_id = Id::new("ctx_ow_hover");
    let mut ow_hovered: bool = ctx.data_mut(|d| d.get_temp(ow_hover_id).unwrap_or(false));

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

    egui::Area::new(Id::new("ctx_menu_area"))
        .fixed_pos(Pos2::new(x, y))
        .order(Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame {
                fill: menu_fill,
                rounding: Rounding::same(s.radius_md),
                stroke: Stroke::new(1.0_f32, c.border()),
                shadow: epaint::Shadow {
                    offset: Vec2::new(0.0, 4.0),
                    blur: 12.0,
                    spread: 0.0,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 80),
                },
                inner_margin: Margin::same(sp.xs),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.set_width(menu_w - sp.xs * 2.0);

                if let Some(ref entry) = cm.target.clone() {
                    // ── File/Folder context menu — sadece core items ─────────

                    // Open
                    if ctx_item(ui, t, icon(t, "folder_open"), "Open", None).ctx_act() {
                        if entry.is_dir { state.navigate(entry.path.clone()); }
                        else { std::process::Command::new("xdg-open").arg(&entry.path).spawn().ok(); }
                        state.panels.close_context_menu();
                    }
                    if entry.is_dir {
                        if ctx_item(ui, t, icon(t, "folder_open"), "Open in New Tab", None).ctx_act() {
                            state.open_in_new_tab(entry.path.clone());
                            state.panels.close_context_menu();
                        }
                    }

                    // Open With — ana menüde doğrudan erişilebilir
                    let ow_resp = ctx_item(ui, t, icon(t, "open_with"), "Open With…", Some("▶"));
                    if ow_resp.hovered() {
                        ow_hovered = true;
                    }
                    if ow_resp.ctx_act() {
                        state.panels.open_open_with(entry.path.clone());
                        state.panels.close_context_menu();
                    }

                    ctx_sep(ui, t);

                    let has_selection = state.selection.count() > 0;
                    let has_clipboard = crate::backend::clipboard::clipboard_get()
                        .map(|cb| !cb.paths.is_empty())
                        .unwrap_or(false);

                    // Cut
                    if has_selection {
                        if ctx_item(ui, t, icon(t, "cut"), "Cut", Some("Ctrl+X")).ctx_act() {
                            let paths = state.selected_paths();
                            let count = paths.len();
                            crate::backend::clipboard::clipboard_cut(paths).ok();
                            state.notify.info(format!("{} öğe panoya kesildi", count));
                            state.panels.close_context_menu();
                        }
                    } else {
                        ctx_item_disabled(ui, t, icon(t, "cut"), "Cut", Some("Ctrl+X"));
                    }

                    // Copy
                    if has_selection {
                        if ctx_item(ui, t, icon(t, "copy"), "Copy", Some("Ctrl+C")).ctx_act() {
                            let paths = state.selected_paths();
                            let count = paths.len();
                            crate::backend::clipboard::clipboard_copy(paths).ok();
                            state.notify.info(format!("{} öğe panoya kopyalandı", count));
                            state.panels.close_context_menu();
                        }
                    } else {
                        ctx_item_disabled(ui, t, icon(t, "copy"), "Copy", Some("Ctrl+C"));
                    }

                    // Duplicate
                    if has_selection {
                        if ctx_item(ui, t, icon(t, "copy"), "Duplicate", Some("Ctrl+D")).ctx_act() {
                            state.duplicate_selected();
                            state.panels.close_context_menu();
                        }
                    } else {
                        ctx_item_disabled(ui, t, icon(t, "copy"), "Duplicate", Some("Ctrl+D"));
                    }

                    // Paste
                    if has_clipboard {
                        if ctx_item(ui, t, icon(t, "paste"), "Paste", Some("Ctrl+V")).ctx_act() {
                            let p = if entry.is_dir { entry.path.clone() } else { state.nav.current_path.clone() };
                            if let Some(cb) = crate::backend::clipboard::clipboard_get() {
                                let is_copy = cb.op == crate::backend::clipboard::ClipboardOp::Copy;
                                if !is_copy {
                                    let _ = crate::backend::clipboard::clipboard_clear();
                                }
                                state.start_transfer(cb.paths, p, is_copy);
                            }
                            state.panels.close_context_menu();
                        }
                    } else {
                        ctx_item_disabled(ui, t, icon(t, "paste"), "Paste", Some("Ctrl+V"));
                    }

                    // Copy Path
                    if ctx_item(ui, t, icon(t, "copy"), "Copy Path", None).ctx_act() {
                        crate::backend::clipboard::copy_to_system_clipboard_async(entry.path.clone());
                        state.notify.info("Dosya yolu panoya kopyalandı".into());
                        state.panels.close_context_menu();
                    }

                    ctx_sep(ui, t);

                    // Rename / Bulk Rename
                    let rename_label = if state.selection.count() >= 2 { "Bulk Rename…" } else { "Rename" };
                    if has_selection {
                        if ctx_item(ui, t, icon(t, "rename"), rename_label, Some("F2")).clicked() {
                            if state.selection.count() >= 2 { state.panels.open_bulk_rename(); }
                            else { state.start_rename(&entry.path.clone()); }
                            state.panels.close_context_menu();
                        }
                    } else {
                        ctx_item_disabled(ui, t, icon(t, "rename"), rename_label, Some("F2"));
                    }

                    ctx_sep(ui, t);

                    // New Folder / New File
                    if ctx_item(ui, t, icon(t, "new_folder"), "New Folder", Some("Ctrl+Shift+N")).clicked() {
                        let path = unique_new_folder_path(&state.nav.current_path);
                        try_create_dir_or_elevate(state, path);
                        state.panels.close_context_menu();
                    }
                    if ctx_item(ui, t, icon(t, "new_file"), "New File", Some("Ctrl+N")).clicked() {
                        let path = unique_new_file_path(&state.nav.current_path);
                        try_create_file_or_elevate(state, path);
                        state.panels.close_context_menu();
                    }

                    ctx_sep(ui, t);

                    let is_current_in_trash = state.nav.current_path.starts_with("trash:")
                        || state.nav.current_path.contains("/.local/share/Trash");

                    // Delete / Restore (trash / permanent)
                    if has_selection {
                        if is_current_in_trash {
                            if ctx_item_colored(ui, t, icon(t, "refresh"), "Restore from Trash", None, c.accent()).clicked() {
                                let paths = state.selected_paths();
                                let mut restored = Vec::new();
                                for p in &paths {
                                    match crate::backend::fs_ops::restore_from_trash(p) {
                                        Ok(orig) => restored.push(orig),
                                        Err(e) => state.notify.error(format!("Geri yükleme hatası: {}", e)),
                                    }
                                }
                                state.refresh();
                                if !restored.is_empty() {
                                    let count = restored.len();
                                    state.notify.success(format!("{} öğe çöp kutusundan geri yüklendi", count));
                                    state.select_paths_and_queue(restored);
                                }
                                state.panels.close_context_menu();
                            }
                            if ctx_item_colored(ui, t, icon(t, "error"), "Delete Permanently", Some("Del"), c.danger()).clicked() {
                                let paths = state.selected_paths();
                                try_delete_or_elevate(state, paths, true);
                                state.panels.close_context_menu();
                            }
                        } else {
                            if ctx_item_colored(ui, t, icon(t, "delete"), "Move to Trash", Some("Del"), c.warning()).clicked() {
                                let paths = state.selected_paths();
                                try_delete_or_elevate(state, paths, false);
                                state.panels.close_context_menu();
                            }
                            if ctx_item_colored(ui, t, icon(t, "error"), "Delete Permanently", Some("Shift+Del"), c.danger()).clicked() {
                                let paths = state.selected_paths();
                                try_delete_or_elevate(state, paths, true);
                                state.panels.close_context_menu();
                            }
                        }
                    } else {
                        if is_current_in_trash {
                            ctx_item_disabled(ui, t, icon(t, "refresh"), "Restore from Trash", None);
                            ctx_item_disabled(ui, t, icon(t, "error"), "Delete Permanently", Some("Del"));
                        } else {
                            ctx_item_disabled(ui, t, icon(t, "delete"), "Move to Trash", Some("Del"));
                            ctx_item_disabled(ui, t, icon(t, "error"), "Delete Permanently", Some("Shift+Del"));
                        }
                    }

                    ctx_sep(ui, t);

                    // ── Other ▶ — hover veya tık ile SOL tarafta açılır ──────
                    let other_resp = ctx_item_arrow(ui, t, icon(t, "plugin"), "Other");
                    let other_id   = egui::Id::new("ctx_other_open");
                    let mut is_other_open: bool = ui.ctx().data_mut(|d| d.get_temp(other_id).unwrap_or(false));

                    if other_resp.hovered() || other_resp.clicked() {
                        is_other_open = true;
                        ui.ctx().data_mut(|d| d.insert_temp(other_id, true));
                    }

                    if is_other_open {
                        // Context menu'nun sol tarafında aç
                        let cm_rect = ui.min_rect();
                        let sub_w   = 220.0_f32;
                        let sub_x   = cm_rect.min.x - sub_w - 4.0;
                        let sub_y   = other_resp.rect.min.y;

                        egui::Area::new(egui::Id::new("ctx_other_panel"))
                            .fixed_pos(egui::pos2(sub_x, sub_y))
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                egui::Frame {
                                    fill: menu_fill,
                                    rounding: egui::Rounding::same(s.radius_md),
                                    stroke: egui::Stroke::new(1.0_f32, c.border()),
                                    inner_margin: egui::Margin::same(sp.sm),
                                    shadow: epaint::Shadow {
                                        offset: egui::Vec2::new(0.0, 4.0),
                                        blur: 12.0, spread: 0.0,
                                        color: egui::Color32::from_rgba_unmultiplied(0,0,0,80),
                                    },
                                    ..Default::default()
                                }.show(ui, |ui| {
                                    ui.set_width(sub_w - sp.sm * 2.0);

                                    // Open With
                                    if ctx_item(ui, t, icon(t, "open_with"), "Open With…", None).clicked() {
                                        state.panels.open_open_with(entry.path.clone());
                                        state.panels.close_context_menu();
                                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                    }

                                    // Open Terminal Here
                                    let dir_path = if entry.is_dir { entry.path.clone() } else { state.nav.current_path.clone() };
                                    if ctx_item(ui, t, icon(t, "terminal"), "Open Terminal Here", None).clicked() {
                                        state.open_terminal(Some(dir_path));
                                        state.panels.close_context_menu();
                                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                    }

                                    ctx_sep(ui, t);

                                    // Create Symlink
                                    if ctx_item(ui, t, icon(t, "symlink"), "Create Symlink", None).clicked() {
                                        let src  = entry.path.clone();
                                        let dest = format!("{}/{}.lnk", state.nav.current_path, entry.name);
                                        std::os::unix::fs::symlink(&src, &dest).ok();
                                        state.refresh();
                                        state.panels.close_context_menu();
                                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                    }

                                    // Add to Bookmarks
                                    if ctx_item(ui, t, icon(t, "star"), "Add to Bookmarks", None).clicked() {
                                        crate::backend::fs_ops::add_bookmark(entry.name.clone(), entry.path.clone()).ok();
                                        state.sidebar.bookmarks = crate::backend::fs_ops::get_bookmarks();
                                        state.panels.close_context_menu();
                                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                    }

                                    ctx_sep(ui, t);

                                    // Archive ops
                                    let is_archive = matches!(
                                        std::path::Path::new(&entry.path)
                                            .extension().and_then(|e| e.to_str()).unwrap_or(""),
                                        "zip"|"tar"|"gz"|"tgz"|"bz2"|"xz"|"zst"|"7z"|"rar"
                                    );
                                    if is_archive {
                                        if ctx_item(ui, t, icon(t, "folder_open"), "Extract Here", None).clicked() {
                                            state.pending_extract = Some((entry.path.clone(), state.nav.current_path.clone()));
                                            state.panels.close_context_menu();
                                            ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                        }
                                    }
                                    if ctx_item(ui, t, icon(t, "copy"), "Compress…", None).clicked() {
                                        state.panels.open_compress();
                                        state.panels.close_context_menu();
                                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                    }

                                    // Checksum (dosya için)
                                    if !entry.is_dir {
                                        if ctx_item(ui, t, icon(t, "info"), "Checksum", None).clicked() {
                                            state.pending_checksum_request = Some(entry.path.clone());
                                            state.panels.close_context_menu();
                                            ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                        }
                                    }

                                    ctx_sep(ui, t);

                                    // Properties
                                    if ctx_item(ui, t, icon(t, "info"), "Properties", None).clicked() {
                                        state.panels.open_properties(entry.path.clone());
                                        state.panels.close_context_menu();
                                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                    }

                                    // User commands
                                    let cmds = crate::backend::commands::load_user_commands();
                                    let ctx_cmds: Vec<_> = cmds.iter().filter(|c| c.show_in_context_menu).collect();
                                    if !ctx_cmds.is_empty() {
                                        ctx_sep(ui, t);
                                        for cmd in ctx_cmds {
                                            if ctx_item(ui, t, icon(t, &cmd.icon), &cmd.name, None).clicked() {
                                                let id  = cmd.id.clone();
                                                let sel = Some(entry.path.clone());
                                                let dir = state.nav.current_path.clone();
                                                std::thread::spawn(move || {
                                                    crate::backend::commands::run_user_command(id, sel, dir).ok();
                                                });
                                                state.panels.close_context_menu();
                                                ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                            }
                                        }
                                    }
                                });

                                // Diğer panelin veya ana menünün üzerinde olup olmadığını kontrol et
                                let sub_min = ui.min_rect();
                                let pointer_in_sub = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().map(|p| sub_min.expand(6.0).contains(p)).unwrap_or(false)
                                });
                                let pointer_in_main = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().map(|p| cm_rect.expand(4.0).contains(p)).unwrap_or(false)
                                });

                                if !pointer_in_sub && !pointer_in_main {
                                    ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                                }
                            });
                    }

                    // Escape ile Other kapat
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
                    }

                } else {
                    // ── Background context menu ───────────────────────────
                    let has_clipboard = crate::backend::clipboard::clipboard_get()
                        .map(|cb| !cb.paths.is_empty())
                        .unwrap_or(false);

                    if has_clipboard {
                        if ctx_item(ui, t, icon(t, "paste"), "Paste", Some("Ctrl+V")).clicked() {
                            let p = state.nav.current_path.clone();
                            if let Some(cb) = crate::backend::clipboard::clipboard_get() {
                                let is_copy = cb.op == crate::backend::clipboard::ClipboardOp::Copy;
                                if !is_copy {
                                    let _ = crate::backend::clipboard::clipboard_clear();
                                }
                                state.start_transfer(cb.paths, p, is_copy);
                            }
                            state.panels.close_context_menu();
                        }
                    } else {
                        ctx_item_disabled(ui, t, icon(t, "paste"), "Paste", Some("Ctrl+V"));
                    }

                    if ctx_item(ui, t, icon(t, "new_folder"), "New Folder", Some("Ctrl+Shift+N")).clicked() {
                        let path = unique_new_folder_path(&state.nav.current_path);
                        try_create_dir_or_elevate(state, path);
                        state.panels.close_context_menu();
                    }

                    if ctx_item(ui, t, icon(t, "new_file"), "New File", Some("Ctrl+N")).clicked() {
                        let path = unique_new_file_path(&state.nav.current_path);
                        try_create_file_or_elevate(state, path);
                        state.panels.close_context_menu();
                    }

                    ctx_sep(ui, t);

                    // Open Terminal in current dir
                    let cur_path = state.nav.current_path.clone();
                    if ctx_item(ui, t, icon(t, "terminal"), "Open Terminal Here", None).clicked() {
                        state.open_terminal(Some(cur_path));
                        state.panels.close_context_menu();
                    }

                    if ctx_item(ui, t, icon(t, "refresh"), "Refresh", Some("F5")).clicked() {
                        state.refresh();
                        state.panels.close_context_menu();
                    }
                }
            });
        });

    // ── Open With submenu ─────────────────────────────────────────────────────
    if ow_hovered {
        if let Some(ref entry) = cm.target.clone() {
            let path = entry.path.clone();
            // Hızlı app listesini al (cache edilmemiş — küçük dosyalar için OK)
            let (default_app, recommended) =
                crate::backend::open_with::get_quick_apps(&path);

            let sub_w = 210.0_f32;
            let sub_x = if x + menu_w + sub_w + 8.0 > screen.right() {
                (x - sub_w - 4.0).max(8.0)
            } else {
                x + menu_w + 4.0
            };
            let sub_item_h = t.context_menu.item_height;
            let sub_count  = 1 + default_app.is_some() as usize + recommended.len();
            let sub_h = sub_item_h * sub_count as f32 + sp.sm * 2.0;
            let sub_y = y.min(screen.bottom() - sub_h - 8.0);
            let sub_rect = Rect::from_min_size(
                Pos2::new(sub_x, sub_y), Vec2::new(sub_w, sub_h)
            );

            let _sub_resp = egui::Area::new(Id::new("ctx_ow_submenu"))
                .fixed_pos(Pos2::new(sub_x, sub_y))
                .order(Order::Tooltip)
                .show(ctx, |ui| {
                    egui::Frame {
                        fill: menu_fill,
                        rounding: Rounding::same(s.radius_md),
                        stroke: Stroke::new(1.0_f32, c.border()),
                        shadow: epaint::Shadow {
                            offset: Vec2::new(2.0, 4.0), blur: 8.0, spread: 0.0,
                            color: Color32::from_rgba_unmultiplied(0,0,0,70),
                        },
                        inner_margin: Margin::same(sp.xs),
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.set_width(sub_w - sp.xs * 2.0);

                        // Default
                        if let Some(ref app) = default_app {
                            let r = ctx_item_app(ui, t, app, true);
                            if r.clicked() {
                                let p = path.clone(); let id = app.id.clone();
                                std::thread::spawn(move || {
                                    crate::backend::open_with::open_with_app(p, id).ok();
                                });
                                state.panels.close_context_menu();
                                ow_hovered = false;
                            }
                        }

                        // Recommended
                        for app in &recommended {
                            let r = ctx_item_app(ui, t, app, false);
                            if r.clicked() {
                                let p = path.clone(); let id = app.id.clone();
                                std::thread::spawn(move || {
                                    crate::backend::open_with::open_with_app(p, id).ok();
                                });
                                state.panels.close_context_menu();
                                ow_hovered = false;
                            }
                        }

                        ctx_sep(ui, t);

                        // More…
                        if ctx_item(ui, t, icon(t, "open_with"), "More Applications…", None).clicked() {
                            state.panels.open_open_with(path.clone());
                            state.panels.close_context_menu();
                            ow_hovered = false;
                        }
                    });
                });

            // Submenu veya ana menü dışına çıkınca kapat
            let sub_hovered = ctx.input(|i| {
                i.pointer.hover_pos()
                    .map(|p| sub_rect.expand(6.0).contains(p) || menu_rect.expand(4.0).contains(p))
                    .unwrap_or(false)
            });
            if !sub_hovered { ow_hovered = false; }
        }
    }

    // Close on click outside (ana menü veya açılan alt pencereler dışı)
    if ctx.input(|i| i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            let is_other_open: bool = ctx.data_mut(|d| d.get_temp(egui::Id::new("ctx_other_open")).unwrap_or(false));
            let mut in_any_menu = menu_rect.contains(pos);
            if is_other_open {
                // Other sub_rect sol tarafta:
                let other_sub_rect = Rect::from_min_size(
                    Pos2::new(menu_rect.min.x - 224.0, menu_rect.min.y),
                    Vec2::new(224.0, 360.0)
                );
                if other_sub_rect.contains(pos) {
                    in_any_menu = true;
                }
            }
            if !in_any_menu {
                state.panels.close_context_menu();
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("ctx_other_open"), false));
                ow_hovered = false;
            }
        }
    }

    ctx.data_mut(|d| d.insert_temp(ow_hover_id, ow_hovered));
}

// ── Widget helpers ────────────────────────────────────────────────────────────

fn ctx_item(ui: &mut Ui, t: &AexTheme, icon_str: &str, label: &str, shortcut: Option<&str>) -> Response {
    ctx_item_colored(ui, t, icon_str, label, shortcut, t.colors.text_secondary())
}

fn ctx_item_arrow(ui: &mut Ui, t: &AexTheme, icon_str: &str, label: &str) -> Response {
    let c  = &t.colors;
    let s  = &t.shape;
    let h  = t.context_menu.item_height;
    let w  = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() { c.bg_surface1() } else { Color32::TRANSPARENT };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);
        ui.painter().text(
            rect.min + Vec2::new(t.spacing.md, h / 2.0),
            Align2::LEFT_CENTER, icon_str,
            FontId::new(t.icons.size_sm, FontFamily::Monospace), c.text_secondary(),
        );
        let lx = t.spacing.md + 20.0;
        ui.painter().text(
            rect.min + Vec2::new(lx, h / 2.0),
            Align2::LEFT_CENTER, label,
            FontId::new(t.typography.size_sm, FontFamily::Proportional), c.text_secondary(),
        );
        // Arrow ▶
        ui.painter().text(
            rect.max - Vec2::new(t.spacing.md, h / 2.0),
            Align2::RIGHT_CENTER, "▶",
            FontId::new(8.0, FontFamily::Proportional), c.text_hint(),
        );
    }
    resp
}

fn ctx_item_app(ui: &mut Ui, t: &AexTheme, app: &crate::backend::open_with::AppEntry, is_default: bool) -> Response {
    let c = &t.colors; let s = &t.shape;
    let h = (t.context_menu.item_height + 4.0).max(28.0);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() { c.bg_surface1() } else { Color32::TRANSPARENT };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);

        // App Icon
        let icon_size = 18.0_f32;
        let icon_rect = Rect::from_center_size(
            rect.min + Vec2::new(t.spacing.md + icon_size / 2.0, h / 2.0),
            Vec2::splat(icon_size),
        );

        if let Some(tex) = crate::ui::open_with::get_or_load_app_icon(ui.ctx(), app) {
            ui.painter().image(
                tex.id(),
                icon_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            let fb_char = app.name.chars().next().unwrap_or('A').to_uppercase().to_string();
            ui.painter().rect_filled(icon_rect, Rounding::same(4.0), c.bg_surface2());
            ui.painter().text(
                icon_rect.center(),
                Align2::CENTER_CENTER,
                &fb_char,
                FontId::new(10.0, FontFamily::Proportional),
                c.text_primary(),
            );
        }

        let color = if is_default { c.accent() } else { c.text_secondary() };
        let label = if is_default { format!("★ {}", app.name) } else { app.name.clone() };
        let text_x = t.spacing.md + icon_size + 8.0;
        ui.painter().text(
            rect.min + Vec2::new(text_x, h / 2.0),
            Align2::LEFT_CENTER, &label,
            FontId::new(t.typography.size_sm, FontFamily::Proportional), color,
        );
    }
    resp
}


fn ctx_item_colored(ui: &mut Ui, t: &AexTheme, icon_str: &str, label: &str, shortcut: Option<&str>, color: Color32) -> Response {
    let c  = &t.colors;
    let s  = &t.shape;
    let h  = t.context_menu.item_height;
    let w  = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() { c.bg_surface1() } else { Color32::TRANSPARENT };
        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);
        if t.context_menu.show_icons {
            ui.painter().text(
                rect.min + Vec2::new(t.spacing.md, h / 2.0),
                Align2::LEFT_CENTER, icon_str,
                FontId::new(t.icons.size_sm, FontFamily::Monospace), color,
            );
        }
        let lx = if t.context_menu.show_icons { t.spacing.md + 20.0 } else { t.spacing.md };
        ui.painter().text(
            rect.min + Vec2::new(lx, h / 2.0),
            Align2::LEFT_CENTER, label,
            FontId::new(t.typography.size_sm, FontFamily::Proportional), color,
        );
        if t.context_menu.show_shortcuts {
            if let Some(sc) = shortcut {
                ui.painter().text(
                    rect.max - Vec2::new(t.spacing.md, h / 2.0),
                    Align2::RIGHT_CENTER, sc,
                    FontId::new(t.typography.size_xs, FontFamily::Monospace), c.text_hint(),
                );
            }
        }
    }
    resp
}

fn ctx_item_disabled(ui: &mut Ui, t: &AexTheme, icon_str: &str, label: &str, shortcut: Option<&str>) -> Response {
    let c  = &t.colors;
    let s  = &t.shape;
    let h  = t.context_menu.item_height;
    let w  = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    if ui.is_rect_visible(rect) {
        if t.context_menu.show_icons {
            ui.painter().text(
                rect.min + Vec2::new(t.spacing.md, h / 2.0),
                Align2::LEFT_CENTER, icon_str,
                FontId::new(t.icons.size_sm, FontFamily::Monospace), c.text_disabled(),
            );
        }
        let lx = if t.context_menu.show_icons { t.spacing.md + 20.0 } else { t.spacing.md };
        ui.painter().text(
            rect.min + Vec2::new(lx, h / 2.0),
            Align2::LEFT_CENTER, label,
            FontId::new(t.typography.size_sm, FontFamily::Proportional), c.text_disabled(),
        );
        if t.context_menu.show_shortcuts {
            if let Some(sc) = shortcut {
                ui.painter().text(
                    rect.max - Vec2::new(t.spacing.md, h / 2.0),
                    Align2::RIGHT_CENTER, sc,
                    FontId::new(t.typography.size_xs, FontFamily::Monospace), c.text_disabled(),
                );
            }
        }
    }
    resp
}

fn ctx_sep(ui: &mut Ui, t: &AexTheme) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top() + t.spacing.xs;
    ui.painter().hline(rect.left()..=rect.right(), y, Stroke::new(0.5_f32, t.colors.border()));
    ui.add_space(t.spacing.xs * 2.0 + 0.5);
}

pub fn unique_new_folder_path(parent: &str) -> String {
    if parent.starts_with("adb://") {
        return format!("{}/New Folder", parent.trim_end_matches('/'));
    }
    let base = format!("{}/New Folder", parent.trim_end_matches('/'));
    if !std::path::Path::new(&base).exists() {
        return base;
    }
    let mut i = 2;
    loop {
        let p = format!("{}/New Folder ({})", parent.trim_end_matches('/'), i);
        if !std::path::Path::new(&p).exists() {
            return p;
        }
        i += 1;
    }
}

pub fn unique_new_file_path(parent: &str) -> String {
    if parent.starts_with("adb://") {
        return format!("{}/New File.txt", parent.trim_end_matches('/'));
    }
    let base = format!("{}/New File.txt", parent.trim_end_matches('/'));
    if !std::path::Path::new(&base).exists() {
        return base;
    }
    let mut i = 2;
    loop {
        let p = format!("{}/New File ({}).txt", parent.trim_end_matches('/'), i);
        if !std::path::Path::new(&p).exists() {
            return p;
        }
        i += 1;
    }
}

pub fn try_create_dir_or_elevate(state: &mut AppState, dir_path: String) {
    if dir_path.starts_with("adb://") {
        if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(&dir_path) {
            if let Err(e) = crate::backend::adb::adb_mkdir(&serial, &dev_path) {
                state.notify.error(format!("ADB klasör oluşturma hatası: {}", e));
            } else {
                state.refresh();
                state.selection.select_only(dir_path.clone());
                state.start_rename(&dir_path);
            }
        }
        return;
    }

    if let Err(e) = crate::backend::clipboard::tracked_create_dir(dir_path.clone()) {
        let lower = e.to_lowercase();
        if lower.contains("permission denied")
            || lower.contains("işlem engellendi")
            || lower.contains("access denied")
            || lower.contains("operation not permitted")
            || lower.contains("yetki")
        {
            // 1. Try pkexec
            let status = std::process::Command::new("pkexec")
                .arg("mkdir")
                .arg("-p")
                .arg(&dir_path)
                .status();
            match status {
                Ok(s) if s.success() => {
                    state.notify.success("Klasör root yetkisiyle oluşturuldu".into());
                    state.refresh();
                    state.selection.select_only(dir_path.clone());
                    state.start_rename(&dir_path);
                    return;
                }
                _ => {}
            }

            // 2. Fallback to sudo password dialog
            state.pending_root_auth = Some(crate::state::RootAuthRequest {
                title: "Yeni Klasör Oluştur (Root)".to_string(),
                cmd: "mkdir".to_string(),
                args: vec!["-p".to_string(), dir_path],
                reload: true,
                auth_type: None,
            });
            state.pending_root_wrong = false;
        } else {
            state.notify.error(format!("Klasör oluşturma hatası: {}", e));
        }
    } else {
        state.refresh();
        state.selection.select_only(dir_path.clone());
        state.start_rename(&dir_path);
    }
}

pub fn try_create_file_or_elevate(state: &mut AppState, file_path: String) {
    if file_path.starts_with("adb://") {
        if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(&file_path) {
            if let Err(e) = crate::backend::adb::adb_touch(&serial, &dev_path) {
                state.notify.error(format!("ADB dosya oluşturma hatası: {}", e));
            } else {
                state.refresh();
                state.selection.select_only(file_path.clone());
                state.start_rename(&file_path);
            }
        }
        return;
    }

    if let Err(e) = crate::backend::clipboard::tracked_create_file(file_path.clone()) {
        let lower = e.to_lowercase();
        if lower.contains("permission denied")
            || lower.contains("işlem engellendi")
            || lower.contains("access denied")
            || lower.contains("operation not permitted")
            || lower.contains("yetki")
        {
            // 1. Try pkexec
            let status = std::process::Command::new("pkexec")
                .arg("touch")
                .arg(&file_path)
                .status();
            match status {
                Ok(s) if s.success() => {
                    state.notify.success("Dosya root yetkisiyle oluşturuldu".into());
                    state.refresh();
                    state.selection.select_only(file_path.clone());
                    state.start_rename(&file_path);
                    return;
                }
                _ => {}
            }

            // 2. Fallback to sudo password dialog
            state.pending_root_auth = Some(crate::state::RootAuthRequest {
                title: "Yeni Dosya Oluştur (Root)".to_string(),
                cmd: "touch".to_string(),
                args: vec![file_path],
                reload: true,
                auth_type: None,
            });
            state.pending_root_wrong = false;
        } else {
            state.notify.error(format!("Dosya oluşturma hatası: {}", e));
        }
    } else {
        state.refresh();
        state.selection.select_only(file_path.clone());
        state.start_rename(&file_path);
    }
}


pub(crate) fn try_delete_or_elevate(state: &mut AppState, paths: Vec<String>, permanent: bool) {
    let mut need_root = vec![];
    let mut deleted_undos = vec![];

    let any_adb = paths.iter().any(|p| p.starts_with("adb://"));
    if any_adb {
        let mut count = 0;
        for p in &paths {
            if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(p) {
                if let Err(e) = crate::backend::adb::adb_delete(&serial, &dev_path) {
                    state.notify.error(format!("ADB silme hatası: {}", e));
                } else {
                    count += 1;
                }
            }
        }
        state.refresh();
        if count > 0 {
            state.notify.success(format!("{} öğe telefondan kalıcı olarak silindi", count));
        }
        return;
    }

    for (idx, p) in paths.iter().enumerate() {
        let is_dir = std::path::Path::new(p).is_dir();
        if let Err(e) = crate::backend::fs_ops::delete_item(p.clone(), permanent) {
            let lower = e.to_lowercase();
            if lower.contains("permission denied")
                || lower.contains("işlem engellendi")
                || lower.contains("access denied")
                || lower.contains("operation not permitted")
                || lower.contains("yetki")
            {
                need_root.push(p.clone());
            } else {
                state.delete_error_dialog = Some(crate::state::DeleteErrorDialogState {
                    failed_path: p.clone(),
                    error_msg: e,
                    remaining_paths: paths.iter().skip(idx + 1).cloned().collect(),
                    permanent,
                });
                break;
            }
        } else {
            deleted_undos.push(crate::backend::clipboard::UndoAction::Delete {
                path: p.clone(),
                was_dir: is_dir,
            });
        }
    }

    if !deleted_undos.is_empty() {
        let _ = crate::backend::clipboard::push_undo_batch(deleted_undos);
    }

    if !need_root.is_empty() {
        let mut args = vec!["-rf".to_string()];
        args.extend(need_root);
        let title = if permanent { "Kalıcı Olarak Sil (Root)" } else { "Dosyaları Sil (Root)" };

        // 1. Try cached root password first
        if crate::backend::fs_ops::get_root_password().is_some() {
            let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            if crate::backend::fs_ops::sudo_run_elevated("rm", &str_args).is_ok() {
                state.notify.success("Dosyalar root yetkisiyle silindi".into());
                state.refresh();
                return;
            }
        }

        // 2. Try pkexec (sadece küçük argüman listelerinde, 500+ dosyada E2BIG önlemek için sudo batch'e gider)
        if args.len() <= 500 {
            let status = std::process::Command::new("pkexec")
                .arg("rm")
                .args(&args)
                .status();
            if let Ok(s) = status {
                if s.success() {
                    state.notify.success("Dosyalar root yetkisiyle silindi".into());
                    state.refresh();
                    return;
                }
            }
        }

        // 2. Fallback to sudo password dialog
        state.pending_root_auth = Some(crate::state::RootAuthRequest {
            title: title.to_string(),
            cmd: "rm".to_string(),
            args,
            reload: true,
            auth_type: None,
        });
        state.pending_root_wrong = false;
    } else {
        state.refresh();
    }
}
