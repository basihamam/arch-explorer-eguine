// src/backend/thumbnails.rs
// Disk-persistent thumbnail cache (Nemo-style aspect-ratio preserving & Lanczos3 high-fidelity).
// 3 kalite seviyesi: Low(160) / Mid(320) / High(512)
// Aspect ratio bozulmuyor, kırpılmıyor, Lanczos3 anti-aliasing ile aşırı büyük resimlerde kalite kaybı önleniyor.
// Bir kere üretilip ~/.cache/arch-explorer/thumbnails/ altına kaydedilir.
// Kaynak dosya mtime değişirse stale sayılır ve yeniden üretilir.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThumbQuality { Low, Mid, High }

impl ThumbQuality {
    pub fn px(self) -> u32 {
        match self { Self::Low => 160, Self::Mid => 320, Self::High => 512 }
    }
    pub fn jpeg_q(self) -> u8 {
        match self { Self::Low => 85, Self::Mid => 90, Self::High => 95 }
    }
    pub fn suffix(self) -> &'static str {
        // px boyutunu suffix'e dahil et — kalite değişince eskiler karışmaz
        match self { Self::Low => "160", Self::Mid => "320", Self::High => "512" }
    }
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("thumbnail")
}

fn thumb_path(src: &Path, quality: ThumbQuality) -> PathBuf {
    // Dosya path'ini hash'le — çakışmayı önle
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    src.hash(&mut h);
    let hash = h.finish();
    cache_dir().join(format!("{:x}_{}.jpg", hash, quality.suffix()))
}

fn src_mtime(path: &Path) -> Option<SystemTime> {
    path.metadata().ok()?.modified().ok()
}

fn thumb_mtime(thumb: &Path) -> Option<SystemTime> {
    thumb.metadata().ok()?.modified().ok()
}

/// Thumbnail'i döner. Önce cache'i kontrol eder, stale ise yeniden üretir.
/// `None` → üretilemedi (desteklenmeyen format vb.)
pub fn get_thumbnail(src_path: &str, quality: ThumbQuality) -> Option<Vec<u8>> {
    let src  = Path::new(src_path);
    let dst  = thumb_path(src, quality);

    // Cache geçerli mi?
    if dst.exists() {
        if let (Some(src_t), Some(dst_t)) = (src_mtime(src), thumb_mtime(&dst)) {
            if dst_t >= src_t {
                return std::fs::read(&dst).ok();
            }
        }
    }

    // Üret + kaydet
    generate_and_save(src, &dst, quality)
}

fn generate_and_save(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let ext = src.extension()?.to_str()?.to_lowercase();

    // GIF — sadece ilk kare
    if ext == "gif" {
        return generate_gif_thumb(src, dst, quality);
    }

    // SVG — rsvg-convert veya inkscape ile rasterize
    if ext == "svg" {
        return generate_svg_thumb(src, dst, quality);
    }

    // Video — ffmpeg ile ilk saniyeden kare yakala
    if matches!(ext.as_str(), "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts") {
        return generate_video_thumb(src, dst, quality);
    }

    // Ses — ffmpeg ile gömülü albüm kapağını çıkar
    if matches!(ext.as_str(), "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma") {
        return generate_audio_thumb(src, dst, quality);
    }

    // Normal raster formatlar
    let img = image::open(src).ok().or_else(|| {
        let path_str = src.to_string_lossy().to_string();
        let bytes = crate::backend::fs_ops::read_file_bytes_elevated(&path_str).ok()?;
        image::load_from_memory(&bytes).ok()
    })?;
    let px  = quality.px();

    // Nemo-style aspect-ratio preserving Lanczos3 downscaling — aşırı büyük resimlerde kalite kaybını önler
    let resized = resize_preserving_aspect(img, px);

    // JPEG olarak kaydet
    std::fs::create_dir_all(dst.parent()?).ok()?;
    let mut buf: Vec<u8> = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q());
    enc.encode_image(&resized).ok()?;
    std::fs::write(dst, &buf).ok();
    Some(buf)
}

fn generate_video_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let px = quality.px();
    for ss in ["00:00:01", "00:00:00"] {
        let out = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-ss", ss,
                "-i", src.to_str()?,
                "-vframes", "1",
                "-f", "image2",
                "-vcodec", "mjpeg",
                "pipe:1",
            ])
            .output();
        if let Ok(o) = out {
            if o.status.success() && !o.stdout.is_empty() {
                if let Ok(img) = image::load_from_memory(&o.stdout) {
                    let resized = resize_preserving_aspect(img, px);
                    std::fs::create_dir_all(dst.parent()?).ok()?;
                    let mut buf = Vec::new();
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
                        .encode_image(&resized).ok()?;
                    std::fs::write(dst, &buf).ok();
                    return Some(buf);
                }
            }
        }
    }
    None
}

fn generate_audio_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let px = quality.px();
    let out = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", src.to_str()?,
            "-an",
            "-vframes", "1",
            "-f", "image2",
            "-vcodec", "mjpeg",
            "pipe:1",
        ])
        .output();
    if let Ok(o) = out {
        if o.status.success() && !o.stdout.is_empty() {
            if let Ok(img) = image::load_from_memory(&o.stdout) {
                let resized = resize_preserving_aspect(img, px);
                std::fs::create_dir_all(dst.parent()?).ok()?;
                let mut buf = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
                    .encode_image(&resized).ok()?;
                std::fs::write(dst, &buf).ok();
                return Some(buf);
            }
        }
    }
    None
}

/// Aspect-ratio preserving Lanczos3 downscale (Nemo style) — eliminates quality degradation on large images
pub fn resize_preserving_aspect(img: image::DynamicImage, max_dim: u32) -> image::DynamicImage {
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 { return img; }

    if iw <= max_dim && ih <= max_dim {
        return img;
    }

    // High quality Lanczos3 filter — prevents aliasing and blurriness on DSLR / 4K / 8K images
    img.resize(max_dim, max_dim, image::imageops::FilterType::Lanczos3)
}

/// Cover resize + center crop
pub fn crop_to_fill(img: image::DynamicImage, tw: u32, th: u32) -> image::DynamicImage {
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 { return img; }

    // Scale to cover: fit en küçük kenarı hedef boyutuna
    let scale_x = tw as f64 / iw as f64;
    let scale_y = th as f64 / ih as f64;
    let scale   = scale_x.max(scale_y);

    let new_w = (iw as f64 * scale).ceil() as u32;
    let new_h = (ih as f64 * scale).ceil() as u32;

    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

    // Merkez crop
    let cx = (new_w.saturating_sub(tw)) / 2;
    let cy = (new_h.saturating_sub(th)) / 2;
    resized.crop_imm(cx, cy, tw.min(new_w), th.min(new_h))
}

fn generate_gif_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let path_str = src.to_str()?;
    let frames = crate::ui::preview_panel::load_gif_frames(path_str);
    let frame = frames.into_iter().next()?;
    let img_buf = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)?;
    let img = image::DynamicImage::ImageRgba8(img_buf);
    let px = quality.px();
    let resized = resize_preserving_aspect(img, px);

    std::fs::create_dir_all(dst.parent()?).ok()?;
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
        .encode_image(&resized).ok()?;
    std::fs::write(dst, &buf).ok();
    Some(buf)
}

fn generate_svg_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let px = quality.px();
    // rsvg-convert dene (librsvg)
    let tmp = dst.with_extension("_tmp.png");
    let ok = std::process::Command::new("rsvg-convert")
        .args(["-w", &px.to_string(), "-h", &px.to_string(),
               "-o", tmp.to_str()?, src.to_str()?])
        .status().ok()?.success();

    if ok && tmp.exists() {
        let img  = image::open(&tmp).ok()?;
        let _ = std::fs::remove_file(&tmp);
        let resized = resize_preserving_aspect(img, px);
        std::fs::create_dir_all(dst.parent()?).ok()?;
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
            .encode_image(&resized).ok()?;
        std::fs::write(dst, &buf).ok();
        return Some(buf);
    }

    // inkscape fallback
    let ok2 = std::process::Command::new("inkscape")
        .args(["--export-type=png",
               &format!("--export-width={}", px),
               &format!("--export-filename={}", dst.with_extension("_tmp2.png").display()),
               src.to_str()?])
        .status().ok()?.success();
    let tmp2 = dst.with_extension("_tmp2.png");
    if ok2 && tmp2.exists() {
        let img = image::open(&tmp2).ok()?;
        let _ = std::fs::remove_file(&tmp2);
        let resized = resize_preserving_aspect(img, px);
        std::fs::create_dir_all(dst.parent()?).ok()?;
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
            .encode_image(&resized).ok()?;
        std::fs::write(dst, &buf).ok();
        return Some(buf);
    }

    None
}

/// JPEG bytes'ı egui ColorImage'a çevir
pub fn jpeg_to_color_image(jpeg: &[u8]) -> Option<egui::ColorImage> {
    let img  = image::load_from_memory(jpeg).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize], rgba.as_raw()))
}

/// Tüm cache'i temizle
pub fn clear_cache() {
    let _ = std::fs::remove_dir_all(cache_dir());
}

pub fn cache_entry_count() -> usize {
    std::fs::read_dir(cache_dir())
        .map(|d| d.count()).unwrap_or(0)
}

/// Wayland Drag & Drop için dosyanın gerçek thumbnail'ini veya şık simgesini 64x64 BGRA olarak üretir
pub fn generate_drag_preview_bgra(paths: &[String]) -> Option<(u32, u32, Vec<u8>)> {
    if paths.is_empty() { return None; }
    let first = &paths[0];
    let path = Path::new(first);
    let count = paths.len();
    const W: usize = 64;
    const H: usize = 64;

    // 1. Önce görsel veya video önbelleğindeki thumbnail'i dene
    let mut loaded_img: Option<image::RgbaImage> = None;
    if let Some(jpeg) = get_thumbnail(first, ThumbQuality::Low) {
        if let Ok(dyn_img) = image::load_from_memory(&jpeg) {
            loaded_img = Some(dyn_img.thumbnail_exact(W as u32, H as u32).to_rgba8());
        }
    }
    if loaded_img.is_none() {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico") {
            if let Ok(dyn_img) = image::open(path) {
                loaded_img = Some(dyn_img.thumbnail_exact(W as u32, H as u32).to_rgba8());
            }
        }
    }

    let is_dir = path.is_dir();
    let mut bgra = vec![0u8; W * H * 4];

    if let Some(rgba) = loaded_img {
        let (img_w, img_h) = rgba.dimensions();
        let off_x = ((W as i32 - img_w as i32) / 2).max(0) as usize;
        let off_y = ((H as i32 - img_h as i32) / 2).max(0) as usize;

        for y in 0..H {
            for x in 0..W {
                let idx = (y * W + x) * 4;
                // Yuvarlatılmış köşeler (radius = 7)
                let rx = if x < 7 { 7 - x } else if x >= W - 7 { x - (W - 8) } else { 0 };
                let ry = if y < 7 { 7 - y } else if y >= H - 7 { y - (H - 8) } else { 0 };
                if rx * rx + ry * ry > 49 {
                    continue; // Şeffaf dış alan
                }

                // Kenarlık vurgusu
                let is_border = x == 0 || x == W - 1 || y == 0 || y == H - 1
                    || (rx * rx + ry * ry >= 36);

                if is_border {
                    bgra[idx] = 255;     // B
                    bgra[idx + 1] = 140; // G
                    bgra[idx + 2] = 160; // R
                    bgra[idx + 3] = 240; // A
                    continue;
                }

                // Önizleme pikseli
                if x >= off_x && x < off_x + img_w as usize && y >= off_y && y < off_y + img_h as usize {
                    let ix = (x - off_x) as u32;
                    let iy = (y - off_y) as u32;
                    let p = rgba.get_pixel(ix, iy);
                    let a = p[3] as f32 / 255.0;
                    bgra[idx] = (p[2] as f32 * a) as u8;     // B (premultiplied)
                    bgra[idx + 1] = (p[1] as f32 * a) as u8; // G
                    bgra[idx + 2] = (p[0] as f32 * a) as u8; // R
                    bgra[idx + 3] = p[3];                    // A
                } else {
                    bgra[idx] = 30;
                    bgra[idx + 1] = 20;
                    bgra[idx + 2] = 20;
                    bgra[idx + 3] = 220;
                }
            }
        }
    } else {
        // Görsel olmayan dosyalar için şık neon dosya/klasör kartı
        let (base_b, base_g, base_r) = if is_dir {
            (40, 180, 240) // Amber klasör sarısı
        } else {
            (240, 100, 130) // Şık mor-mavi belge tonu
        };

        for y in 0..H {
            for x in 0..W {
                let idx = (y * W + x) * 4;
                let rx = if x < 8 { 8 - x } else if x >= W - 8 { x - (W - 9) } else { 0 };
                let ry = if y < 8 { 8 - y } else if y >= H - 8 { y - (H - 9) } else { 0 };
                if rx * rx + ry * ry > 64 {
                    continue;
                }

                let is_border = x <= 1 || x >= W - 2 || y <= 1 || y >= H - 2 || (rx * rx + ry * ry >= 45);
                if is_border {
                    bgra[idx] = base_b;
                    bgra[idx + 1] = base_g;
                    bgra[idx + 2] = base_r;
                    bgra[idx + 3] = 255;
                } else if is_dir && y >= 14 && y <= 18 && x >= 8 && x <= 28 {
                    // Klasör kapağı sekmesi
                    bgra[idx] = base_b;
                    bgra[idx + 1] = base_g;
                    bgra[idx + 2] = base_r;
                    bgra[idx + 3] = 230;
                } else {
                    // Yarı saydam gövde
                    bgra[idx] = (base_b as f32 * 0.25) as u8 + 15;
                    bgra[idx + 1] = (base_g as f32 * 0.25) as u8 + 15;
                    bgra[idx + 2] = (base_r as f32 * 0.25) as u8 + 15;
                    bgra[idx + 3] = 230;
                }
            }
        }
    }

    // Çoklu seçim rozeti (+N badge)
    if count > 1 {
        let cx: i32 = 50;
        let cy: i32 = 50;
        let cr: i32 = 11;
        for y in (cy - cr)..(cy + cr) {
            for x in (cx - cr)..(cx + cr) {
                if x >= 0 && x < W as i32 && y >= 0 && y < H as i32 {
                    let dx = x - cx;
                    let dy = y - cy;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq <= cr * cr {
                        let idx = (y as usize * W + x as usize) * 4;
                        if dist_sq >= (cr - 2) * (cr - 2) {
                            bgra[idx] = 255; bgra[idx + 1] = 255; bgra[idx + 2] = 255; bgra[idx + 3] = 255;
                        } else {
                            bgra[idx] = 100; bgra[idx + 1] = 50; bgra[idx + 2] = 255; bgra[idx + 3] = 245;
                        }
                    }
                }
            }
        }
    }

    Some((W as u32, H as u32, bgra))
}
