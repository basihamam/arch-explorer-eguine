// preview.rs — Dosya önizleme
// image → base64, text → ilk 100 satır, video/audio → ffprobe, binary → hex dump

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB (büyük resimler rahatça önizlensin)
const MAX_TEXT_BYTES:  u64 =   2 * 1024 * 1024; //   2 MB
const PREVIEW_LINES:   usize = 100;
const HEX_BYTES:       usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePreview {
    /// "image" | "text" | "video" | "audio" | "dir" | "binary"
    pub kind:         String,
    pub mime_type:    String,
    pub size:         u64,
    pub image_b64:    Option<String>,
    pub image_rgba:   Option<(Vec<u8>, u32, u32)>, // Pre-decoded (raw RGBA, width, height) in background
    pub text_content: Option<String>,
    pub media_info:   Option<serde_json::Value>,
    pub line_count:   Option<usize>,
    pub is_truncated: bool,
    pub language:     Option<String>, // text highlight hint
    pub dir_entries:  Option<Vec<(String, bool)>>, // Pre-collected first 10 entries in background
}

pub fn get_file_preview(path: String) -> Result<FilePreview, String> {
    if path.starts_with("adb://") {
        if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(&path) {
            let temp_dir = std::env::temp_dir().join("arch-explorer-adb-prev");
            let _ = std::fs::create_dir_all(&temp_dir);
            let fname = std::path::Path::new(&dev_path)
                .file_name().map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "prev_file".to_string());
            let local_target = temp_dir.join(&fname);
            let local_str = local_target.to_string_lossy().to_string();

            let mut cmd = Command::new("adb");
            cmd.args(["-s", &serial, "pull", &dev_path, &local_str]);
            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    return get_file_preview(local_str);
                }
            }
            return Err("ADB dosyası önizleme için indirilemedi".to_string());
        }
    }

    let p = PathBuf::from(&path);

    let (meta_is_dir, meta_size, is_special) = if p.exists() {
        if let Ok(meta) = fs::symlink_metadata(&p) {
            #[cfg(unix)]
            {
                let ft = meta.file_type();
                if ft.is_fifo() || ft.is_socket() || ft.is_char_device() || ft.is_block_device() {
                    let desc = if ft.is_fifo() {
                        "FIFO (Adlandırılmış Boru)"
                    } else if ft.is_socket() {
                        "Unix Soketi"
                    } else if ft.is_char_device() {
                        "Karakter Cihazı"
                    } else {
                        "Blok Cihazı"
                    };
                    return Ok(FilePreview {
                        kind: "binary".into(),
                        mime_type: "inode/special".into(),
                        size: 0,
                        image_b64: None,
                        image_rgba: None,
                        text_content: Some(format!("{} önizlenemez.", desc)),
                        media_info: None,
                        line_count: None,
                        is_truncated: false,
                        language: None,
                        dir_entries: None,
                    });
                }
            }
            let real = fs::metadata(&p).unwrap_or(meta);
            (real.is_dir(), if real.is_dir() { 0 } else { real.len() }, false)
        } else {
            (false, 0, false)
        }
    } else {
        // Normal p.exists() false dönebilir (root klasörlerinin altındaki dosyalar için)
        // Root şifresi varsa sudo python ile stat dene
        if let Some(pw) = crate::backend::fs_ops::get_root_password() {
            let py_stat = format!(r#"
import os, stat, json
try:
    st = os.stat("{path}")
    print(json.dumps({{"ok": True, "is_dir": stat.S_ISDIR(st.st_mode), "size": st.st_size}}))
except Exception as e:
    print(json.dumps({{"ok": False, "err": str(e)}}))
"#);
            let mut child = Command::new("sudo")
                .args(["-S", "python3", "-c", &py_stat])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(format!("{}\n", pw).as_bytes());
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            if out.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                        let is_dir = val.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                        let sz = val.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                        (is_dir, if is_dir { 0 } else { sz }, false)
                    } else {
                        return Err(format!("Bulunamadı: {}", path));
                    }
                } else {
                    return Err(format!("Bulunamadı: {}", path));
                }
            } else {
                return Err(format!("Bulunamadı: {}", path));
            }
        } else {
            return Err(format!("Bulunamadı: {}", path));
        }
    };

    let size = meta_size;
    let mime = if meta_is_dir {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(&p).first_or_octet_stream().to_string()
    };

    // ── Klasör — güvenli sınırla say ve ilk 10 öğeyi topla ────────────
    if meta_is_dir {
        let mut count = 0;
        let mut is_plus = false;
        let mut entries = Vec::new();

        if let Ok(rd) = fs::read_dir(&p) {
            for entry in rd.flatten() {
                count += 1;
                if entries.len() < 10 {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry.file_type().map(|f| f.is_dir()).unwrap_or(false);
                    entries.push((name, is_dir));
                }
                if count > 1000 {
                    is_plus = true;
                    break;
                }
            }
        } else if let Some(pw) = crate::backend::fs_ops::get_root_password() {
            // Sudo ile klasör içeriğini tara
            let py_dir = format!(r#"
import os, stat, json
entries = []
count = 0
try:
    with os.scandir("{path}") as it:
        for entry in it:
            count += 1
            if len(entries) < 10:
                try:
                    st = entry.stat(follow_symlinks=False)
                    entries.append((entry.name, stat.S_ISDIR(st.st_mode)))
                except Exception:
                    continue
            if count > 1000:
                break
    print(json.dumps({{"status": "ok", "count": count, "entries": entries}}))
except Exception as e:
    print(json.dumps({{"status": "error", "message": str(e)}}))
"#);
            let mut child = Command::new("sudo")
                .args(["-S", "python3", "-c", &py_dir])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(format!("{}\n", pw).as_bytes());
            }
            if let Ok(out) = child.wait_with_output() {
                if out.status.success() {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                        if val.get("status").and_then(|v| v.as_str()) == Some("ok") {
                            count = val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            if let Some(arr) = val.get("entries").and_then(|v| v.as_array()) {
                                for item in arr {
                                    if let (Some(n), Some(d)) = (item.get(0).and_then(|v| v.as_str()), item.get(1).and_then(|v| v.as_bool())) {
                                        entries.push((n.to_string(), d));
                                    }
                                }
                            }
                            if count > 1000 {
                                is_plus = true;
                            }
                        }
                    }
                }
            }
        }

        entries.sort_by(|a, b| a.0.cmp(&b.0));
        return Ok(FilePreview {
            kind: "dir".into(),
            mime_type: mime,
            size: 0,
            image_b64: None,
            image_rgba: None,
            text_content: None,
            media_info: None,
            line_count: Some(count),
            is_truncated: is_plus,
            language: None,
            dir_entries: Some(entries),
        });
    }

    // ── Resim — arka planda decode ve boyutlandır ────────────────────
    if mime.starts_with("image/") {
        if size > MAX_IMAGE_BYTES {
            return Ok(FilePreview {
                kind: "image".into(),
                mime_type: mime,
                size,
                image_b64: None,
                image_rgba: None,
                text_content: Some(format!("Görsel çok büyük ({:.1} MB > 100 MB)", size as f64 / 1_048_576.0)),
                media_info: None,
                line_count: None,
                is_truncated: false,
                language: None,
                dir_entries: None,
            });
        }
        let bytes = fs::read(&p).or_else(|_| crate::backend::fs_ops::read_file_bytes_elevated(&path))
            .map_err(|e| format!("Resim okunamadı: {}", e))?;
        let mut image_rgba = image::load_from_memory(&bytes).ok().map(|img| {
            let (w, h) = (img.width(), img.height());
            let img = if w > 2560 || h > 2560 {
                img.resize(2560, 2560, image::imageops::FilterType::Lanczos3)
            } else {
                img
            };
            let rgba = img.to_rgba8();
            let (nw, nh) = rgba.dimensions();
            (rgba.into_raw(), nw, nh)
        });

        // Eğer Rust `image` kütüphanesi formatı çözemediyse ffmpeg ile dene
        if image_rgba.is_none() {
            if let Ok(out) = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-i", &path, "-vf", "scale='min(2560,iw)':-1:flags=lanczos", "-vframes", "1", "-f", "image2", "-vcodec", "png", "pipe:1"])
                .output()
            {
                if out.status.success() && !out.stdout.is_empty() {
                    if let Ok(img) = image::load_from_memory(&out.stdout) {
                        let rgba = img.to_rgba8();
                        let (nw, nh) = rgba.dimensions();
                        image_rgba = Some((rgba.into_raw(), nw, nh));
                    }
                }
            }
        }

        return Ok(FilePreview {
            kind: "image".into(),
            mime_type: mime,
            size,
            image_b64: None,
            image_rgba,
            text_content: None,
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
            dir_entries: None,
        });
    }

    // ── Video / Audio — zaman aşımlı ffprobe + thumbnail çıkarma ──────
    if mime.starts_with("video/") || mime.starts_with("audio/") {
        let kind = if mime.starts_with("video/") { "video" } else { "audio" };
        let media_info = try_ffprobe(&path);

        let thumb_bytes = if kind == "video" {
            extract_video_frame(&path)
        } else {
            extract_audio_cover(&path)
        };

        let image_rgba = thumb_bytes.and_then(|bytes| {
            image::load_from_memory(&bytes).ok().map(|img| {
                let img = if img.width() > 1280 || img.height() > 1280 {
                    img.resize(1280, 1280, image::imageops::FilterType::Lanczos3)
                } else {
                    img
                };
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                (rgba.into_raw(), w, h)
            })
        });

        return Ok(FilePreview {
            kind: kind.into(),
            mime_type: mime,
            size,
            image_b64: None,
            image_rgba,
            text_content: None,
            media_info,
            line_count: None,
            is_truncated: false,
            language: None,
            dir_entries: None,
        });
    }

    // ── Metin ────────────────────────────────────────────────────────
    let lang = detect_language(&p);
    let is_text = lang.is_some()
        || mime.starts_with("text/")
        || mime.contains("json")
        || mime.contains("javascript")
        || mime.contains("xml");

    if is_text && size <= MAX_TEXT_BYTES {
        let text_res = fs::read_to_string(&p)
            .or_else(|_| crate::backend::fs_ops::read_file_bytes_elevated(&path).and_then(|b| String::from_utf8(b).map_err(|e| e.to_string())));

        match text_res {
            Ok(content) => {
                let mut lines = Vec::new();
                let mut total = 0;
                for line in content.lines() {
                    total += 1;
                    if lines.len() < PREVIEW_LINES {
                        if line.len() > 300 {
                            let truncated: String = line.chars().take(300).collect();
                            lines.push(format!("{}…", truncated));
                        } else {
                            lines.push(line.to_string());
                        }
                    }
                }
                let truncated = total > PREVIEW_LINES;
                let preview = lines.join("\n");
                return Ok(FilePreview {
                    kind: "text".into(),
                    mime_type: mime,
                    size,
                    image_b64: None,
                    image_rgba: None,
                    text_content: Some(preview),
                    media_info: None,
                    dir_entries: None,
                    line_count: Some(total),
                    is_truncated: truncated,
                    language: lang,
                });
            }
            Err(_) => { /* binary'ye düş */ }
        }
    }

    // ── Binary — hex dump ────────────────────────────────────────────
    let buf = match fs::File::open(&p) {
        Ok(mut f) => {
            let mut buf = vec![0u8; HEX_BYTES];
            let n = f.read(&mut buf).unwrap_or(0);
            buf.truncate(n);
            buf
        }
        Err(_) => {
            let mut elevated = crate::backend::fs_ops::read_file_bytes_elevated(&path).map_err(|e| e.to_string())?;
            elevated.truncate(HEX_BYTES);
            elevated
        }
    };

    let hex = buf
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let hex_part: String = chunk.iter().map(|b| format!("{:02X} ", b)).collect();
            let ascii: String = chunk
                .iter()
                .map(|b| if b.is_ascii_graphic() || *b == b' ' { *b as char } else { '.' })
                .collect();
            format!("{:08X}  {:48} |{}|", i * 16, hex_part, ascii)
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(FilePreview {
        kind: "binary".into(),
        mime_type: mime,
        size,
        image_b64: None,
        image_rgba: None,
        text_content: Some(hex),
        media_info: None,
        dir_entries: None,
        line_count: None,
        is_truncated: size > HEX_BYTES as u64,
        language: Some("hex".into()),
    })
}

/// Dosya uzantısından syntax highlight dili tahmin et
fn detect_language(p: &PathBuf) -> Option<String> {
    let ext = p.extension()?.to_str()?.to_lowercase();
    let lang = match ext.as_str() {
        "rs"                         => "rust",
        "py"                         => "python",
        "js" | "mjs" | "cjs"        => "javascript",
        "ts"                         => "typescript",
        "svelte"                     => "svelte",
        "html" | "htm"               => "html",
        "css" | "scss" | "less"      => "css",
        "json"                       => "json",
        "toml"                       => "toml",
        "yaml" | "yml"               => "yaml",
        "md" | "markdown"            => "markdown",
        "sh" | "bash" | "zsh"        => "bash",
        "lua"                        => "lua",
        "go"                         => "go",
        "c" | "h"                    => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "java"                       => "java",
        "kt"                         => "kotlin",
        "dart"                       => "dart",
        "sql"                        => "sql",
        "xml"                        => "xml",
        "txt" | "log" | "conf"
        | "cfg" | "ini" | "env"      => "plaintext",
        _                            => return None,
    };
    Some(lang.to_string())
}

/// ffprobe ile media metadata al — max 1.5 saniye zaman aşımı ile
fn try_ffprobe(path: &str) -> Option<serde_json::Value> {
    let mut child = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() { return None; }
                let mut stdout = child.stdout.take()?;
                let mut text = String::new();
                stdout.read_to_string(&mut text).ok()?;
                let raw: serde_json::Value = serde_json::from_str(&text).ok()?;
                
                // Ham JSON yerine temiz, anlaşılır ve düzenli alanlar çıkar (sorcerer PreviewPanel stili)
                let mut map = serde_json::Map::new();
                if let Some(fmt) = raw.get("format") {
                    if let Some(dur_str) = fmt.get("duration").and_then(|v| v.as_str()) {
                        if let Ok(secs) = dur_str.parse::<f64>() {
                            let total_secs = secs.round() as u64;
                            let h = total_secs / 3600;
                            let m = (total_secs % 3600) / 60;
                            let s = total_secs % 60;
                            let dur_fmt = if h > 0 {
                                format!("{}:{:02}:{:02}", h, m, s)
                            } else {
                                format!("{:02}:{:02}", m, s)
                            };
                            map.insert("Duration".to_string(), serde_json::Value::String(dur_fmt));
                            map.insert("duration_secs".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(secs).unwrap_or(serde_json::Number::from(0))));
                        }
                    }
                    if let Some(sz_str) = fmt.get("size").and_then(|v| v.as_str()) {
                        if let Ok(sz) = sz_str.parse::<u64>() {
                            const K: u64 = 1024;
                            let sz_fmt = if sz < K {
                                format!("{} B", sz)
                            } else if sz < K * K {
                                format!("{:.1} KB", sz as f64 / K as f64)
                            } else if sz < K * K * K {
                                format!("{:.1} MB", sz as f64 / (K * K) as f64)
                            } else {
                                format!("{:.2} GB", sz as f64 / (K * K * K) as f64)
                            };
                            map.insert("Size".to_string(), serde_json::Value::String(sz_fmt));
                        }
                    }
                    if let Some(br_str) = fmt.get("bit_rate").and_then(|v| v.as_str()) {
                        if let Ok(br) = br_str.parse::<u64>() {
                            map.insert("Bitrate".to_string(), serde_json::Value::String(format!("{} kbps", br / 1000)));
                        }
                    }
                    if let Some(format_name) = fmt.get("format_long_name").or_else(|| fmt.get("format_name")).and_then(|v| v.as_str()) {
                        let short_fmt = format_name.split(',').next().unwrap_or(format_name).trim();
                        map.insert("Format".to_string(), serde_json::Value::String(short_fmt.to_string()));
                    }
                }
                if let Some(streams) = raw.get("streams").and_then(|v| v.as_array()) {
                    for s in streams {
                        let ctype = s.get("codec_type").and_then(|v| v.as_str()).unwrap_or("");
                        let cname = s.get("codec_name").and_then(|v| v.as_str()).unwrap_or("");
                        if ctype == "video" && !map.contains_key("Video") {
                            let w = s.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
                            let h = s.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
                            if w > 0 && h > 0 {
                                map.insert("Video".to_string(), serde_json::Value::String(format!("{} {}x{}", cname.to_lowercase(), w, h)));
                            } else {
                                map.insert("Video".to_string(), serde_json::Value::String(cname.to_lowercase()));
                            }
                            if let Some(fps_str) = s.get("r_frame_rate").and_then(|v| v.as_str()) {
                                let parts: Vec<&str> = fps_str.split('/').collect();
                                if parts.len() == 2 {
                                    if let (Ok(num), Ok(den)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                                        if den > 0.0 {
                                            map.insert("FPS".to_string(), serde_json::Value::String(format!("{:.2} fps", num / den)));
                                        }
                                    }
                                }
                            }
                        } else if ctype == "audio" && !map.contains_key("Audio") {
                            let channels = s.get("channels").and_then(|v| v.as_u64()).unwrap_or(2);
                            let sr = s.get("sample_rate").and_then(|v| v.as_str()).unwrap_or("44100");
                            map.insert("Audio".to_string(), serde_json::Value::String(format!("{} {}ch {}Hz", cname.to_lowercase(), channels, sr)));
                        }
                    }
                }
                return Some(serde_json::Value::Object(map));
            }
            Ok(None) => {
                if start.elapsed().as_millis() > 1500 {
                    child.kill().ok();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            Err(_) => return None,
        }
    }
}

/// Video'dan ilk saniyeden kare çıkar
fn extract_video_frame(path: &str) -> Option<Vec<u8>> {
    for ss in ["00:00:01", "00:00:00"] {
        let out = Command::new("ffmpeg")
            .args([
                "-y",
                "-ss", ss,
                "-i", path,
                "-vframes", "1",
                "-f", "image2",
                "-vcodec", "mjpeg",
                "pipe:1",
            ])
            .output();
        if let Ok(o) = out {
            if o.status.success() && !o.stdout.is_empty() {
                return Some(o.stdout);
            }
        }
    }
    None
}

/// Ses dosyasından gömülü albüm kapağını çıkar
fn extract_audio_cover(path: &str) -> Option<Vec<u8>> {
    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", path,
            "-an",
            "-vframes", "1",
            "-f", "image2",
            "-vcodec", "mjpeg",
            "pipe:1",
        ])
        .output();
    if let Ok(o) = out {
        if o.status.success() && !o.stdout.is_empty() {
            return Some(o.stdout);
        }
    }
    None
}
