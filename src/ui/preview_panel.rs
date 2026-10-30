// src/ui/preview_panel.rs
// Dosya önizleme — text, image (base64/svg/gif), dir info, binary hex, disk summary
use egui::*;
use egui::load::SizedTexture;
use crate::backend::preview::{get_file_preview, FilePreview};
use crate::theme::types::AexTheme;
use crate::state::AppState;
use super::{icon, human_size};

pub fn render(ui: &mut Ui, t: &AexTheme, state: &mut AppState, tx: &std::sync::mpsc::Sender<crate::app::BackendMsg>) {
    // Panel çok darsa render etme — negatif size panic'i önle
    if ui.available_width() < 20.0 || ui.available_height() < 20.0 { return; }

    let c  = &t.colors;
    let sp = &t.spacing;

    // Sidebar/Content sol border
    let rect = ui.available_rect_before_wrap();
    ui.painter().vline(
        rect.left(),
        rect.top()..=rect.bottom(),
        Stroke::new(1.0_f32, c.border()),
    );

    egui::Frame {
        inner_margin: Margin::symmetric(sp.md, sp.sm),
        fill: Color32::TRANSPARENT,
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.spacing_mut().scroll.bar_width = 8.0;
        ui.spacing_mut().scroll.handle_min_length = 28.0;
        ui.set_max_width(ui.available_width());
        egui::ScrollArea::both()
            .id_source("preview_panel_main_scroll")
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                let selected: Vec<String> = state.selection.paths();

                if selected.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PREVIEW")
                                .color(c.text_hint())
                                .size(t.typography.size_xs)
                                .strong()
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button(icon(t, "close")).clicked() {
                                state.media_player.stop();
                                state.panels.close();
                            }
                        });
                    });
                    ui.add_space(sp.sm);
                    empty_preview(ui, t, state);
                    return;
                }

                if selected.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PREVIEW")
                                .color(c.text_hint())
                                .size(t.typography.size_xs)
                                .strong()
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button(icon(t, "close")).clicked() {
                                state.media_player.stop();
                                state.panels.close();
                            }
                        });
                    });
                    ui.add_space(sp.sm);
                    ui.vertical_centered(|ui| {
                        ui.add_space(sp.xl);
                        ui.label(RichText::new(icon(t, "file")).color(c.text_disabled()).size(40.0));
                        ui.add_space(sp.sm);
                        ui.label(RichText::new(format!("{} öğe seçildi", selected.len())).color(c.text_hint()).size(t.typography.size_sm));
                        ui.label(RichText::new("Çoklu seçimde önizleme gösterilmez").color(c.text_hint()).size(t.typography.size_xs));
                    });
                    return;
                }

                let path = selected[0].clone();

                // Farklı dosya seçildiyse texture cache'ini temizle
                let tex_path_id = Id::new("preview_tex_path");
                let last_path: Option<String> = ui.ctx().data_mut(|d| d.get_temp(tex_path_id));
                if last_path.as_deref() != Some(&path) {
                    ui.ctx().data_mut(|d| {
                        if let Some(ref lp) = last_path {
                            d.remove::<GifCache>(Id::new(format!("gif_cache_{}", lp)));
                            d.remove::<std::time::Instant>(Id::new(format!("gif_start_{}", lp)));
                        }
                        d.insert_temp(tex_path_id, path.clone());
                        d.remove::<egui::TextureHandle>(Id::new("preview_tex_handle"));
                        d.remove::<egui::TextureHandle>(Id::new("live_video_tex_cache"));
                    });
                }

                // 1. Cache kontrolü
                let cached_opt = state.preview_cache.get(&path).cloned();
                if let Some(cached_res) = cached_opt {
                    match cached_res {
                        Ok(preview) => {
                            render_preview(ui, t, &path, &preview, state);
                        }
                        Err(e) => {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("PREVIEW")
                                        .color(c.text_hint())
                                        .size(t.typography.size_xs)
                                        .strong()
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.small_button(icon(t, "close")).clicked() {
                                        state.media_player.stop();
                                        state.panels.close();
                                    }
                                });
                            });
                            ui.add_space(sp.sm);
                            ui.label(
                                RichText::new(format!("{} {}", icon(t, "error"), e))
                                    .color(c.danger())
                                    .size(t.typography.size_sm),
                            );
                        }
                    }
                    return;
                }

                // 2. Yükleniyor durumu
                if state.preview_loading.as_deref() == Some(&path) {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PREVIEW")
                                .color(c.text_hint())
                                .size(t.typography.size_xs)
                                .strong()
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button(icon(t, "close")).clicked() {
                                state.panels.close();
                            }
                        });
                    });
                    ui.add_space(sp.xl);
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.add_space(sp.sm);
                        ui.label(
                            RichText::new("Önizleme yükleniyor…")
                                .color(c.text_hint())
                                .size(t.typography.size_sm)
                        );
                    });
                    return;
                }

                // 3. Arka planda async yükleme başlat (UI thread asla bloklanmaz!)
                state.preview_loading = Some(path.clone());
                let tx_clone = tx.clone();
                let path_clone = path.clone();
                std::thread::spawn(move || {
                    let res = crate::backend::preview::get_file_preview(path_clone.clone());
                    tx_clone.send(crate::app::BackendMsg::PreviewLoaded {
                        path: path_clone,
                        result: res,
                    }).ok();
                });

                // İlk karede placeholder göster
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PREVIEW")
                            .color(c.text_hint())
                            .size(t.typography.size_xs)
                            .strong()
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(icon(t, "close")).clicked() {
                            state.panels.close();
                        }
                    });
                });
                ui.add_space(sp.xl);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(sp.sm);
                    ui.label(
                        RichText::new("Önizleme yükleniyor…")
                            .color(c.text_hint())
                            .size(t.typography.size_sm)
                    );
                });
            });
    });
}

// ── Meta Bilgisi Çıkarma ───────────────────────────────────────────────────────

fn get_file_meta_info(path: &str, state: &AppState) -> (String, String, String, u64) {
    if let Some(entry) = state.dir.entries.iter().find(|e| e.path == path) {
        return (entry.modified.clone(), entry.permissions.clone(), entry.extension.clone(), entry.size);
    }
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        let real_meta = std::fs::metadata(path).unwrap_or(meta.clone());
        let modified = real_meta.modified().ok()
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "—".into());
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let chars = ['x', 'w', 'r'];
        let mut perm = String::with_capacity(10);
        if real_meta.is_dir() { perm.push('d'); }
        else if meta.file_type().is_symlink() { perm.push('l'); }
        else { perm.push('-'); }
        for shift in [6u32, 3, 0] {
            for (i, &c) in chars.iter().rev().enumerate() {
                if mode & (1 << (shift + i as u32)) != 0 {
                    perm.push(c);
                } else {
                    perm.push('-');
                }
            }
        }
        let ext = std::path::Path::new(path).extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let sz = if real_meta.is_dir() { 0 } else { real_meta.len() };
        (modified, perm, ext, sz)
    } else {
        ("—".into(), "—".into(), "".into(), 0)
    }
}

fn kind_icon_str(kind: &str, is_dir: bool) -> &'static str {
    if is_dir {
        "📁"
    } else {
        match kind {
            "image"  => "🖼",
            "video"  => "🎬",
            "audio"  => "🎵",
            "text"   => "📝",
            "binary" => "⬡",
            "dir"    => "📁",
            _        => "📄",
        }
    }
}

fn meta_row(ui: &mut Ui, t: &AexTheme, key: &str, val: &str, is_mono: bool) {
    let c = &t.colors;
    let is_long = val.len() > 18 || val.contains('\n') || val.contains('/') || val.contains('\\');

    if is_long {
        ui.vertical(|ui| {
            ui.label(RichText::new(key).color(c.text_hint()).size(10.0));
            let mut txt = RichText::new(val).color(c.text_secondary()).size(10.0);
            if is_mono {
                txt = txt.font(FontId::new(9.0, FontFamily::Monospace));
            }
            ui.add(egui::Label::new(txt).wrap_mode(egui::TextWrapMode::Wrap));
        });
    } else {
        ui.horizontal(|ui| {
            ui.allocate_ui(Vec2::new(56.0, 15.0), |ui| {
                ui.label(RichText::new(key).color(c.text_hint()).size(10.0));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut txt = RichText::new(val).color(c.text_secondary()).size(10.0);
                if is_mono {
                    txt = txt.font(FontId::new(9.0, FontFamily::Monospace));
                }
                ui.add(egui::Label::new(txt).wrap_mode(egui::TextWrapMode::Truncate));
            });
        });
    }
}

fn badge_widget(ui: &mut Ui, t: &AexTheme, text: &str, is_accent: bool) {
    let c = &t.colors;
    let bg = if is_accent {
        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 35)
    } else {
        c.bg_crust()
    };
    let stroke_color = if is_accent {
        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 90)
    } else {
        c.border()
    };
    let text_color = if is_accent { c.accent() } else { c.text_hint() };

    egui::Frame {
        fill: bg,
        rounding: Rounding::same(t.shape.radius_sm),
        stroke: Stroke::new(1.0_f32, stroke_color),
        inner_margin: Margin::symmetric(5.0, 1.5),
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.label(
            RichText::new(text)
                .color(text_color)
                .size(9.0)
                .strong()
        );
    });
}

// ── Render Preview ────────────────────────────────────────────────────────────

fn render_preview(ui: &mut Ui, t: &AexTheme, path: &str, preview: &FilePreview, state: &mut AppState) {
    if ui.available_width() < 10.0 { return; }

    let c  = &t.colors;
    let sp = &t.spacing;

    let filename = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let (modified, permissions, file_ext, fs_size) = get_file_meta_info(path, state);
    let size_bytes = if preview.kind == "dir" { state.du_sizes.get(path).copied().unwrap_or(0) } else { if preview.size > 0 { preview.size } else { fs_size } };
    let is_dir = preview.kind == "dir";

    // ── 1. Header (Kind Icon + Title + Badges + Close) ──
    let is_exec = state.dir.entries.iter().find(|e| e.path == path).map(|e| e.is_executable).unwrap_or_else(|| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0 && !m.is_dir()).unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            false
        }
    });

    let (icon_str, icon_col) = if let Some(e) = state.dir.entries.iter().find(|e| e.path == path) {
        (crate::ui::file_icon(t, e), crate::ui::icon_color_for_entry(t, e))
    } else {
        let dummy = crate::backend::fs_ops::FileEntry {
            name: filename.clone(),
            path: path.to_string(),
            is_dir,
            size: size_bytes,
            modified: modified.clone(),
            created: String::new(),
            permissions: permissions.clone(),
            mime_type: preview.mime_type.clone(),
            extension: file_ext.clone(),
            hidden: filename.starts_with('.'),
            is_symlink: false,
            is_executable: is_exec,
        };
        (crate::ui::file_icon(t, &dummy), crate::ui::icon_color_for_entry(t, &dummy))
    };

    // ── 1. Top Bar (PREVIEW + Close) ──
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("PREVIEW")
                .color(c.text_hint())
                .size(t.typography.size_xs)
                .strong()
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button(icon(t, "close")).clicked() {
                state.media_player.stop();
                state.panels.close();
            }
        });
    });

    ui.add_space(sp.xs);

    // ── 2. Header (Kind Icon on Left + Title & Badges on Right, like arch-explorer-source) ──
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(icon_str)
                .size(22.0)
                .color(icon_col)
        );
        ui.add_space(sp.xs);

        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(&filename)
                        .color(c.text_primary())
                        .size(t.typography.size_sm)
                        .strong()
                ).wrap_mode(egui::TextWrapMode::Wrap)
            ).on_hover_text(path);

            ui.add_space(2.0);

            ui.horizontal(|ui| {
                if is_exec {
                    badge_widget(ui, t, "EXEC", true);
                }

                let badge_text = if !file_ext.is_empty() {
                    file_ext.to_uppercase()
                } else if is_dir {
                    "DIR".to_string()
                } else if is_exec {
                    "BINARY".to_string()
                } else {
                    let sub = preview.mime_type.split('/').nth(1).unwrap_or("FILE");
                    sub.to_uppercase()
                };
                badge_widget(ui, t, &badge_text, false);

                if let Some(ref lang) = preview.language {
                    badge_widget(ui, t, &lang.to_uppercase(), true);
                }
            });
        });
    });

    ui.add_space(sp.sm);

    // ── 2. Metadata Tablosu ──
    ui.vertical(|ui| {
        // Size
        let size_str = if is_dir {
            if let Some(&sz) = state.du_sizes.get(path) {
                human_size(sz)
            } else {
                format!("{} öğe", preview.line_count.unwrap_or(0))
            }
        } else {
            human_size(size_bytes)
        };
        meta_row(ui, t, "Size", &size_str, false);

        // Modified
        if !modified.is_empty() {
            meta_row(ui, t, "Modified", &modified, false);
        }

        // Perms
        if !permissions.is_empty() {
            meta_row(ui, t, "Perms", &permissions, true);
        }

        // Dir item count
        if is_dir {
            if let Some(cnt) = preview.line_count {
                meta_row(ui, t, "İçerik", &format!("{} öğe", cnt), false);
            }
        }
    });

    ui.add_space(sp.sm);

    // ── 3. Ayrım Çizgisi ──
    let sep_rect = ui.available_rect_before_wrap();
    if sep_rect.width() > 0.0 {
        ui.painter().hline(
            sep_rect.left()..=sep_rect.right(),
            sep_rect.top(),
            Stroke::new(1.0_f32, c.border()),
        );
    }
    ui.add_space(sp.sm);

    // ── 4. İçerik Bölümü ──
    let ext_lower = file_ext.to_lowercase();
    if ext_lower == "svg" {
        render_svg(ui, t, path);
        return;
    }
    if ext_lower == "gif" {
        render_gif(ui, t, path);
        return;
    }

    match preview.kind.as_str() {
        "image"  => render_image(ui, t, preview),
        "text"   => render_text(ui, t, preview),
        "dir"    => render_dir(ui, t, preview, path, state),
        "video"  => render_media(ui, t, preview, "video", path, state),
        "audio"  => render_media(ui, t, preview, "audio", path, state),
        "binary" => render_binary(ui, t, preview),
        _        => render_generic(ui, t, preview),
    }
}

// ── Image ─────────────────────────────────────────────────────────────────────

fn render_image(ui: &mut Ui, t: &AexTheme, preview: &FilePreview) {
    let c  = &t.colors;
    let sp = &t.spacing;

    if let Some((ref raw, w, h)) = preview.image_rgba {
        let tex_id = egui::Id::new("preview_tex_handle");

        let handle: egui::TextureHandle = {
            let cached = ui.ctx().data_mut(|d| {
                d.get_temp::<egui::TextureHandle>(tex_id)
            });
            match cached {
                Some(h) => h,
                None => {
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        raw,
                    );
                    let new_h = ui.ctx().load_texture(
                        "preview_img",
                        color_img,
                        egui::TextureOptions::LINEAR,
                    );
                    ui.ctx().data_mut(|d| d.insert_temp(tex_id, new_h.clone()));
                    new_h
                }
            }
        };

        let size    = handle.size_vec2();
        let avail_x = ui.available_width().max(1.0);
        let scale   = avail_x / size.x;
        let disp    = Vec2::new((size.x * scale).max(1.0), (size.y * scale).max(1.0));

        ui.vertical_centered(|ui| {
            ui.image(SizedTexture::new(&handle, disp));
        });
        ui.add_space(sp.sm);
        ui.label(
            RichText::new(format!("{}x{}", size.x as u32, size.y as u32))
                .color(c.text_hint())
                .size(t.typography.size_xs)
        );
    } else {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(icon(t, "preview")).color(t.colors.accent()).size(48.0));
            ui.label(
                RichText::new("Resim önizleme için çok büyük")
                    .color(c.text_hint())
                    .size(t.typography.size_xs)
            );
        });
    }
}

// ── SVG ───────────────────────────────────────────────────────────────────────

fn render_svg(ui: &mut Ui, t: &AexTheme, path: &str) {
    let c  = &t.colors;
    let sp = &t.spacing;

    let uri = format!("file://{}", path);
    let avail_w = (ui.available_width() - sp.sm * 2.0).max(1.0);

    let frame_id = egui::Id::new(format!("svg_frame_{}", path));
    let frame_count: u32 = ui.ctx().data_mut(|d| {
        let n: u32 = d.get_temp(frame_id).unwrap_or(0);
        d.insert_temp(frame_id, n + 1);
        n
    });

    if frame_count < 10 {
        ui.ctx().request_repaint();
    }

    ui.vertical_centered(|ui| {
        ui.add(
            egui::Image::new(uri)
                .max_size(Vec2::new(avail_w, 320.0))
                .maintain_aspect_ratio(true)
                .rounding(Rounding::same(t.shape.radius_sm))
        );
    });

    ui.add_space(sp.xs);
    ui.label(
        RichText::new("SVG Vektör")
            .color(c.text_hint())
            .size(t.typography.size_xs)
    );
}

// ── GIF ───────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct GifCache {
    frames:   Vec<egui::TextureHandle>,
    delays:   Vec<u64>,
    total_ms: u64,
    size:     [u32; 2],
}

pub struct DecodedGifFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub delay_ms: u64,
}

pub fn load_gif_frames(path: &str) -> Vec<DecodedGifFrame> {
    use image::AnimationDecoder;

    // 1. Try reading bytes: standard fs or elevated sudo
    let bytes = std::fs::read(path)
        .or_else(|_| crate::backend::fs_ops::read_file_bytes_elevated(path))
        .ok();

    if let Some(ref data) = bytes {
        // Strateji 1: image::codecs::gif::GifDecoder via into_frames()
        let cursor = std::io::Cursor::new(data);
        if let Ok(dec) = image::codecs::gif::GifDecoder::new(cursor) {
            let mut frames_out = Vec::new();
            for item in dec.into_frames() {
                if let Ok(frame) = item {
                    let (numer, denom) = frame.delay().numer_denom_ms();
                    let delay_ms = if denom == 0 { 100 } else { (numer as u64 / denom as u64).max(20) };
                    let img = frame.into_buffer();
                    let (w, h) = img.dimensions();
                    frames_out.push(DecodedGifFrame {
                        rgba: img.into_raw(),
                        width: w,
                        height: h,
                        delay_ms,
                    });
                }
            }
            if !frames_out.is_empty() {
                return frames_out;
            }
        }

        // Strateji 2: Tek kare veya statik GIF (image::load_from_memory)
        if let Ok(dyn_img) = image::load_from_memory_with_format(data, image::ImageFormat::Gif)
            .or_else(|_| image::load_from_memory(data)) {
            let rgba = dyn_img.to_rgba8();
            let (w, h) = rgba.dimensions();
            return vec![DecodedGifFrame {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
                delay_ms: 100,
            }];
        }
    }

    // Strateji 3: ffmpeg ile kareleri çıkarma (bozuk GIF header veya disposal hataları durumunda)
    let tmp_dir = std::env::temp_dir().join(format!("aex_gif_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let out_pattern = tmp_dir.join("f_%04d.png");

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-i", path,
            "-fps_mode", "passthrough",
            out_pattern.to_str().unwrap_or(""),
        ])
        .status();

    if let Ok(s) = status {
        if s.success() {
            let mut entries: Vec<_> = std::fs::read_dir(&tmp_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("png"))
                .collect();
            entries.sort();

            let mut frames_out = Vec::new();
            for p in &entries {
                if let Ok(img) = image::open(p) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    frames_out.push(DecodedGifFrame {
                        rgba: rgba.into_raw(),
                        width: w,
                        height: h,
                        delay_ms: 100,
                    });
                }
                let _ = std::fs::remove_file(p);
            }
            let _ = std::fs::remove_dir(&tmp_dir);
            if !frames_out.is_empty() {
                return frames_out;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // Strateji 4: ImageMagick / GraphicsMagick convert fallback
    let tmp_dir2 = std::env::temp_dir().join(format!("aex_conv_gif_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir2);
    let out_pattern2 = tmp_dir2.join("frame.png");

    let status2 = std::process::Command::new("magick")
        .args([
            "-coalesce",
            path,
            out_pattern2.to_str().unwrap_or(""),
        ])
        .stderr(std::process::Stdio::null())
        .status()
        .or_else(|_| {
            std::process::Command::new("convert")
                .args([
                    "-coalesce",
                    path,
                    out_pattern2.to_str().unwrap_or(""),
                ])
                .stderr(std::process::Stdio::null())
                .status()
        });

    if let Ok(s) = status2 {
        if s.success() {
            let mut entries: Vec<_> = std::fs::read_dir(&tmp_dir2)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("png"))
                .collect();
            entries.sort();

            let mut frames_out = Vec::new();
            for p in &entries {
                if let Ok(img) = image::open(p) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    frames_out.push(DecodedGifFrame {
                        rgba: rgba.into_raw(),
                        width: w,
                        height: h,
                        delay_ms: 100,
                    });
                }
                let _ = std::fs::remove_file(p);
            }
            let _ = std::fs::remove_dir(&tmp_dir2);
            if !frames_out.is_empty() {
                return frames_out;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp_dir2);

    Vec::new()
}

fn render_gif(ui: &mut Ui, t: &AexTheme, path: &str) {
    let c  = &t.colors;
    let sp = &t.spacing;

    let gif_id  = egui::Id::new(format!("gif_cache_{}", path));
    let time_id = egui::Id::new(format!("gif_start_{}", path));

    let cache: GifCache = {
        let existing = ui.ctx().data_mut(|d| d.get_temp::<GifCache>(gif_id));
        match existing {
            Some(c) => c,
            None => {
                let decoded = load_gif_frames(path);
                let new_cache = if !decoded.is_empty() {
                    let mut handles = Vec::with_capacity(decoded.len());
                    let mut delays  = Vec::with_capacity(decoded.len());
                    let mut size    = [decoded[0].width, decoded[0].height];

                    for (i, frame) in decoded.into_iter().enumerate() {
                        delays.push(frame.delay_ms);
                        if i == 0 { size = [frame.width, frame.height]; }

                        let color_img = egui::ColorImage::from_rgba_unmultiplied(
                            [frame.width as usize, frame.height as usize],
                            &frame.rgba,
                        );
                        let handle = ui.ctx().load_texture(
                            format!("gif_frame_{}_{}", path, i),
                            color_img,
                            egui::TextureOptions::LINEAR,
                        );
                        handles.push(handle);
                    }

                    let total_ms = delays.iter().sum::<u64>().max(1);
                    GifCache { frames: handles, delays, total_ms, size }
                } else {
                    GifCache { frames: vec![], delays: vec![], total_ms: 1, size: [1, 1] }
                };

                ui.ctx().data_mut(|d| d.insert_temp(gif_id, new_cache.clone()));
                ui.ctx().data_mut(|d| {
                    d.insert_temp(time_id, std::time::Instant::now());
                });
                new_cache
            }
        }
    };

    if cache.frames.is_empty() {
        ui.label(RichText::new("GIF çözülemedi").color(c.danger()).size(t.typography.size_sm));
        return;
    }

    let start: std::time::Instant = ui.ctx().data_mut(|d| {
        d.get_temp(time_id).unwrap_or_else(|| {
            let now = std::time::Instant::now();
            d.insert_temp(time_id, now);
            now
        })
    });

    let elapsed_ms = (start.elapsed().as_millis() as u64) % cache.total_ms;
    let mut acc = 0u64;
    let mut frame_idx = 0;
    for (i, &delay) in cache.delays.iter().enumerate() {
        acc += delay;
        if elapsed_ms < acc {
            frame_idx = i;
            break;
        }
        frame_idx = i;
    }

    let handle = &cache.frames[frame_idx];
    let [w, h] = cache.size;

    let avail_x = ui.available_width().max(1.0);
    let scale   = avail_x / w as f32;
    let disp    = Vec2::new((w as f32 * scale).max(1.0), (h as f32 * scale).max(1.0));

    ui.vertical_centered(|ui| {
        ui.image(SizedTexture::new(handle, disp));
    });
    ui.add_space(sp.sm);
    ui.label(
        RichText::new(format!(
            "{}×{}  •  {} kare  •  {:.1} fps",
            w, h,
            cache.frames.len(),
            1000.0 / cache.delays[frame_idx].max(1) as f32,
        ))
        .color(c.text_hint())
        .size(t.typography.size_xs)
    );

    ui.ctx().request_repaint();
}

// ── Text ──────────────────────────────────────────────────────────────────────

fn render_text(ui: &mut Ui, t: &AexTheme, preview: &FilePreview) {
    let c  = &t.colors;
    let sp = &t.spacing;

    if preview.is_truncated {
        ui.label(
            RichText::new("İlk 100 satır gösteriliyor")
                .color(c.text_hint())
                .size(9.0)
                .italics()
        );
        ui.add_space(sp.xs);
    } else if let Some(lines) = preview.line_count {
        ui.label(
            RichText::new(format!("{} satır", lines))
                .color(c.text_hint())
                .size(9.0)
                .italics()
        );
        ui.add_space(sp.xs);
    }

    if let Some(ref content) = preview.text_content {
        let box_fill = if t.effects.glass_panels {
            Color32::from_rgba_unmultiplied(
                c.bg_crust().r(), c.bg_crust().g(), c.bg_crust().b(),
                (t.effects.card_opacity * 160.0) as u8,
            )
        } else {
            c.bg_crust()
        };
        let max_w = (ui.available_width() - sp.sm * 2.0).max(40.0);
        let avail_h = (ui.available_height() - sp.sm).max(280.0);
        egui::Frame {
            fill: box_fill,
            rounding: Rounding::same(t.shape.radius_sm),
            inner_margin: Margin::same(sp.sm),
            stroke: Stroke::new(1.0_f32, c.border()),
            ..Default::default()
        }
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_source("preview_text_scroll_inner")
                .max_height(avail_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(content)
                                .font(FontId::new(9.5, FontFamily::Monospace))
                                .color(c.text_secondary())
                        )
                        .wrap_mode(egui::TextWrapMode::Wrap)
                    );
                });
        });
    }
}

fn format_preview_entry(name: &str, is_dir: bool, max_chars: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_chars || max_chars <= 4 {
        return name.to_string();
    }
    if is_dir {
        let prefix: String = name.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", prefix)
    } else {
        if let Some(dot_idx) = name.rfind('.') {
            let stem = &name[..dot_idx];
            let ext = &name[dot_idx..]; // includes dot
            let ext_len = ext.chars().count();
            if ext_len < max_chars.saturating_sub(2) {
                let keep_stem = max_chars.saturating_sub(ext_len + 1);
                let stem_prefix: String = stem.chars().take(keep_stem).collect();
                return format!("{}…{}", stem_prefix, ext);
            }
        }
        let prefix: String = name.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", prefix)
    }
}

// ── Dir ───────────────────────────────────────────────────────────────────────

fn render_dir(ui: &mut Ui, t: &AexTheme, preview: &FilePreview, path: &str, state: &mut AppState) {
    if ui.available_width() < 10.0 { return; }
    let c  = &t.colors;
    let sp = &t.spacing;
    let item_count = preview.line_count.unwrap_or(0);

    let size_str = if let Some(&sz) = state.du_sizes.get(path) {
        format!("  ·  {}", super::human_size(sz))
    } else {
        let request_id = egui::Id::new(format!("du_req_{}", path));
        let already_requested: bool = ui.ctx().data_mut(|d| d.get_temp(request_id).unwrap_or(false));
        if !already_requested {
            ui.ctx().data_mut(|d| d.insert_temp(request_id, true));
            state.pending_du_request = Some(path.to_string());
        }
        "  ·  Hesaplanıyor…".into()
    };

    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon(t, "folder_open")).color(t.colors.accent()).size(42.0));
        ui.add_space(sp.xs);
        ui.label(
            RichText::new(format!("{} öğe{}", item_count, size_str))
                .color(c.text_secondary())
                .size(t.typography.size_base)
                .strong()
        );
    });

    ui.add_space(sp.sm);

    if let Some(ref entries) = preview.dir_entries {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("KLASÖR İÇERİĞİ")
                    .color(c.text_hint())
                    .size(t.typography.size_xs)
                    .strong()
            );
        });
        ui.add_space(sp.xs);

        let row_h = 24.0_f32;
        for (name, is_sub_dir) in entries {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
            let is_hovered = resp.hovered();

            if is_hovered {
                let hover_bg = Color32::from_rgba_unmultiplied(
                    c.bg_surface2().r(), c.bg_surface2().g(), c.bg_surface2().b(), 70
                );
                ui.painter().rect_filled(rect, Rounding::same(t.shape.radius_sm), hover_bg);
            }

            let icon_str = if *is_sub_dir { icon(t, "folder") } else { icon(t, "file") };
            let icon_col = if *is_sub_dir { c.accent() } else { c.text_hint() };

            let icon_pos = Pos2::new(rect.min.x + 4.0, rect.center().y);
            ui.painter().text(
                icon_pos,
                Align2::LEFT_CENTER,
                icon_str,
                FontId::new(13.0, FontFamily::Monospace),
                icon_col,
            );

            let text_left = rect.min.x + 24.0;
            let text_max_w = (rect.max.x - text_left - 50.0).max(20.0);
            let approx_chars = (text_max_w / 7.0).floor() as usize;
            let display_name = format_preview_entry(name, *is_sub_dir, approx_chars.max(6));

            ui.painter().text(
                Pos2::new(text_left, rect.center().y),
                Align2::LEFT_CENTER,
                display_name,
                FontId::new(t.typography.size_xs + 0.5, FontFamily::Proportional),
                c.text_secondary(),
            );

            let sub_label = if *is_sub_dir { "klasör" } else { "" };
            if !sub_label.is_empty() {
                ui.painter().text(
                    Pos2::new(rect.max.x - 4.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    sub_label,
                    FontId::new(9.5, FontFamily::Proportional),
                    c.text_hint(),
                );
            }

            if resp.double_clicked() && *is_sub_dir {
                state.navigate(format!("{}/{}", path.trim_end_matches('/'), name));
            }

            resp.on_hover_text(if *is_sub_dir {
                format!("{}\n(Girmek için çift tıkla)", name)
            } else {
                name.clone()
            });
            ui.add_space(2.0);
        }

        if item_count > entries.len() {
            ui.add_space(sp.xs);
            ui.label(
                RichText::new(format!("…ve {} öğe daha", item_count - entries.len()))
                    .color(c.text_hint())
                    .size(t.typography.size_xs)
            );
        }
    }
}

// ── Media (arch-explorer-sorcerer / arch-explorer-source Style) ───────────────

fn render_media(ui: &mut Ui, t: &AexTheme, preview: &FilePreview, kind: &str, path: &str, state: &mut AppState) {
    let is_video = kind == "video";
    let is_playing = state.media_player.is_playing() && state.media_player.current_path.as_deref() == Some(path);

    if is_video {
        render_video_sorcerer(ui, t, preview, path, state, is_playing);
    } else {
        render_audio_sorcerer(ui, t, preview, path, state, is_playing);
    }
}

fn render_video_sorcerer(
    ui: &mut Ui,
    t: &AexTheme,
    preview: &FilePreview,
    path: &str,
    state: &mut AppState,
    is_playing: bool,
) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let avail_w = ui.available_width().max(160.0);
    let card_h = (avail_w * 0.5625).clamp(140.0, 520.0);
    let (card_rect, card_resp) = ui.allocate_exact_size(Vec2::new(avail_w, card_h), Sense::click());

    // 1. Dark container background & border (.pv-video-poster-card)
    let bg_color = Color32::from_rgb(0, 0, 0);
    let border_color = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
    ui.painter().rect_filled(card_rect, Rounding::same(s.radius_md), bg_color);
    ui.painter().rect_stroke(card_rect, Rounding::same(s.radius_md), Stroke::new(1.0, border_color));

    // 2. Content rendering
    let mut frame_drawn = false;

    if is_playing {
        let frame_opt = if let Ok(lock) = state.media_player.latest_frame.lock() {
            lock.as_ref().map(|f| (f.data.clone(), f.width, f.height))
        } else {
            None
        };

        if let Some((raw, fw, fh)) = frame_opt {
            let color_img = egui::ColorImage::from_rgba_unmultiplied([fw, fh], &raw);
            let tex_id = egui::Id::new("live_video_tex_cache");
            let mut tex: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(tex_id));
            let tex_handle = match tex.as_mut() {
                Some(existing) => {
                    existing.set(color_img, egui::TextureOptions::LINEAR);
                    existing.clone()
                }
                None => {
                    let created = ui.ctx().load_texture("live_video_preview", color_img, egui::TextureOptions::LINEAR);
                    ui.ctx().data_mut(|d| d.insert_temp(tex_id, created.clone()));
                    created
                }
            };

            let scale = (card_rect.width() / fw as f32).min(card_rect.height() / fh as f32);
            let disp = Vec2::new((fw as f32 * scale).max(1.0), (fh as f32 * scale).max(1.0));
            let draw_rect = Rect::from_center_size(card_rect.center(), disp);

            ui.painter().image(
                tex_handle.id(),
                draw_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            frame_drawn = true;
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    if !frame_drawn {
        // Render poster thumbnail if available
        if let Some((ref raw, w, h)) = preview.image_rgba {
            let poster_id = egui::Id::new(format!("vid_poster_{}", path));
            let mut tex: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(poster_id));
            let tex_handle = match tex.as_mut() {
                Some(existing) => existing.clone(),
                None => {
                    let color_img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], raw);
                    let created = ui.ctx().load_texture("vid_poster", color_img, egui::TextureOptions::LINEAR);
                    ui.ctx().data_mut(|d| d.insert_temp(poster_id, created.clone()));
                    created
                }
            };

            let scale = (card_rect.width() / w as f32).min(card_rect.height() / h as f32);
            let disp = Vec2::new((w as f32 * scale).max(1.0), (h as f32 * scale).max(1.0));
            let draw_rect = Rect::from_center_size(card_rect.center(), disp);

            ui.painter().image(
                tex_handle.id(),
                draw_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // Centered 🎬 placeholder
            ui.painter().text(
                card_rect.center(),
                Align2::CENTER_CENTER,
                "🎬",
                FontId::proportional(42.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, 100),
            );
        }

        // Center Play Button (.pv-video-play-center)
        let center_pos = card_rect.center();
        let circle_r = 24.0;
        let circle_rect = Rect::from_center_size(center_pos, Vec2::splat(circle_r * 2.0));
        let is_circle_hovered = ui.rect_contains_pointer(circle_rect);

        let circle_fill = if is_circle_hovered {
            c.accent()
        } else {
            Color32::from_rgba_unmultiplied(130, 100, 255, 225)
        };

        ui.painter().circle_filled(center_pos, circle_r, circle_fill);
        ui.painter().circle_stroke(center_pos, circle_r, Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230)));
        ui.painter().text(
            center_pos + Vec2::new(2.0, 0.0),
            Align2::CENTER_CENTER,
            "▶",
            FontId::proportional(18.0),
            Color32::WHITE,
        );

        if card_resp.clicked() {
            state.media_player.play(path.to_string(), true);
        }
    }

    // 3. Top bar overlay (.pv-video-top-bar)
    let mpv_btn = egui::Button::new(
        RichText::new("▶ mpv")
            .color(Color32::WHITE)
            .size(10.5)
            .strong()
    )
    .fill(Color32::from_rgba_unmultiplied(15, 15, 20, 210))
    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 65)))
    .rounding(Rounding::same(5.0));

    let mpv_rect = Rect::from_min_size(card_rect.min + Vec2::new(8.0, 8.0), Vec2::new(56.0, 26.0));
    if ui.put(mpv_rect, mpv_btn).on_hover_text("Varsayılan Oynatıcıda Aç (mpv)").clicked() {
        let p = path.to_string();
        std::thread::spawn(move || {
            std::process::Command::new("mpv").arg(&p).spawn().ok();
        });
    }

    // Right button: ⛶ Tam Ekran (.pv-video-fs-corner-btn)
    let fs_btn = egui::Button::new(
        RichText::new("⛶ Tam Ekran")
            .color(Color32::WHITE)
            .size(10.5)
            .strong()
    )
    .fill(Color32::from_rgba_unmultiplied(15, 15, 20, 210))
    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 65)))
    .rounding(Rounding::same(5.0));

    let fs_w = 88.0;
    let fs_rect = Rect::from_min_size(pos2(card_rect.max.x - 8.0 - fs_w, card_rect.min.y + 8.0), Vec2::new(fs_w, 26.0));
    if ui.put(fs_rect, fs_btn).on_hover_text("Tam Ekran").clicked() {
        if !is_playing {
            state.media_player.play(path.to_string(), true);
        }
        state.is_video_fullscreen = true;
    }

    // If playing: ⏹ Durdur button
    if is_playing {
        let stop_btn = egui::Button::new(
            RichText::new("⏹ Durdur")
                .color(c.danger())
                .size(10.5)
                .strong()
        )
        .fill(Color32::from_rgba_unmultiplied(30, 15, 20, 220))
        .stroke(Stroke::new(1.0, c.danger()))
        .rounding(Rounding::same(5.0));

        let stop_w = 72.0;
        let stop_rect = Rect::from_min_size(pos2(fs_rect.min.x - 6.0 - stop_w, card_rect.min.y + 8.0), Vec2::new(stop_w, 26.0));
        if ui.put(stop_rect, stop_btn).clicked() {
            state.media_player.stop();
        }
    }

    // 4. Bottom-right badge overlay (.pv-video-badge-overlay / .video-overlay-badge)
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("VİDEO")
        .to_uppercase();

    let galley = ui.painter().layout_no_wrap(
        ext.clone(),
        FontId::new(9.5, FontFamily::Proportional),
        Color32::from_rgb(255, 209, 102),
    );
    let badge_w = galley.size().x + 12.0;
    let badge_h = 18.0;
    let badge_pos = pos2(
        card_rect.max.x - badge_w - 8.0,
        card_rect.max.y - badge_h - 8.0,
    );
    let badge_rect = Rect::from_min_size(badge_pos, Vec2::new(badge_w, badge_h));

    ui.painter().rect_filled(badge_rect, Rounding::same(4.0), Color32::from_rgba_unmultiplied(0, 0, 0, 195));
    ui.painter().rect_stroke(badge_rect, Rounding::same(4.0), Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 50)));
    ui.painter().text(
        badge_rect.center(),
        Align2::CENTER_CENTER,
        &ext,
        FontId::new(9.5, FontFamily::Proportional),
        Color32::from_rgb(255, 209, 102),
    );

    // KESİN İMLEÇ DÜZELTMESİ: ui.put butonları yerleştirdikten sonra egui imleci kartın tepesine çeker!
    // Bu satır ile imleci kartın bittiği alt noktaya muntazamca ilerletiyoruz:
    ui.advance_cursor_after_rect(card_rect);
    ui.add_space(sp.xs);

    // ── Video Timeline Bar & Media Controls ──────────────────────────────────────
    let total_dur = parse_duration_seconds(preview.media_info.as_ref());
    if total_dur > 0.0 && state.media_player.total_duration == 0.0 {
        state.media_player.total_duration = total_dur;
    }
    let effective_total_dur = if state.media_player.total_duration > 0.0 {
        state.media_player.total_duration
    } else {
        total_dur
    };

    let elapsed = state.media_player.elapsed_secs();
    let remaining = (effective_total_dur - elapsed).max(0.0);

    // 1. Timeline Scrubber Track (always shown for videos with duration)
    if effective_total_dur > 0.0 {
        let track_h = 8.0;
        let track_w = ui.available_width().max(60.0);
        let (track_rect, track_resp) = ui.allocate_exact_size(Vec2::new(track_w, track_h), Sense::click_and_drag());

        let progress = (elapsed / effective_total_dur).clamp(0.0, 1.0);
        let is_hovered = track_resp.hovered();
        let bg_bar_color = Color32::from_rgba_unmultiplied(255, 255, 255, if is_hovered { 50 } else { 30 });
        let fill_bar_color = if is_hovered { c.accent() } else { Color32::from_rgb(130, 100, 255) };

        // Background groove
        ui.painter().rect_filled(track_rect, Rounding::same(4.0), bg_bar_color);

        // Filled progress
        let filled_w = track_rect.width() * progress as f32;
        if filled_w > 0.0 {
            let filled_rect = Rect::from_min_size(track_rect.min, Vec2::new(filled_w, track_h));
            ui.painter().rect_filled(filled_rect, Rounding::same(4.0), fill_bar_color);
        }

        // Handle knob
        let knob_x = track_rect.min.x + filled_w;
        let knob_r = if is_hovered { 6.0 } else { 4.0 };
        ui.painter().circle_filled(pos2(knob_x, track_rect.center().y), knob_r, Color32::WHITE);
        ui.painter().circle_stroke(pos2(knob_x, track_rect.center().y), knob_r, Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100)));

        // Click or Drag on timeline to seek
        if track_resp.clicked() || track_resp.dragged() {
            if let Some(pos) = track_resp.interact_pointer_pos() {
                let rel_x = (pos.x - track_rect.min.x) / track_rect.width();
                let target_sec = (rel_x as f64 * effective_total_dur).clamp(0.0, effective_total_dur);
                state.media_player.seek_to(target_sec);
            }
        }

        ui.add_space(sp.xs);

        // 2. Time Labels (Geçen Süre sol, Kalan Süre sağ)
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format_time_hms(elapsed))
                    .color(c.text_secondary())
                    .size(11.0)
                    .monospace()
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("-{}", format_time_hms(remaining)))
                        .color(c.text_hint())
                        .size(11.0)
                        .monospace()
                );
            });
        });
        ui.add_space(sp.xs);
    }

    // 3. Playback Controls Bar (.pv-video-controls)
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // Önceki Video (⏮)
        let prev_btn = egui::Button::new(RichText::new("⏮").size(12.0))
            .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
            .rounding(Rounding::same(4.0));
        if ui.add(prev_btn).on_hover_text("Önceki Medya").clicked() {
            state.navigate_media_sibling(false);
        }

        // -5s Geri Git (⏪)
        let rew_btn = egui::Button::new(RichText::new("⏪ -5s").size(11.0))
            .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
            .rounding(Rounding::same(4.0));
        if ui.add(rew_btn).on_hover_text("5 saniye geri git (Sol Ok)").clicked() {
            state.media_player.seek_relative(-5.0);
        }

        // Oynat / Duraklat (▶ / ⏸)
        let is_running = state.media_player.is_playing && !state.media_player.is_paused;
        let play_icon = if is_running { "⏸" } else { "▶" };
        let play_color = if is_running { c.accent() } else { Color32::from_rgb(130, 100, 255) };
        let play_btn = egui::Button::new(RichText::new(play_icon).color(Color32::WHITE).size(13.0).strong())
            .fill(play_color)
            .rounding(Rounding::same(4.0));
        if ui.add_sized(Vec2::new(34.0, 24.0), play_btn).on_hover_text(if is_running { "Duraklat (Boşluk)" } else { "Oynat (Boşluk)" }).clicked() {
            if !state.media_player.is_playing && !state.media_player.is_paused {
                state.media_player.play_at(path.to_string(), true, 0.0, effective_total_dur);
            } else {
                state.media_player.toggle_pause();
            }
        }

        // +5s İleri Git (⏩)
        let ff_btn = egui::Button::new(RichText::new("+5s ⏩").size(11.0))
            .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
            .rounding(Rounding::same(4.0));
        if ui.add(ff_btn).on_hover_text("5 saniye ileri git (Sağ Ok)").clicked() {
            state.media_player.seek_relative(5.0);
        }

        // Sonraki Video (⏭)
        let next_btn = egui::Button::new(RichText::new("⏭").size(12.0))
            .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
            .rounding(Rounding::same(4.0));
        if ui.add(next_btn).on_hover_text("Sonraki Medya").clicked() {
            state.navigate_media_sibling(true);
        }

        // Durdur (⏹)
        if state.media_player.is_playing || state.media_player.is_paused {
            let stop_bar_btn = egui::Button::new(RichText::new("⏹").color(c.danger()).size(12.0))
                .fill(Color32::from_rgba_unmultiplied(255, 40, 40, 20))
                .stroke(Stroke::new(1.0, c.danger()))
                .rounding(Rounding::same(4.0));
            if ui.add(stop_bar_btn).on_hover_text("Durdur").clicked() {
                state.media_player.stop();
            }
        }
    });
    ui.add_space(sp.sm);

    // 4. Playing status indicator
    if is_playing {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let status_msg = if state.media_player.is_paused {
                "⏸ Video Duraklatıldı"
            } else {
                "● Video Oynatılıyor…"
            };
            let status_color = if state.media_player.is_paused {
                c.warning()
            } else {
                c.success()
            };
            ui.label(
                RichText::new(status_msg)
                    .color(status_color)
                    .size(t.typography.size_xs)
                    .strong()
            );
        });
        ui.add_space(sp.xs);
    }

    // 5. Media Summary Table (.pv-media-table)
    render_media_summary_table(ui, t, preview);
}

fn render_audio_sorcerer(
    ui: &mut Ui,
    t: &AexTheme,
    preview: &FilePreview,
    path: &str,
    state: &mut AppState,
    is_playing: bool,
) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let total_dur = parse_duration_seconds(preview.media_info.as_ref());
    if total_dur > 0.0 && state.media_player.total_duration == 0.0 {
        state.media_player.total_duration = total_dur;
    }
    let effective_total_dur = if state.media_player.total_duration > 0.0 {
        state.media_player.total_duration
    } else {
        total_dur
    };

    let elapsed = state.media_player.elapsed_secs();
    let remaining = (effective_total_dur - elapsed).max(0.0);

    // Koyu / Siyah Kart (Kullanıcının istediği siyah alan içine tüm kontroller)
    let card_fill = Color32::from_rgba_unmultiplied(16, 16, 24, 235);
    egui::Frame {
        fill: card_fill,
        rounding: Rounding::same(s.radius_md),
        stroke: Stroke::new(1.0, c.border()),
        inner_margin: Margin::same(12.0),
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.set_width(ui.available_width());

        // 1. Üst Kısım: Albüm Kapağı (Cover Art) veya Vinil + Başlık ve Format Rozeti
        ui.horizontal(|ui| {
            let cover_size = Vec2::splat(68.0);
            if let Some((ref raw, w, h)) = preview.image_rgba {
                let tex_id = egui::Id::new(format!("audio_cover_{}", path));
                let cover_handle = ui.ctx().data_mut(|d| d.get_temp::<egui::TextureHandle>(tex_id)).unwrap_or_else(|| {
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        raw,
                    );
                    let handle = ui.ctx().load_texture(
                        format!("audio_cover_{}", path),
                        color_img,
                        egui::TextureOptions::LINEAR,
                    );
                    ui.ctx().data_mut(|d| d.insert_temp(tex_id, handle.clone()));
                    handle
                });

                let (cover_rect, _) = ui.allocate_exact_size(cover_size, Sense::hover());
                // Albüm kapağını yuvarlak köşeli çiz
                ui.painter().rect_filled(cover_rect, Rounding::same(6.0), Color32::BLACK);
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                ui.painter().image(cover_handle.id(), cover_rect, uv, Color32::WHITE);
                let border_col = if is_playing { c.accent() } else { Color32::from_rgba_unmultiplied(255, 255, 255, 40) };
                ui.painter().rect_stroke(cover_rect, Rounding::same(6.0), Stroke::new(1.0, border_col));
            } else {
                // Kapak görseli yoksa şık vinil / plak efekti
                let (icon_rect, _) = ui.allocate_exact_size(cover_size, Sense::hover());
                ui.painter().circle_filled(icon_rect.center(), cover_size.x * 0.48, Color32::from_rgba_unmultiplied(25, 25, 35, 255));
                ui.painter().circle_stroke(icon_rect.center(), cover_size.x * 0.48, Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 25)));
                ui.painter().circle_stroke(icon_rect.center(), cover_size.x * 0.32, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 15)));
                ui.painter().circle_stroke(icon_rect.center(), cover_size.x * 0.18, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20)));
                ui.painter().text(
                    icon_rect.center(),
                    Align2::CENTER_CENTER,
                    "🎵",
                    FontId::proportional(24.0),
                    if is_playing { c.accent() } else { c.text_hint() },
                );
            }

            ui.add_space(sp.xs);

            // Başlık & Bilgi
            ui.vertical(|ui| {
                let filename = std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path);

                ui.add(
                    egui::Label::new(
                        RichText::new(filename)
                            .color(c.text_primary())
                            .size(t.typography.size_sm)
                            .strong()
                    ).wrap_mode(egui::TextWrapMode::Truncate)
                );

                ui.add_space(2.0);

                let ext = std::path::Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("SES")
                    .to_uppercase();

                ui.horizontal(|ui| {
                    // Format rozeti
                    let badge_bg = Color32::from_rgba_unmultiplied(255, 209, 102, 35);
                    let badge_fg = Color32::from_rgb(255, 209, 102);
                    egui::Frame {
                        fill: badge_bg,
                        rounding: Rounding::same(4.0),
                        inner_margin: Margin::symmetric(6.0, 2.0),
                        stroke: Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 209, 102, 80)),
                        ..Default::default()
                    }.show(ui, |ui| {
                        ui.label(RichText::new(&ext).color(badge_fg).size(9.5).strong());
                    });

                    if let Some(ref info) = preview.media_info {
                        if let Some(br) = info.get("Bitrate").and_then(|v| v.as_str()) {
                            ui.label(RichText::new(br).color(c.text_hint()).size(10.0));
                        }
                    }
                });

                if preview.image_rgba.is_some() {
                    ui.label(RichText::new("📷 Albüm Kapağı").color(c.accent()).size(9.0));
                }
            });
        });

        ui.add_space(sp.sm);

        // 2. Timeline Scrubber (İlerleme Çubuğu ve Tıklayarak İlerleme)
        let track_h = 7.0;
        let track_w = ui.available_width().max(60.0);
        let (track_rect, track_resp) = ui.allocate_exact_size(Vec2::new(track_w, track_h), Sense::click_and_drag());

        let progress = if effective_total_dur > 0.0 {
            (elapsed / effective_total_dur).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let is_hovered = track_resp.hovered();
        let bg_bar_color = Color32::from_rgba_unmultiplied(255, 255, 255, if is_hovered { 50 } else { 25 });
        let fill_bar_color = if is_hovered { c.accent() } else { Color32::from_rgb(130, 100, 255) };

        // Arka plan oluğu
        ui.painter().rect_filled(track_rect, Rounding::same(3.5), bg_bar_color);

        // Dolu ilerleme
        let filled_w = track_rect.width() * progress as f32;
        if filled_w > 0.0 {
            let filled_rect = Rect::from_min_size(track_rect.min, Vec2::new(filled_w, track_h));
            ui.painter().rect_filled(filled_rect, Rounding::same(3.5), fill_bar_color);
        }

        // Tutma düğmesi (knob)
        let knob_x = track_rect.min.x + filled_w;
        let knob_r = if is_hovered { 5.5 } else { 3.5 };
        ui.painter().circle_filled(pos2(knob_x, track_rect.center().y), knob_r, Color32::WHITE);
        ui.painter().circle_stroke(pos2(knob_x, track_rect.center().y), knob_r, Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100)));

        // Tıklayarak veya sürükleyerek saniyeye atlama
        if (track_resp.clicked() || track_resp.dragged()) && effective_total_dur > 0.0 {
            if let Some(pos) = track_resp.interact_pointer_pos() {
                let rel_x = (pos.x - track_rect.min.x) / track_rect.width();
                let target_sec = (rel_x as f64 * effective_total_dur).clamp(0.0, effective_total_dur);
                state.media_player.seek_to(target_sec);
            }
        }

        ui.add_space(2.0);

        // 3. Süre Göstergesi (Geçen Süre sol, Kalan Süre sağ)
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format_time_hms(elapsed))
                    .color(c.text_secondary())
                    .size(10.5)
                    .monospace()
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let rem_text = if effective_total_dur > 0.0 {
                    format!("-{}", format_time_hms(remaining))
                } else {
                    "--:--".to_string()
                };
                ui.label(
                    RichText::new(rem_text)
                        .color(c.text_hint())
                        .size(10.5)
                        .monospace()
                );
            });
        });

        ui.add_space(sp.xs);

        // 4. Medya Oynatma Kontrolleri (Önceki, -5s, Play/Pause, +5s, Sonraki, Stop)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            // Önceki Parça (⏮)
            let prev_btn = egui::Button::new(RichText::new("⏮").size(12.0))
                .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                .rounding(Rounding::same(4.0));
            if ui.add(prev_btn).on_hover_text("Önceki Medya").clicked() {
                state.navigate_media_sibling(false);
            }

            // -5s Geri Git (⏪)
            let rew_btn = egui::Button::new(RichText::new("⏪ -5s").size(11.0))
                .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                .rounding(Rounding::same(4.0));
            if ui.add(rew_btn).on_hover_text("5 saniye geri git (Sol Ok)").clicked() {
                state.media_player.seek_relative(-5.0);
            }

            // Oynat / Duraklat (▶ / ⏸)
            let is_running = state.media_player.is_playing && !state.media_player.is_paused;
            let play_icon = if is_running { "⏸" } else { "▶" };
            let play_color = if is_running { c.accent() } else { Color32::from_rgb(130, 100, 255) };
            let play_btn = egui::Button::new(RichText::new(play_icon).color(Color32::WHITE).size(13.0).strong())
                .fill(play_color)
                .rounding(Rounding::same(4.0));
            if ui.add_sized(Vec2::new(36.0, 24.0), play_btn).on_hover_text(if is_running { "Duraklat (Boşluk)" } else { "Oynat (Boşluk)" }).clicked() {
                if !state.media_player.is_playing && !state.media_player.is_paused {
                    state.media_player.play_at(path.to_string(), false, 0.0, effective_total_dur);
                } else {
                    state.media_player.toggle_pause();
                }
            }

            // +5s İleri Git (⏩)
            let ff_btn = egui::Button::new(RichText::new("+5s ⏩").size(11.0))
                .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                .rounding(Rounding::same(4.0));
            if ui.add(ff_btn).on_hover_text("5 saniye ileri git (Sağ Ok)").clicked() {
                state.media_player.seek_relative(5.0);
            }

            // Sonraki Parça (⏭)
            let next_btn = egui::Button::new(RichText::new("⏭").size(12.0))
                .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                .rounding(Rounding::same(4.0));
            if ui.add(next_btn).on_hover_text("Sonraki Medya").clicked() {
                state.navigate_media_sibling(true);
            }

            // Durdur (⏹)
            if state.media_player.is_playing || state.media_player.is_paused {
                let stop_bar_btn = egui::Button::new(RichText::new("⏹").color(c.danger()).size(12.0))
                    .fill(Color32::from_rgba_unmultiplied(255, 40, 40, 25))
                    .stroke(Stroke::new(1.0, c.danger()))
                    .rounding(Rounding::same(4.0));
                if ui.add(stop_bar_btn).on_hover_text("Durdur").clicked() {
                    state.media_player.stop();
                }
            }
        });

        // 5. Durum Göstergesi
        if is_playing {
            ui.add_space(sp.xs);
            let status_msg = if state.media_player.is_paused {
                "⏸ Ses Duraklatıldı"
            } else {
                "● Ses Çalınıyor…"
            };
            let status_color = if state.media_player.is_paused {
                c.warning()
            } else {
                c.success()
            };
            ui.label(
                RichText::new(status_msg)
                    .color(status_color)
                    .size(t.typography.size_xs)
                    .strong()
            );
        }
    });

    ui.add_space(sp.xs);

    render_media_summary_table(ui, t, preview);
}

fn render_media_summary_table(ui: &mut Ui, t: &AexTheme, preview: &FilePreview) {
    let c  = &t.colors;
    let sp = &t.spacing;

    if let Some(ref info) = preview.media_info {
        if let Some(obj) = info.as_object() {
            ui.add_space(sp.xs);
            ui.vertical(|ui| {
                let order = ["Duration", "Size", "Bitrate", "Format", "Video", "FPS", "Audio"];
                let mut any_rendered = false;
                for &k in &order {
                    if let Some(v) = obj.get(k) {
                        let owned = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
                        meta_row(ui, t, k, &owned, false);
                        any_rendered = true;
                    }
                }
                if !any_rendered {
                    for (k, v) in obj.iter().take(8) {
                        meta_row(ui, t, k, v.to_string().trim_matches('"'), false);
                    }
                }
            });
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("ffprobe bulunamadı\n(sudo pacman -S ffmpeg)")
                    .color(c.text_hint())
                    .size(t.typography.size_xs)
            );
        });
    }
}

// ── Binary Hex Dump ───────────────────────────────────────────────────────────

fn render_binary(ui: &mut Ui, t: &AexTheme, preview: &FilePreview) {
    let c  = &t.colors;
    let sp = &t.spacing;

    if preview.is_truncated {
        ui.label(
            RichText::new("İlk 512 byte gösteriliyor")
                .color(c.text_hint())
                .size(9.0)
                .italics()
        );
        ui.add_space(sp.xs);
    }

    if let Some(ref hex) = preview.text_content {
        let box_fill = if t.effects.glass_panels {
            Color32::from_rgba_unmultiplied(
                c.bg_crust().r(), c.bg_crust().g(), c.bg_crust().b(),
                (t.effects.card_opacity * 160.0) as u8,
            )
        } else {
            c.bg_crust()
        };
        let avail_h = (ui.available_height() - sp.sm).max(280.0);
        egui::Frame {
            fill: box_fill,
            rounding: Rounding::same(t.shape.radius_sm),
            inner_margin: Margin::same(sp.sm),
            stroke: Stroke::new(1.0_f32, c.border()),
            ..Default::default()
        }
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_source("preview_binary_hex_scroll")
                .max_height(avail_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(hex)
                                .color(c.text_secondary())
                                .font(FontId::new(9.5, FontFamily::Monospace))
                        ).wrap_mode(egui::TextWrapMode::Truncate)
                    );
                });
        });
    }
}

// ── Generic ───────────────────────────────────────────────────────────────────

fn render_generic(ui: &mut Ui, t: &AexTheme, preview: &FilePreview) {
    let c  = &t.colors;
    let sp = &t.spacing;
    ui.vertical_centered(|ui| {
        ui.add_space(sp.xl);
        ui.label(RichText::new(icon(t, "file")).color(c.text_disabled()).size(40.0));
        ui.add_space(sp.sm);
        ui.label(RichText::new("Önizleme mevcut değil").color(c.text_hint()).size(t.typography.size_sm));
        ui.label(RichText::new(&preview.mime_type).color(c.text_hint()).size(t.typography.size_xs));
    });
}

// ── Empty states ──────────────────────────────────────────────────────────────

fn empty_preview(ui: &mut Ui, t: &AexTheme, state: &AppState) {
    let c  = &t.colors;
    let sp = &t.spacing;

    // Bulunulan dizinin diskini bul (getCurrentDisk mantığı)
    let current_path = &state.nav.current_path;
    let disk = state.sidebar.drives.iter()
        .filter(|d| !d.mount_point.is_empty() && current_path.starts_with(&d.mount_point))
        .max_by_key(|d| d.mount_point.len());

    if let Some(d) = disk {
        ui.add_space(sp.xs);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("MEVCUT DİSK")
                    .color(c.text_hint())
                    .size(9.5)
                    .strong()
            );
        });
        ui.add_space(sp.xs);

        let is_ntfs = d.fs_type.to_lowercase().contains("ntfs");
        let disk_icon = if is_ntfs { icon(t, "ntfs") } else { icon(t, "drive") };
        let disk_icon_color = if is_ntfs { Color32::from_rgb(0, 140, 255) } else { c.accent() };

        let disk_fill = if t.effects.glass_panels {
            Color32::from_rgba_unmultiplied(
                c.bg_surface0().r(),
                c.bg_surface0().g(),
                c.bg_surface0().b(),
                (t.effects.card_opacity * 80.0) as u8,
            )
        } else {
            c.bg_surface0()
        };

        egui::Frame {
            fill: disk_fill,
            rounding: Rounding::same(t.shape.radius_sm),
            inner_margin: Margin::same(sp.sm),
            stroke: Stroke::new(1.0_f32, c.border()),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(disk_icon).color(disk_icon_color).size(16.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&d.name).color(c.text_primary()).size(11.0).strong());
                    ui.label(
                        RichText::new(format!("{} · {}", d.device, d.fs_type))
                            .color(c.text_hint())
                            .font(FontId::new(9.0, FontFamily::Monospace))
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(human_size(d.total))
                            .color(c.text_secondary())
                            .font(FontId::new(9.5, FontFamily::Monospace))
                    );
                });
            });

            ui.add_space(sp.xs);

            // Usage bar
            let pct = if d.total > 0 {
                (d.used as f32 / d.total as f32) * 100.0
            } else {
                0.0
            };
            let bar_color = if pct >= 90.0 {
                c.danger()
            } else if pct >= 70.0 {
                c.warning()
            } else {
                c.accent()
            };

            let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(20.0), 4.0), Sense::hover());
            ui.painter().rect_filled(bar_rect, Rounding::same(2.0), c.bg_crust());
            if pct > 0.0 {
                let fill_w = (bar_rect.width() * (pct / 100.0)).min(bar_rect.width());
                let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, 4.0));
                ui.painter().rect_filled(fill_rect, Rounding::same(2.0), bar_color);
            }

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{:.0}%", pct))
                        .color(bar_color)
                        .size(9.0)
                        .strong()
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} boş", human_size(d.available)))
                            .color(c.text_hint())
                            .size(9.0)
                    );
                });
            });
        });

        ui.add_space(sp.lg);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Bir dosyaya tıkla")
                    .color(c.text_hint())
                    .size(10.5)
                    .italics()
            );
        });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(RichText::new("◫").color(c.text_disabled()).size(32.0));
            ui.add_space(sp.sm);
            ui.label(RichText::new("Önizleme için bir dosya seçin").color(c.text_hint()).size(t.typography.size_sm));
        });
    }
}

fn multi_preview(ui: &mut Ui, t: &AexTheme, paths: &[String]) {
    let c  = &t.colors;
    let sp = &t.spacing;
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(icon(t, "file")).color(c.accent()).size(32.0));
        ui.add_space(sp.sm);
        ui.label(
            RichText::new(format!("{} dosya seçildi", paths.len()))
                .color(c.text_secondary())
                .size(t.typography.size_base)
        );
        let total: u64 = paths.iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        if total > 0 {
            ui.label(
                RichText::new(format!("Toplam: {}", human_size(total)))
                    .color(c.text_hint())
                    .size(t.typography.size_sm)
            );
        }
    });
}

pub fn parse_duration_seconds(media_info: Option<&serde_json::Value>) -> f64 {
    if let Some(info) = media_info {
        if let Some(v) = info.get("duration_secs") {
            if let Some(f) = v.as_f64() {
                return f;
            }
        }
        if let Some(d_str) = info.get("Duration").and_then(|v| v.as_str()) {
            let parts: Vec<&str> = d_str.split(':').collect();
            if parts.len() == 2 {
                let m = parts[0].parse::<f64>().unwrap_or(0.0);
                let s = parts[1].parse::<f64>().unwrap_or(0.0);
                return m * 60.0 + s;
            } else if parts.len() == 3 {
                let h = parts[0].parse::<f64>().unwrap_or(0.0);
                let m = parts[1].parse::<f64>().unwrap_or(0.0);
                let s = parts[2].parse::<f64>().unwrap_or(0.0);
                return h * 3600.0 + m * 60.0 + s;
            }
        }
    }
    0.0
}

pub fn format_time_hms(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{:02}:{:02}", m, sec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_icon_str() {
        assert_eq!(kind_icon_str("dir", true), "📁");
        assert_eq!(kind_icon_str("dir", false), "📁");
        assert_eq!(kind_icon_str("image", false), "🖼");
        assert_eq!(kind_icon_str("video", false), "🎬");
        assert_eq!(kind_icon_str("audio", false), "🎵");
        assert_eq!(kind_icon_str("text", false), "📝");
        assert_eq!(kind_icon_str("binary", false), "⬡");
        assert_eq!(kind_icon_str("unknown", false), "📄");
    }

    #[test]
    fn test_video_layout_geometry() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut state = AppState::new();
                let theme = AexTheme::default();
                let mut dummy_preview = FilePreview {
                    kind: "video".into(),
                    mime_type: "video/mp4".into(),
                    size: 15_000_000,
                    image_b64: None,
                    image_rgba: None,
                    text_content: None,
                    media_info: Some(serde_json::json!({
                        "Duration": "00:32",
                        "Size": "15.3 MB",
                        "Bitrate": "3951 kbps",
                        "Format": "QuickTime / MOV",
                        "Video": "h264 1920x1080",
                        "FPS": "60.00 fps",
                        "Audio": "aac 2ch 48000Hz"
                    })),
                    line_count: None,
                    is_truncated: false,
                    language: None,
                    dir_entries: None,
                };
                let before_y = ui.cursor().top();
                render_video_sorcerer(ui, &theme, &dummy_preview, "/test/video.mp4", &mut state, false);
                let after_y = ui.cursor().top();
                assert!(after_y - before_y >= 250.0, "Cursor must advance below video card and summary table: before={}, after={}, diff={}", before_y, after_y, after_y - before_y);
            });
        });
    }

    #[test]
    fn test_format_time_hms() {
        assert_eq!(format_time_hms(0.0), "00:00");
        assert_eq!(format_time_hms(14.2), "00:14");
        assert_eq!(format_time_hms(65.0), "01:05");
        assert_eq!(format_time_hms(3665.0), "1:01:05");
    }

    #[test]
    fn test_parse_duration_seconds() {
        let val1 = serde_json::json!({ "Duration": "01:25" });
        assert_eq!(parse_duration_seconds(Some(&val1)), 85.0);

        let val2 = serde_json::json!({ "Duration": "01:02:03" });
        assert_eq!(parse_duration_seconds(Some(&val2)), 3723.0);

        let val3 = serde_json::json!({ "duration_secs": 42.5 });
        assert_eq!(parse_duration_seconds(Some(&val3)), 42.5);
    }

    #[test]
    fn test_decode_gif() {
        use image::AnimationDecoder;
        let file = std::fs::File::open("/home/aiuser/test.gif");
        assert!(file.is_ok(), "test.gif should exist");
        let reader = std::io::BufReader::new(file.unwrap());
        let dec = image::codecs::gif::GifDecoder::new(reader);
        assert!(dec.is_ok(), "GifDecoder should initialize");
        let frames = dec.unwrap().into_frames().collect_frames();
        assert!(frames.is_ok(), "collect_frames should succeed: {:?}", frames.err());
        assert_eq!(frames.unwrap().len(), 2);

        let one_by_one = "/home/aiuser/1x1.gif";
        if std::path::Path::new(one_by_one).exists() {
            let f = std::fs::File::open(one_by_one).unwrap();
            let reader = std::io::BufReader::new(f);
            let dec = image::codecs::gif::GifDecoder::new(reader);
            println!("1x1.gif dec ok? {}", dec.is_ok());
            if let Ok(d) = dec {
                let frames = d.into_frames().collect_frames();
                println!("1x1.gif frames: {:?}", frames.as_ref().map(|f| f.len()).map_err(|e| e.to_string()));
            }
        }
    }
}

