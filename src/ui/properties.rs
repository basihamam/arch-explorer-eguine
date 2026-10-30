// src/ui/properties.rs — Dosya/klasör özellikleri modalı
// Arch Explorer Svelte PropertiesModal tasarımından port edilmiştir.
use egui::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::ffi::CStr;
use crate::state::AppState;
use crate::theme::types::AexTheme;
use super::{icon, human_size};

#[derive(Clone)]
struct EditPermState {
    path: String,
    octal: String,
    rwx: String,
    recursive: bool,
    is_editing: bool,
    owner_str: String,
    group_str: String,
    is_editing_owner: bool,
    checksum: Option<(String, String)>, // (md5, sha256)
    is_calculating_hash: bool,
}

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState, path: &str) {
    let c  = &t.colors; let s = &t.shape; let sp = &t.spacing;
    let screen = ctx.screen_rect();

    egui::Area::new(Id::new("properties_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0,0,0,130));
            if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
        });

    let pw = 480.0_f32;
    let max_h = (screen.height() - 60.0).clamp(300.0, 680.0);
    let path_buf  = std::path::PathBuf::from(path);
    let file_name = path_buf.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let meta = fs::metadata(&path_buf).ok();
    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let is_link = fs::symlink_metadata(&path_buf)
        .map(|m| m.file_type().is_symlink()).unwrap_or(false);

    // Initial state setup for editing
    let prop_state_id = Id::new("properties_edit_state");
    let mut edit_state = ctx.data_mut(|d| {
        d.get_temp::<EditPermState>(prop_state_id).unwrap_or_else(|| {
            #[cfg(unix)]
            let (initial_octal, initial_rwx) = if let Some(ref m) = meta {
                let oct = format!("{:03o}", m.permissions().mode() & 0o777);
                let rwx = octal_to_rwx(&oct);
                (oct, rwx)
            } else {
                ("755".to_string(), "rwxr-xr-x".to_string())
            };
            #[cfg(not(unix))]
            let (initial_octal, initial_rwx) = ("755".to_string(), "rwxr-xr-x".to_string());

            let (owner_name, group_name) = get_owner_group(&path_buf, &meta);

            EditPermState {
                path: path.to_string(),
                octal: initial_octal,
                rwx: initial_rwx,
                recursive: false,
                is_editing: false,
                owner_str: owner_name,
                group_str: group_name,
                is_editing_owner: false,
                checksum: None,
                is_calculating_hash: false,
            }
        })
    });

    if edit_state.path != path {
        #[cfg(unix)]
        let (initial_octal, initial_rwx) = if let Some(ref m) = meta {
            let oct = format!("{:03o}", m.permissions().mode() & 0o777);
            let rwx = octal_to_rwx(&oct);
            (oct, rwx)
        } else {
            ("755".to_string(), "rwxr-xr-x".to_string())
        };
        #[cfg(not(unix))]
        let (initial_octal, initial_rwx) = ("755".to_string(), "rwxr-xr-x".to_string());

        let (owner_name, group_name) = get_owner_group(&path_buf, &meta);

        edit_state = EditPermState {
            path: path.to_string(),
            octal: initial_octal,
            rwx: initial_rwx,
            recursive: false,
            is_editing: false,
            owner_str: owner_name,
            group_str: group_name,
            is_editing_owner: false,
            checksum: None,
            is_calculating_hash: false,
        };
    }

    let mut state_changed = false;

    egui::Area::new(Id::new("properties_panel"))
        .fixed_pos(Pos2::new(screen.center().x - pw/2.0, (screen.center().y - max_h/2.0).max(20.0)))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(), rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow { offset: Vec2::new(0.0,8.0), blur: 24.0,
                    spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw);
                ui.set_max_height(max_h);

                // Header
                ui.horizontal(|ui| {
                    let ico = if is_link { icon(t, "symlink") }
                              else if is_dir { icon(t, "folder_open") }
                              else { icon(t, "file") };
                    ui.label(RichText::new(ico).color(c.accent()).size(28.0));
                    ui.add_space(sp.sm);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&file_name).color(c.text_primary())
                            .size(t.typography.size_base).strong());
                        ui.label(RichText::new(if is_dir { "Directory" } else if is_link { "Symbolic Link" } else { "File" })
                            .color(c.text_hint()).size(t.typography.size_xs));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("✕").color(c.text_hint()))
                            .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                            state.panels.close();
                        }
                    });
                });

                ui.add_space(sp.sm);
                let r = ui.min_rect();
                ui.painter().hline(r.left()..=r.right(), r.bottom(),
                    Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.sm);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(max_h - 110.0)
                    .show(ui, |ui| {
                        // Section: Genel Bilgiler
                        section_title(ui, t, "GENEL BİLGİLER");
                        egui::Grid::new("props_general_grid")
                            .num_columns(2)
                            .spacing([sp.lg, sp.xs])
                            .show(ui, |ui| {
                                row(ui, t, "Konum", |ui| {
                                    ui.horizontal(|ui| {
                                        let parent = path_buf.parent()
                                            .map(|p| p.to_string_lossy().to_string())
                                            .unwrap_or_default();
                                        ui.label(RichText::new(&parent).color(c.text_secondary())
                                            .size(t.typography.size_xs).monospace());
                                        if ui.add(egui::Button::new(
                                            RichText::new("📋").size(10.0).color(c.text_hint()))
                                            .fill(Color32::TRANSPARENT).stroke(Stroke::NONE))
                                            .on_hover_text("Konumu Kopyala").clicked() {
                                            crate::backend::clipboard::copy_to_system_clipboard_async(parent.clone());
                                            state.notify.success("Konum panoya kopyalandı".into());
                                        }
                                    });
                                });

                                row(ui, t, "Tam Yol", |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(path).color(c.text_secondary())
                                            .size(t.typography.size_xs).monospace());
                                        if ui.add(egui::Button::new(
                                            RichText::new("📋").size(10.0).color(c.text_hint()))
                                            .fill(Color32::TRANSPARENT).stroke(Stroke::NONE))
                                            .on_hover_text("Yolu Kopyala").clicked() {
                                            crate::backend::clipboard::copy_to_system_clipboard_async(path.to_string());
                                            state.notify.success("Tam yol panoya kopyalandı".into());
                                        }
                                    });
                                });

                                if is_link {
                                    if let Ok(target) = fs::read_link(&path_buf) {
                                        row_str(ui, t, "Bağlantı Hedefi", &target.to_string_lossy());
                                    }
                                }

                                if let Some(ref meta) = meta {
                                    if let Some(ext) = path_buf.extension() {
                                        row_str(ui, t, "Uzantı", &format!(".{}", ext.to_string_lossy().to_lowercase()));
                                    }

                                    let mime_id = egui::Id::new("props_mime_cache").with(path);
                                    let mime: String = ui.ctx().data_mut(|d| {
                                        d.get_temp(mime_id).unwrap_or_else(|| {
                                            let detected = std::process::Command::new("file")
                                                .args(["--brief", "--mime-type", path])
                                                .output().ok()
                                                .and_then(|o| String::from_utf8(o.stdout).ok())
                                                .map(|s| s.trim().to_string())
                                                .unwrap_or_else(|| if is_dir { "inode/directory".into() } else { "unknown".into() });
                                            d.insert_temp(mime_id, detected.clone());
                                            detected
                                        })
                                    });
                                    row_str(ui, t, "MIME Türü", &mime);
                                }
                            });

                        ui.add_space(sp.sm);
                        divider(ui, c);
                        ui.add_space(sp.sm);

                        // Section: Boyut & İçerik
                        section_title(ui, t, "BOYUT & İÇERİK");
                        egui::Grid::new("props_size_grid")
                            .num_columns(2)
                            .spacing([sp.lg, sp.xs])
                            .show(ui, |ui| {
                                if let Some(ref meta) = meta {
                                    let size_str = if is_dir {
                                        if let Some(&sz) = state.du_sizes.get(path) {
                                            format!("{} ({} bayt)", human_size(sz), sz)
                                        } else {
                                            state.pending_du_request = Some(path.to_string());
                                            "Hesaplanıyor…".to_string()
                                        }
                                    } else {
                                        format!("{} ({} bayt)", human_size(meta.len()), meta.len())
                                    };
                                    row_str(ui, t, "Boyut", &size_str);

                                    if is_dir {
                                        let count = fs::read_dir(&path_buf)
                                            .map(|d| d.count()).unwrap_or(0);
                                        row_str(ui, t, "İçerik", &format!("{} öğe (bu dizinde)", count));
                                    }
                                }
                            });

                        ui.add_space(sp.sm);
                        divider(ui, c);
                        ui.add_space(sp.sm);

                        // Section: Zaman Damgaları
                        section_title(ui, t, "ZAMAN DAMGALARI");
                        egui::Grid::new("props_time_grid")
                            .num_columns(2)
                            .spacing([sp.lg, sp.xs])
                            .show(ui, |ui| {
                                if let Some(ref meta) = meta {
                                    if let Ok(modified) = meta.modified() {
                                        let dt: chrono::DateTime<chrono::Local> = modified.into();
                                        row_str(ui, t, "Değiştirilme", &dt.format("%Y-%m-%d %H:%M:%S").to_string());
                                    }
                                    if let Ok(created) = meta.created() {
                                        let dt: chrono::DateTime<chrono::Local> = created.into();
                                        row_str(ui, t, "Oluşturulma", &dt.format("%Y-%m-%d %H:%M:%S").to_string());
                                    }
                                    if let Ok(accessed) = meta.accessed() {
                                        let dt: chrono::DateTime<chrono::Local> = accessed.into();
                                        row_str(ui, t, "Son Erişim", &dt.format("%Y-%m-%d %H:%M:%S").to_string());
                                    }
                                }
                            });

                        ui.add_space(sp.sm);
                        divider(ui, c);
                        ui.add_space(sp.sm);

                        // Section: İzinler ve Sahiplik (Interactive Svelte Design)
                        section_title(ui, t, "İZİNLER VE SAHİPLİK");

                        #[cfg(unix)]
                        {
                            let current_mode = meta.as_ref().map(|m| m.permissions().mode()).unwrap_or(0o755);
                            let current_octal_str = format!("{:03o}", current_mode & 0o777);
                            let current_rwx_str = unix_perm_str(current_mode);

                            if !edit_state.is_editing {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("İzinler:").color(c.text_secondary()).size(t.typography.size_xs).strong());
                                    ui.add_space(sp.sm);
                                    ui.label(RichText::new(&current_rwx_str).color(c.text_primary()).size(t.typography.size_xs).monospace());
                                    ui.label(RichText::new(format!("({})", current_octal_str)).color(c.accent()).size(t.typography.size_xs).monospace());
                                    if meta.as_ref().map(|m| m.permissions().readonly()).unwrap_or(false) {
                                        ui.label(RichText::new("Yalnızca Okunur").color(c.warning()).size(t.typography.size_xs));
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.add(egui::Button::new(
                                            RichText::new("✏ Değiştir").color(c.accent()).size(t.typography.size_xs))
                                            .fill(c.bg_surface0()).stroke(Stroke::new(1.0, c.border()))
                                            .rounding(Rounding::same(s.radius_sm))).clicked() {
                                            edit_state.is_editing = true;
                                            edit_state.octal = current_octal_str.clone();
                                            edit_state.rwx = octal_to_rwx(&edit_state.octal);
                                            state_changed = true;
                                        }
                                    });
                                });
                            } else {
                                // Interactive permissions editor
                                egui::Frame {
                                    fill: c.bg_surface0(),
                                    rounding: Rounding::same(s.radius_sm),
                                    stroke: Stroke::new(1.0, c.border()),
                                    inner_margin: Margin::same(sp.md),
                                    ..Default::default()
                                }.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Oktal:").color(c.text_secondary()).size(t.typography.size_xs).strong());
                                        let oct_edit = ui.add_sized(
                                            Vec2::new(50.0, 22.0),
                                            egui::TextEdit::singleline(&mut edit_state.octal)
                                                .font(FontId::new(t.typography.size_xs, FontFamily::Monospace))
                                        );
                                        if oct_edit.changed() {
                                            edit_state.octal = edit_state.octal.chars().filter(|ch| ('0'..='7').contains(ch)).take(3).collect();
                                            if edit_state.octal.len() == 3 {
                                                edit_state.rwx = octal_to_rwx(&edit_state.octal);
                                            }
                                            state_changed = true;
                                        }

                                        ui.add_space(sp.md);

                                        ui.label(RichText::new("RWX:").color(c.text_secondary()).size(t.typography.size_xs).strong());
                                        let rwx_edit = ui.add_sized(
                                            Vec2::new(95.0, 22.0),
                                            egui::TextEdit::singleline(&mut edit_state.rwx)
                                                .font(FontId::new(t.typography.size_xs, FontFamily::Monospace))
                                        );
                                        if rwx_edit.changed() {
                                            edit_state.rwx = edit_state.rwx.to_lowercase().chars().filter(|ch| ['r','w','x','-'].contains(ch)).take(9).collect();
                                            if edit_state.rwx.len() == 9 {
                                                edit_state.octal = rwx_to_octal(&edit_state.rwx);
                                            }
                                            state_changed = true;
                                        }
                                    });

                                    ui.add_space(sp.xs);

                                    // Preset chips
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Hazır:").color(c.text_hint()).size(9.5));
                                        for (preset, label) in [("755", "755 (Standart)"), ("644", "644 (Dosya)"), ("700", "700 (Özel)"), ("777", "777 (Tam Yetki)")] {
                                            let is_act = edit_state.octal == preset;
                                            let bg = if is_act { c.accent() } else { c.bg_surface1() };
                                            let fg = if is_act { c.bg_crust() } else { c.text_secondary() };
                                            if ui.add(egui::Button::new(RichText::new(label).color(fg).size(9.0))
                                                .fill(bg).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() {
                                                edit_state.octal = preset.to_string();
                                                edit_state.rwx = octal_to_rwx(preset);
                                                state_changed = true;
                                            }
                                        }
                                    });

                                    ui.add_space(sp.xs);

                                    // 3-Column matrix (Sahip, Grup, Diğer)
                                    ui.horizontal(|ui| {
                                        let col_w = (ui.available_width() - sp.sm * 2.0) / 3.0;

                                        // Sahip
                                        ui.allocate_ui(Vec2::new(col_w, 48.0), |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.label(RichText::new("👤 Sahip").color(c.text_secondary()).size(10.0).strong());
                                                ui.horizontal(|ui| {
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 0, 'r', &mut state_changed);
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 1, 'w', &mut state_changed);
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 2, 'x', &mut state_changed);
                                                });
                                            });
                                        });

                                        // Grup
                                        ui.allocate_ui(Vec2::new(col_w, 48.0), |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.label(RichText::new("👥 Grup").color(c.text_secondary()).size(10.0).strong());
                                                ui.horizontal(|ui| {
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 3, 'r', &mut state_changed);
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 4, 'w', &mut state_changed);
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 5, 'x', &mut state_changed);
                                                });
                                            });
                                        });

                                        // Diğer
                                        ui.allocate_ui(Vec2::new(col_w, 48.0), |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.label(RichText::new("🌐 Diğer").color(c.text_secondary()).size(10.0).strong());
                                                ui.horizontal(|ui| {
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 6, 'r', &mut state_changed);
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 7, 'w', &mut state_changed);
                                                    render_perm_toggle(ui, c, &mut edit_state.rwx, 8, 'x', &mut state_changed);
                                                });
                                            });
                                        });
                                    });

                                    ui.add_space(sp.xs);

                                    // Readable explanation
                                    let expl = explain_permissions(&edit_state.octal, is_dir);
                                    ui.label(RichText::new(format!("👤 {}", expl[0])).color(c.text_hint()).size(9.0));
                                    ui.label(RichText::new(format!("👥 {}", expl[1])).color(c.text_hint()).size(9.0));
                                    ui.label(RichText::new(format!("🌐 {}", expl[2])).color(c.text_hint()).size(9.0));

                                    if is_dir {
                                        ui.add_space(2.0);
                                        ui.checkbox(&mut edit_state.recursive, RichText::new("Alt dizin ve dosyalara da uygula (-R)").size(10.0).color(c.text_secondary()));
                                    }

                                    ui.add_space(sp.xs);
                                    ui.horizontal(|ui| {
                                        if ui.add(egui::Button::new(
                                            RichText::new("✓ İzinleri Kaydet").color(c.bg_crust()).size(t.typography.size_xs).strong())
                                            .fill(c.accent()).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() {
                                            if let Ok(oct_val) = u32::from_str_radix(&edit_state.octal, 8) {
                                                apply_permissions(path, oct_val, edit_state.recursive, state);
                                                edit_state.is_editing = false;
                                                state_changed = true;
                                            } else {
                                                state.notify.error("Geçersiz oktal izin!".into());
                                            }
                                        }

                                        if ui.add(egui::Button::new(
                                            RichText::new("✕ İptal").color(c.text_hint()).size(t.typography.size_xs))
                                            .fill(c.bg_surface1()).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() {
                                            edit_state.is_editing = false;
                                            state_changed = true;
                                        }
                                    });
                                });
                            }
                        }

                        ui.add_space(sp.sm);

                        // Ownership display & edit
                        let (owner_name, group_name) = get_owner_group(&path_buf, &meta);
                        let (uid, gid) = get_owner_uids(&meta);

                        if !edit_state.is_editing_owner {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Sahip / Grup:").color(c.text_secondary()).size(t.typography.size_xs).strong());
                                ui.add_space(sp.sm);
                                ui.label(RichText::new(format!("{} : {}", owner_name, group_name)).color(c.text_primary()).size(t.typography.size_xs).strong());
                                ui.label(RichText::new(format!("({}:{})", uid, gid)).color(c.text_hint()).size(t.typography.size_xs).monospace());

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("✏ Değiştir").color(c.accent()).size(t.typography.size_xs))
                                        .fill(c.bg_surface0()).stroke(Stroke::new(1.0, c.border()))
                                        .rounding(Rounding::same(s.radius_sm))).clicked() {
                                        edit_state.is_editing_owner = true;
                                        edit_state.owner_str = owner_name.clone();
                                        edit_state.group_str = group_name.clone();
                                        state_changed = true;
                                    }
                                });
                            });
                        } else {
                            egui::Frame {
                                fill: c.bg_surface0(),
                                rounding: Rounding::same(s.radius_sm),
                                stroke: Stroke::new(1.0, c.border()),
                                inner_margin: Margin::same(sp.md),
                                ..Default::default()
                            }.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Sahip:").color(c.text_secondary()).size(t.typography.size_xs).strong());
                                    ui.add_sized(Vec2::new(100.0, 22.0), egui::TextEdit::singleline(&mut edit_state.owner_str).font(FontId::new(t.typography.size_xs, FontFamily::Monospace)));

                                    ui.add_space(sp.sm);

                                    ui.label(RichText::new("Grup:").color(c.text_secondary()).size(t.typography.size_xs).strong());
                                    ui.add_sized(Vec2::new(100.0, 22.0), egui::TextEdit::singleline(&mut edit_state.group_str).font(FontId::new(t.typography.size_xs, FontFamily::Monospace)));
                                });

                                if is_dir {
                                    ui.add_space(2.0);
                                    ui.checkbox(&mut edit_state.recursive, RichText::new("Alt dizin ve dosyalara da uygula (-R)").size(10.0).color(c.text_secondary()));
                                }

                                ui.add_space(sp.xs);
                                ui.horizontal(|ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("✓ Sahipliği Kaydet").color(c.bg_crust()).size(t.typography.size_xs).strong())
                                        .fill(c.accent()).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() {
                                        apply_owner(path, &edit_state.owner_str, &edit_state.group_str, edit_state.recursive, state);
                                        edit_state.is_editing_owner = false;
                                        state_changed = true;
                                    }

                                    if ui.add(egui::Button::new(
                                        RichText::new("✕ İptal").color(c.text_hint()).size(t.typography.size_xs))
                                        .fill(c.bg_surface1()).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() {
                                        edit_state.is_editing_owner = false;
                                        state_changed = true;
                                    }
                                });
                            });
                        }

                        // Checksum (Sadece dosyalar için)
                        if !is_dir {
                            ui.add_space(sp.sm);
                            divider(ui, c);
                            ui.add_space(sp.sm);

                            section_title(ui, t, "SAĞLAMA TOPLAMI (CHECKSUM)");
                            if let Some((ref md5, ref sha256)) = edit_state.checksum {
                                egui::Frame {
                                    fill: c.bg_surface0(),
                                    rounding: Rounding::same(s.radius_sm),
                                    stroke: Stroke::new(1.0, c.border()),
                                    inner_margin: Margin::same(sp.sm),
                                    ..Default::default()
                                }.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("MD5:").color(c.text_secondary()).size(9.5).strong());
                                        ui.label(RichText::new(md5).color(c.text_primary()).size(9.5).monospace());
                                        if ui.add(egui::Button::new(RichText::new("📋").size(10.0).color(c.text_hint())).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).on_hover_text("MD5 Kopyala").clicked() {
                                            crate::backend::clipboard::copy_to_system_clipboard_async(md5.clone());
                                            state.notify.success("MD5 panoya kopyalandı".into());
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("SHA256:").color(c.text_secondary()).size(9.5).strong());
                                        let short_sha = if sha256.len() > 24 { format!("{}…{}", &sha256[..12], &sha256[sha256.len()-12..]) } else { sha256.clone() };
                                        ui.label(RichText::new(short_sha).color(c.text_primary()).size(9.5).monospace()).on_hover_text(sha256);
                                        if ui.add(egui::Button::new(RichText::new("📋").size(10.0).color(c.text_hint())).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).on_hover_text("SHA256 Kopyala").clicked() {
                                            crate::backend::clipboard::copy_to_system_clipboard_async(sha256.clone());
                                            state.notify.success("SHA256 panoya kopyalandı".into());
                                        }
                                    });
                                });
                            } else if edit_state.is_calculating_hash {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Hesaplanıyor…").color(c.accent()).size(t.typography.size_xs));
                                });
                            } else {
                                if ui.add(egui::Button::new(
                                    RichText::new("MD5 / SHA256 Doğrula").color(c.text_secondary()).size(t.typography.size_xs))
                                    .fill(c.bg_surface0()).stroke(Stroke::new(1.0, c.border()))
                                    .rounding(Rounding::same(s.radius_sm))).clicked() {
                                    edit_state.is_calculating_hash = true;
                                    state.pending_checksum_request = Some(path.to_string());
                                    state_changed = true;
                                }
                            }
                        }
                    });

                ui.add_space(sp.sm);
                let r = ui.min_rect();
                ui.painter().hline(r.left()..=r.right(), r.bottom(),
                    Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.sm);

                // Footer buttons
                ui.horizontal(|ui| {
                    let term_path = if is_dir { path.to_string() }
                        else { path_buf.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default() };
                    if ui.add(egui::Button::new(
                        RichText::new(format!("{} Terminal", icon(t, "terminal")))
                            .color(c.text_secondary()).size(t.typography.size_xs))
                        .fill(c.bg_surface0()).stroke(Stroke::new(1.0_f32, c.border()))
                        .rounding(Rounding::same(s.radius_sm))).clicked() {
                        state.open_terminal(Some(term_path));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(
                            RichText::new("Tamam").color(c.bg_crust()).size(t.typography.size_sm).strong())
                            .fill(c.accent()).stroke(Stroke::NONE)
                            .rounding(Rounding::same(s.radius_sm))).clicked() {
                            state.panels.close();
                        }
                    });
                });
            });
        });

    // Check if background checksum finished
    if let Some((chk_p, md5, sha256)) = state.pending_checksum.take() {
        if chk_p == path {
            edit_state.checksum = Some((md5, sha256));
            edit_state.is_calculating_hash = false;
            state_changed = true;
        }
    }

    if state_changed {
        ctx.data_mut(|d| d.insert_temp(prop_state_id, edit_state));
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_title(ui: &mut Ui, t: &AexTheme, text: &str) {
    ui.label(RichText::new(text).color(t.colors.text_hint()).size(9.5).strong());
    ui.add_space(2.0);
}

fn divider(ui: &mut Ui, c: &crate::theme::types::AexColors) {
    let r = ui.min_rect();
    ui.painter().hline(r.left()..=r.right(), r.bottom(), Stroke::new(1.0_f32, c.border()));
}

fn row(ui: &mut Ui, t: &AexTheme, label: &str, content: impl FnOnce(&mut Ui)) {
    ui.label(RichText::new(label).color(t.colors.text_secondary())
        .size(t.typography.size_xs).strong());
    content(ui);
    ui.end_row();
}

fn row_str(ui: &mut Ui, t: &AexTheme, label: &str, value: &str) {
    row(ui, t, label, |ui| {
        ui.label(RichText::new(value).color(t.colors.text_primary())
            .size(t.typography.size_xs));
    });
}

fn render_perm_toggle(ui: &mut Ui, c: &crate::theme::types::AexColors, rwx: &mut String, idx: usize, char_target: char, changed: &mut bool) {
    let has_perm = rwx.chars().nth(idx) == Some(char_target);
    let bg = if has_perm { c.accent() } else { c.bg_surface1() };
    let fg = if has_perm { c.bg_crust() } else { c.text_disabled() };
    let btn = ui.add_sized(Vec2::new(24.0, 22.0), egui::Button::new(RichText::new(char_target.to_string()).color(fg).size(11.0).strong())
        .fill(bg).stroke(Stroke::NONE).rounding(Rounding::same(3.0)));
    if btn.clicked() {
        let mut chars: Vec<char> = format!("{: <9}", rwx).chars().take(9).collect();
        if chars[idx] == char_target {
            chars[idx] = '-';
        } else {
            chars[idx] = char_target;
        }
        *rwx = chars.into_iter().collect();
        *changed = true;
    }
}

fn octal_to_rwx(oct: &str) -> String {
    let s = format!("{:0>3}", oct.trim());
    let mut rwx = String::new();
    let chars = s.chars().collect::<Vec<_>>();
    let lookup = [
        "---", "--x", "-w-", "-wx",
        "r--", "r-x", "rw-", "rwx"
    ];
    for &ch in chars.iter().rev().take(3).collect::<Vec<_>>().iter().rev() {
        let n = ch.to_digit(8).unwrap_or(0) as usize;
        rwx.push_str(lookup[n.min(7)]);
    }
    rwx
}

fn rwx_to_octal(rwx: &str) -> String {
    let s = format!("{: <9}", rwx);
    let chars: Vec<char> = s.chars().collect();
    let mut oct = String::new();
    for i in (0..9).step_by(3) {
        let mut val = 0;
        if chars.get(i) == Some(&'r') { val += 4; }
        if chars.get(i+1) == Some(&'w') { val += 2; }
        if chars.get(i+2) == Some(&'x') { val += 1; }
        oct.push_str(&val.to_string());
    }
    oct
}

fn explain_permissions(oct: &str, is_dir: bool) -> [String; 3] {
    let s = format!("{:0>3}", oct.trim());
    let chars: Vec<char> = s.chars().collect();
    let exp = |ch: char, role: &str| -> String {
        let v = ch.to_digit(8).unwrap_or(0);
        let mut acts = Vec::new();
        if v & 4 != 0 { acts.push("Okuma (r)"); }
        if v & 2 != 0 { acts.push("Yazma (w)"); }
        if v & 1 != 0 { acts.push(if is_dir { "Giriş (x)" } else { "Çalıştırma (x)" }); }
        if acts.is_empty() {
            format!("{}: Yetki Yok (—)", role)
        } else {
            format!("{}: {}", role, acts.join(", "))
        }
    };

    [
        exp(chars[0], "Sahip"),
        exp(chars[1], "Grup"),
        exp(chars[2], "Diğer"),
    ]
}

#[cfg(unix)]
fn unix_perm_str(mode: u32) -> String {
    let bits = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),
    ];
    let mut s = String::with_capacity(9);
    for (bit, ch) in &bits {
        s.push(if mode & bit != 0 { *ch } else { '-' });
    }
    s
}

fn get_owner_group(_path: &std::path::Path, meta: &Option<fs::Metadata>) -> (String, String) {
    #[cfg(unix)]
    if let Some(ref m) = meta {
        let uid = m.uid();
        let gid = m.gid();

        let owner = unsafe {
            let pw = libc::getpwuid(uid);
            if !pw.is_null() && !(*pw).pw_name.is_null() {
                CStr::from_ptr((*pw).pw_name).to_string_lossy().to_string()
            } else {
                uid.to_string()
            }
        };

        let group = unsafe {
            let gr = libc::getgrgid(gid);
            if !gr.is_null() && !(*gr).gr_name.is_null() {
                CStr::from_ptr((*gr).gr_name).to_string_lossy().to_string()
            } else {
                gid.to_string()
            }
        };
        return (owner, group);
    }
    ("unknown".into(), "unknown".into())
}

fn get_owner_uids(meta: &Option<fs::Metadata>) -> (u32, u32) {
    #[cfg(unix)]
    if let Some(ref m) = meta {
        return (m.uid(), m.gid());
    }
    (0, 0)
}

fn apply_permissions(path: &str, mode: u32, recursive: bool, state: &mut AppState) {
    #[cfg(unix)]
    {
        let is_dir = std::path::Path::new(path).is_dir();
        let res = if recursive && is_dir {
            std::process::Command::new("chmod")
                .args(["-R", &format!("{:o}", mode), path])
                .output()
                .map(|out| out.status.success())
                .unwrap_or(false)
        } else {
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).is_ok()
        };

        if res {
            state.notify.success("İzinler başarıyla güncellendi".into());
        } else {
            // Root gerekebilir
            state.pending_root_auth = Some(crate::state::RootAuthRequest {
                title: "İzin Değişikliği İçin Root Yetkisi".into(),
                cmd: "chmod".into(),
                args: if recursive && is_dir { vec!["-R".into(), format!("{:o}", mode), path.to_string()] } else { vec![format!("{:o}", mode), path.to_string()] },
                reload: true,
                auth_type: None,
            });
        }
    }
}

fn apply_owner(path: &str, owner: &str, group: &str, recursive: bool, state: &mut AppState) {
    let target = if group.is_empty() { owner.to_string() } else { format!("{}:{}", owner, group) };
    let is_dir = std::path::Path::new(path).is_dir();
    let mut cmd = std::process::Command::new("chown");
    if recursive && is_dir {
        cmd.arg("-R");
    }
    cmd.arg(&target).arg(path);

    let res = cmd.output().map(|out| out.status.success()).unwrap_or(false);
    if res {
        state.notify.success("Sahiplik başarıyla güncellendi".into());
    } else {
        // Root auth dialog trigger
        state.pending_root_auth = Some(crate::state::RootAuthRequest {
            title: "Sahiplik Değişikliği İçin Root Yetkisi".into(),
            cmd: "chown".into(),
            args: if recursive && is_dir { vec!["-R".into(), target, path.to_string()] } else { vec![target, path.to_string()] },
            reload: true,
            auth_type: None,
        });
    }
}
