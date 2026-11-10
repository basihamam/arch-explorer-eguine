/// clipboard.rs — Dosya kopyala/kes/yapıştır + undo stack
/// Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Z, Delete hepsini buradan yönetiyoruz
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use anyhow::Result;

// ---- Global state (Tauri app'te tek instance) ----

/// Clipboard: neyi kopyaladın/kestiktin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardState {
    pub op: ClipboardOp,
    pub paths: Vec<String>,
}

/// Undo için yapılan işlem kaydı
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UndoAction {
    /// Dosya/klasör taşındı: geri al = tekrar taşı
    Move { from: String, to: String },
    /// Dosya/klasör kopyalandı: geri al = hedefi sil
    Copy { dest: String },
    /// Rename: geri al = eski isime döndür
    Rename { old_path: String, new_path: String },
    /// Oluşturma: geri al = sil
    Create { path: String },
    /// Silme: geri al = trash'ten geri al (trash-cli varsa)
    Delete { path: String, was_dir: bool },
    /// Üzerine yazma (Replace): geri al = hedefi silip yedeklenen eski dosyayı/klasörü geri getir
    Replace {
        dest: String,
        backup_path: String,
        was_dir: bool,
        is_copy: bool,
        orig_src: Option<String>,
    },
    /// Toplu işlem (tek seferde çoklu kopyalama/taşıma/silme — tek bir Ctrl+Z ile hepsi geri alınır)
    Batch(Vec<UndoAction>),
}

impl UndoAction {
    pub fn collect_targets(&self, out: &mut Vec<String>) {
        match self {
            UndoAction::Copy { dest } => out.push(dest.clone()),
            UndoAction::Move { from, .. } => out.push(from.clone()),
            UndoAction::Replace { dest, .. } => out.push(dest.clone()),
            UndoAction::Create { path } => out.push(path.clone()),
            UndoAction::Batch(actions) => {
                for a in actions {
                    a.collect_targets(out);
                }
            }
            _ => {}
        }
    }

    pub fn target_dest(&self) -> Option<String> {
        let mut targets = Vec::new();
        self.collect_targets(&mut targets);
        targets.into_iter().next()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    pub message: String,
    pub restored_paths: Vec<String>,
}

// Thread-safe global state
static CLIPBOARD: Mutex<Option<ClipboardState>> = Mutex::new(None);
static UNDO_STACK: Mutex<Vec<UndoAction>> = Mutex::new(Vec::new());
static LAST_SYSTEM_CLIPBOARD_CHECK: Mutex<Option<(Option<ClipboardState>, std::time::Instant)>> = Mutex::new(None);

const MAX_UNDO: usize = 50;

// ---- Clipboard commands ----

/// Seçili dosyaları clipboard'a kopyala (Ctrl+C)
pub fn clipboard_copy(paths: Vec<String>) -> Result<(), String> {
    let state = ClipboardState {
        op: ClipboardOp::Copy,
        paths: paths.clone(),
    };
    {
        let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        *cb = Some(state.clone());
    }
    if let Ok(mut cache) = LAST_SYSTEM_CLIPBOARD_CHECK.lock() {
        *cache = Some((Some(state), std::time::Instant::now()));
    }
    copy_files_to_system_clipboard_async(paths, false);
    Ok(())
}

/// Seçili dosyaları clipboard'a kes (Ctrl+X)
pub fn clipboard_cut(paths: Vec<String>) -> Result<(), String> {
    let state = ClipboardState {
        op: ClipboardOp::Cut,
        paths: paths.clone(),
    };
    {
        let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        *cb = Some(state.clone());
    }
    if let Ok(mut cache) = LAST_SYSTEM_CLIPBOARD_CHECK.lock() {
        *cache = Some((Some(state), std::time::Instant::now()));
    }
    copy_files_to_system_clipboard_async(paths, true);
    Ok(())
}

/// Sistem panosunu arka planda günceller — asla UI thread'i dondurmaz
pub fn copy_to_system_clipboard_async(text: String) {
    if text.is_empty() { return; }
    std::thread::spawn(move || {
        // 1. Wayland ortamında wl-copy dene
        if let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return;
                }
            }
        }

        // 2. X11 ortamında xclip dene
        if let Ok(mut child) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return;
                }
            }
        }

        // 3. Fallback: arboard (arka plan iş parçacığında olduğu için UI donmaz)
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(text);
        }
    });
}

/// Dosyaları sistem panosuna tüm MIME türleriyle (text/uri-list, x-special/gnome-copied-files, text/plain) yazar
/// Harici uygulamalarla (Firefox, Chrome, Dolphin, Nautilus vb.) tam uyumlu çalışır
pub fn copy_files_to_system_clipboard_async(paths: Vec<String>, is_cut: bool) {
    if paths.is_empty() { return; }
    std::thread::spawn(move || {
        let plain_text = paths.join("\n");
        let uris: Vec<String> = paths.iter().map(|p| {
            let encoded: String = p.chars().map(|c| {
                if c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '_' || c == '-' || c == '~' {
                    c.to_string()
                } else {
                    format!("%{:02X}", c as u8)
                }
            }).collect();
            format!("file://{}", encoded)
        }).collect();

        let uri_list = uris.join("\r\n") + "\r\n";
        let action_str = if is_cut { "cut" } else { "copy" };
        let gnome_copied = format!("{}\n{}", action_str, uris.join("\n")) + "\n";

        // 1. Wayland ortamında wl-copy ile text/uri-list yaz
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Ok(mut child) = std::process::Command::new("wl-copy")
                .args(["-t", "text/uri-list"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(uri_list.as_bytes());
                }
                let _ = child.wait();
            }
        }

        // 2. X11 ortamında xclip ile yaz
        if std::env::var("DISPLAY").is_ok() {
            let _ = std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "text/uri-list"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(uri_list.as_bytes());
                    }
                    child.wait()
                });

            let _ = std::process::Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(plain_text.as_bytes());
                    }
                    child.wait()
                });
        }

        // 3. Arboard ile metin kopyala (arka planda çalışır, UI asla etkilenmez)
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(plain_text);
        }
    });
}

/// Harici komutları UI donmasını önlemek için ayrı thread'de kısa zaman aşımıyla çalıştırır
fn get_command_output_fast(cmd: &str, args: &[&str], timeout_ms: u64) -> Option<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let cmd_str = cmd.to_string();
    let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    std::thread::spawn(move || {
        let output = std::process::Command::new(&cmd_str)
            .args(&args_vec)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let _ = tx.send(output.ok());
    });

    match rx.recv_timeout(std::time::Duration::from_millis(timeout_ms)) {
        Ok(Some(output)) if output.status.success() => Some(output.stdout),
        _ => None,
    }
}

/// Clipboard içeriğini anında döner — UI her karede render edilirken ASLA harici komut çalıştırmaz
pub fn clipboard_get() -> Option<ClipboardState> {
    if let Ok(cb) = CLIPBOARD.lock() {
        if let Some(ref state) = *cb {
            return Some(state.clone());
        }
    }
    None
}

/// Ctrl+V veya yapıştır tıklandığında taze sistem panosunu kontrol et
pub fn clipboard_get_fresh() -> Option<ClipboardState> {
    // 1. Önce sistem panosundan (harici uygulamalardan) yeni kopyalanmış dosya var mı kontrol et
    if let Some(sys) = get_clipboard_state_from_system() {
        let valid_paths: Vec<String> = sys.paths.into_iter()
            .filter(|p| std::path::Path::new(p).exists())
            .collect();
        if !valid_paths.is_empty() {
            let state = ClipboardState { op: sys.op, paths: valid_paths };
            if let Ok(mut cb) = CLIPBOARD.lock() {
                *cb = Some(state.clone());
            }
            return Some(state);
        }
    }

    // 2. Sistem panosunda harici dosya yoksa, dahili panoyu kullan
    if let Ok(cb) = CLIPBOARD.lock() {
        if let Some(ref state) = *cb {
            let valid_paths: Vec<String> = state.paths.iter()
                .filter(|p| std::path::Path::new(p).exists())
                .cloned()
                .collect();
            if !valid_paths.is_empty() {
                return Some(ClipboardState {
                    op: state.op.clone(),
                    paths: valid_paths,
                });
            }
        }
    }

    None
}

/// Wayland (wl-paste) ve X11 (xclip) üzerinden panodaki dosya yollarını çeker (ASLA arboard ile UI thread'i dondurmaz!)
pub fn get_clipboard_state_from_system() -> Option<ClipboardState> {
    // Wayland kontrolü
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Some(out) = get_command_output_fast("wl-paste", &["-t", "text/uri-list"], 80) {
            let s = String::from_utf8_lossy(&out);
            let (paths, op) = parse_uri_list_with_op(&s);
            if !paths.is_empty() {
                return Some(ClipboardState { op, paths });
            }
        }
        if let Some(out) = get_command_output_fast("wl-paste", &["-t", "x-special/gnome-copied-files"], 80) {
            let s = String::from_utf8_lossy(&out);
            let (paths, op) = parse_uri_list_with_op(&s);
            if !paths.is_empty() {
                return Some(ClipboardState { op, paths });
            }
        }
        if let Some(out) = get_command_output_fast("wl-paste", &["--no-newline"], 80) {
            let s = String::from_utf8_lossy(&out);
            let (paths, op) = parse_uri_list_with_op(&s);
            if !paths.is_empty() {
                return Some(ClipboardState { op, paths });
            }
        }
    }

    // X11 kontrolü
    if std::env::var("DISPLAY").is_ok() {
        if let Some(out) = get_command_output_fast("xclip", &["-selection", "clipboard", "-t", "text/uri-list", "-o"], 80) {
            let s = String::from_utf8_lossy(&out);
            let (paths, op) = parse_uri_list_with_op(&s);
            if !paths.is_empty() {
                return Some(ClipboardState { op, paths });
            }
        }
        if let Some(out) = get_command_output_fast("xclip", &["-selection", "clipboard", "-o"], 80) {
            let s = String::from_utf8_lossy(&out);
            let (paths, op) = parse_uri_list_with_op(&s);
            if !paths.is_empty() {
                return Some(ClipboardState { op, paths });
            }
        }
    }

    None
}

/// Geriye dönük uyumluluk: Sistem panosundan sadece dosya yollarını çeker
pub fn get_files_from_system_clipboard() -> Vec<String> {
    get_clipboard_state_from_system().map(|cb| cb.paths).unwrap_or_default()
}

pub fn parse_uri_list_string(raw: &str) -> Vec<String> {
    parse_uri_list_with_op(raw).0
}

pub fn parse_uri_list_with_op(raw: &str) -> (Vec<String>, ClipboardOp) {
    let mut results = Vec::new();
    let mut op = ClipboardOp::Copy;

    for line in raw.lines() {
        let trimmed = line.trim().trim_matches(|c| c == '\'' || c == '"');
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "cut" {
            op = ClipboardOp::Cut;
            continue;
        }
        if trimmed == "copy" {
            op = ClipboardOp::Copy;
            continue;
        }

        let clean = if let Some(stripped) = trimmed.strip_prefix("file://") {
            let without_host = if let Some(after_localhost) = stripped.strip_prefix("localhost/") {
                format!("/{}", after_localhost)
            } else if stripped.starts_with('/') {
                stripped.to_string()
            } else {
                format!("/{}", stripped)
            };
            percent_decode_path(&without_host)
        } else if trimmed.starts_with('/') {
            trimmed.to_string()
        } else {
            continue;
        };

        if std::path::Path::new(&clean).exists() {
            results.push(clean);
        }
    }
    (results, op)
}

fn percent_decode_path(input: &str) -> String {
    let mut bytes = Vec::new();
    let input_bytes = input.as_bytes();
    let mut i = 0;
    while i < input_bytes.len() {
        if input_bytes[i] == b'%' && i + 2 < input_bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&input_bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    bytes.push(b);
                    i += 3;
                    continue;
                }
            }
        }
        bytes.push(input_bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Clipboard'ı temizle
pub fn clipboard_clear() -> Result<(), String> {
    {
        let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        *cb = None;
    }
    if let Ok(mut cache) = LAST_SYSTEM_CLIPBOARD_CHECK.lock() {
        *cache = Some((None, std::time::Instant::now()));
    }
    Ok(())
}

/// Clipboard'daki dosyaları hedef dizine yapıştır (Ctrl+V)
/// Copy ise dosyaları kopyalar, Cut ise taşır
pub fn clipboard_paste(dest_dir: String) -> Result<Vec<String>, String> {
    let state = {
        let cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        cb.clone().ok_or_else(|| "Clipboard boş".to_string())?
    };

    let dest = PathBuf::from(&dest_dir);
    if !dest.is_dir() {
        return Err(format!("Hedef dizin bulunamadı: {}", dest_dir));
    }

    let mut pasted = vec![];
    let mut undo_actions = vec![];

    for src_path in &state.paths {
        let src = PathBuf::from(src_path);
        if !src.exists() {
            continue; // Cut sonrası tekrar paste gibi durumlar
        }

        let file_name = src.file_name()
            .ok_or_else(|| format!("Geçersiz dosya adı: {}", src_path))?;

        // Çakışma varsa yeni isim ver
        let dest_path = resolve_conflict(&dest, file_name.to_string_lossy().as_ref());

        match state.op {
            ClipboardOp::Copy => {
                copy_recursive(&src, &dest_path)
                    .map_err(|e| format!("Kopyalama hatası {}: {}", src_path, e))?;

                undo_actions.push(UndoAction::Copy {
                    dest: dest_path.to_string_lossy().to_string(),
                });
            }
            ClipboardOp::Cut => {
                // Önce kopyala, sonra sil (cross-device safe)
                let actual_src = src_path.clone();
                move_item_inner(&src, &dest_path)
                    .map_err(|e| format!("Taşıma hatası {}: {}", src_path, e))?;

                undo_actions.push(UndoAction::Move {
                    from: dest_path.to_string_lossy().to_string(),
                    to: actual_src,
                });
            }
        }

        pasted.push(dest_path.to_string_lossy().to_string());
    }

    // Cut sonrası clipboard'ı temizle
    if state.op == ClipboardOp::Cut {
        let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        *cb = None;
    }

    // Undo stack'e ekle
    push_undo_batch(undo_actions)?;

    Ok(pasted)
}

// ---- Undo ----

/// Son işlemi geri al (Ctrl+Z)
pub fn undo_last() -> Result<UndoResult, String> {
    let action = {
        let mut stack = UNDO_STACK.lock().map_err(|e| e.to_string())?;
        stack.pop().ok_or_else(|| "Geri alınacak işlem yok".to_string())?
    };

    let mut restored_paths = Vec::new();
    let message = undo_single_action(&action, &mut restored_paths)?;
    Ok(UndoResult {
        message,
        restored_paths,
    })
}

fn undo_single_action(action: &UndoAction, restored: &mut Vec<String>) -> Result<String, String> {
    let desc = match action {
        UndoAction::Move { from, to } => {
            let f = PathBuf::from(from);
            let t = PathBuf::from(to);
            move_item_inner(&f, &t)
                .map_err(|e| format!("Undo taşıma hatası: {}", e))?;
            restored.push(to.clone());
            format!("Geri alındı: {} → {}", from, to)
        }
        UndoAction::Copy { dest } => {
            let p = PathBuf::from(dest);
            if p.is_dir() {
                fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
            } else {
                fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
            format!("Geri alındı: kopyalanan dosya silindi ({})", dest)
        }
        UndoAction::Rename { old_path, new_path } => {
            fs::rename(new_path, old_path).map_err(|e| e.to_string())?;
            restored.push(old_path.clone());
            format!("Geri alındı: {} → {}", new_path, old_path)
        }
        UndoAction::Create { path } => {
            let p = PathBuf::from(path);
            if p.is_dir() {
                fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
            } else {
                fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
            format!("Geri alındı: oluşturulan silindi ({})", path)
        }
        UndoAction::Delete { path, .. } => {
            match crate::backend::fs_ops::restore_from_trash(path) {
                Ok(restored_path) => {
                    restored.push(restored_path.clone());
                    format!("Silme geri alındı: {} çöp kutusundan geri yüklendi", restored_path)
                }
                Err(e) => {
                    return Err(format!("Silme geri alınamadı: {}", e));
                }
            }
        }
        UndoAction::Replace { dest, backup_path, was_dir: _, is_copy, orig_src } => {
            let d = PathBuf::from(dest);
            let b = PathBuf::from(backup_path);

            // 1. Yeni aktarılan dosyayı kaldır (veya taşıma idiyse eski kaynağına geri taşı)
            if d.exists() {
                if !*is_copy {
                    if let Some(src) = orig_src {
                        let s = PathBuf::from(src);
                        let _ = move_item_inner(&d, &s);
                        restored.push(src.clone());
                    } else if d.is_dir() {
                        let _ = fs::remove_dir_all(&d);
                    } else {
                        let _ = fs::remove_file(&d);
                    }
                } else if d.is_dir() {
                    let _ = fs::remove_dir_all(&d);
                } else {
                    let _ = fs::remove_file(&d);
                }
            }

            // 2. Yedeklenen orijinal dosyayı/klasörü yerine geri koy
            if b.exists() {
                move_item_inner(&b, &d)
                    .map_err(|e| format!("Üzerine yazılan yedek geri yüklenemedi: {}", e))?;
                // Yedek klasörünü temizle (ana dizini boş kaldıysa)
                if let Some(parent) = b.parent() {
                    let _ = fs::remove_dir(parent);
                }
                restored.push(dest.clone());
            }

            format!("Geri alındı: üzerine yazılan eski dosya geri getirildi ({})", dest)
        }
        UndoAction::Batch(actions) => {
            // Eğer batch içindeki tüm elemanlar UndoAction::Delete ise,
            // tek tek sudo/pkexec çalıştırmasın diye topluca restore_from_trash_batch çağır!
            let all_deletes = actions.iter().all(|a| matches!(a, UndoAction::Delete { .. }));
            if all_deletes && !actions.is_empty() {
                let paths: Vec<String> = actions.iter().filter_map(|a| {
                    if let UndoAction::Delete { path, .. } = a {
                        Some(path.clone())
                    } else {
                        None
                    }
                }).collect();
                match crate::backend::fs_ops::restore_from_trash_batch(&paths) {
                    Ok(mut restored_paths) => {
                        let count = restored_paths.len();
                        restored.append(&mut restored_paths);
                        format!("Silme geri alındı: {} öğe çöp kutusundan geri yüklendi", count)
                    }
                    Err(e) => {
                        return Err(format!("Toplu silme geri alınamadı: {}", e));
                    }
                }
            } else {
                let count = actions.len();
                for a in actions.iter().rev() {
                    let _ = undo_single_action(a, restored);
                }
                format!("Geri alındı: {} öğelik toplu işlem geri alındı", count)
            }
        }
    };

    Ok(desc)
}

/// Undo stack'i görüntüle (ne kadar undo var)
pub fn get_undo_stack() -> Vec<String> {
    let stack = UNDO_STACK.lock().unwrap_or_else(|e| e.into_inner());
    stack.iter().map(|a| format!("{:?}", a)).collect()
}

/// Undo stack'i temizle
pub fn clear_undo_stack() -> Result<(), String> {
    let mut stack = UNDO_STACK.lock().map_err(|e| e.to_string())?;
    stack.clear();
    Ok(())
}

// ---- Tracked file operations (undo kayıtlı versiyonlar) ----

/// Taşı — undo kayıtlı
pub fn tracked_move(src: String, dest: String) -> Result<(), String> {
    let s = PathBuf::from(&src);
    let d = PathBuf::from(&dest);
    move_item_inner(&s, &d).map_err(|e| e.to_string())?;
    push_undo(UndoAction::Move {
        from: dest,
        to: src,
    })?;
    Ok(())
}

/// Kopyala — undo kayıtlı
pub fn tracked_copy(src: String, dest: String) -> Result<(), String> {
    crate::backend::fs_ops::copy_item(src, dest.clone())?;
    push_undo(UndoAction::Copy { dest })?;
    Ok(())
}

/// Rename — undo kayıtlı
pub fn tracked_rename(old_path: String, new_path: String) -> Result<(), String> {
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())?;
    push_undo(UndoAction::Rename {
        old_path: old_path.clone(),
        new_path,
    })?;
    Ok(())
}

/// Dosya/klasör oluştur — undo kayıtlı
pub fn tracked_create_file(path: String) -> Result<(), String> {
    fs::File::create(&path).map_err(|e| e.to_string())?;
    push_undo(UndoAction::Create { path })?;
    Ok(())
}

/// Klasör oluştur — undo kayıtlı
pub fn tracked_create_dir(path: String) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    push_undo(UndoAction::Create { path })?;
    Ok(())
}

/// Sil — undo kayıtlı (trash'e gönder)
pub fn tracked_delete(path: String, permanent: bool) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let was_dir = p.is_dir();

    if permanent {
        if was_dir {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        // Kalıcı silme undo edilemez ama stack'e koy ki kullanıcı bilsin
        push_undo(UndoAction::Delete { path, was_dir })?;
    } else {
        trash::delete(&path).map_err(|e| e.to_string())?;
        push_undo(UndoAction::Delete { path, was_dir })?;
    }

    Ok(())
}

// ---- Internal helpers ----

pub fn overwritten_backups_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("overwritten_backups")
}

/// Üzerine yazılacak dosya/klasörü geçici güvenli bir konuma yedekler.
/// Ctrl+Z yapıldığında buradan asıl konumuna geri döndürülür.
pub fn backup_overwritten_item(dest_path: &Path) -> Result<PathBuf, String> {
    if !dest_path.exists() {
        return Err(format!("Yedeklenecek hedef bulunamadı: {:?}", dest_path));
    }

    let root_dir = overwritten_backups_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let rand_id = (std::process::id() as u128) ^ timestamp;
    let backup_sub = root_dir.join(format!("{}_{}", timestamp, rand_id));
    fs::create_dir_all(&backup_sub).map_err(|e| format!("Yedek dizini oluşturulamadı: {}", e))?;

    let file_name = dest_path.file_name()
        .ok_or_else(|| "Geçersiz dosya/klasör adı".to_string())?;
    let backup_target = backup_sub.join(file_name);

    // Hızlı taşıma dene (aynı mount ise anında taşır, değilse copy+delete)
    move_item_inner(dest_path, &backup_target)
        .map_err(|e| format!("Üzerine yazılan dosya yedeklenemedi: {}", e))?;

    Ok(backup_target)
}

fn prune_action_backups(action: &UndoAction) {
    match action {
        UndoAction::Replace { backup_path, .. } => {
            let p = PathBuf::from(backup_path);
            if p.exists() {
                if p.is_dir() {
                    let _ = fs::remove_dir_all(&p);
                } else {
                    let _ = fs::remove_file(&p);
                }
            }
            if let Some(parent) = p.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
        UndoAction::Batch(actions) => {
            for a in actions {
                prune_action_backups(a);
            }
        }
        _ => {}
    }
}

pub(crate) fn push_undo(action: UndoAction) -> Result<(), String> {
    let mut stack = UNDO_STACK.lock().map_err(|e| e.to_string())?;
    if stack.len() >= MAX_UNDO {
        let evicted = stack.remove(0);
        prune_action_backups(&evicted);
    }
    stack.push(action);
    Ok(())
}

pub fn push_undo_batch(actions: Vec<UndoAction>) -> Result<(), String> {
    if actions.is_empty() { return Ok(()); }
    if actions.len() == 1 {
        push_undo(actions.into_iter().next().unwrap())
    } else {
        push_undo(UndoAction::Batch(actions))
    }
}

/// Çakışma varsa "(2)", "(3)"... ekle
pub(crate) fn resolve_conflict(dir: &Path, name: &str) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }

    let stem = Path::new(name).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let ext = Path::new(name).extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for i in 2..=999 {
        let new_name = format!("{} ({}){}", stem, i, ext);
        let candidate = dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    dir.join(format!("{}_copy{}", stem, ext))
}

pub(crate) fn copy_recursive(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let dst_path = dst.join(entry.file_name());
            copy_recursive(&entry.path(), &dst_path)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
    }
    Ok(())
}

pub(crate) fn move_item_inner(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // Aynı filesystem: rename yeterli
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    // Cross-device: kopyala + sil
    copy_recursive(src, dst)?;
    if src.is_dir() {
        fs::remove_dir_all(src)?;
    } else {
        fs::remove_file(src)?;
    }
    Ok(())
}

#[cfg(test)]
pub static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_clipboard_copy_paste_undo() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let test_dir = format!("/tmp/ae_test_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        let src_dir = format!("{}/src", test_dir);
        let dst_dir = format!("{}/dst", test_dir);
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();

        let test_file = format!("{}/hello.txt", src_dir);
        fs::write(&test_file, "hello clipboard").unwrap();

        // 1. Copy to clipboard
        clipboard_copy(vec![test_file.clone()]).unwrap();
        assert!(clipboard_get().is_some());
        assert_eq!(clipboard_get().unwrap().paths.len(), 1);

        // 2. Paste into dst_dir
        let pasted = clipboard_paste(dst_dir.clone()).unwrap();
        assert_eq!(pasted.len(), 1);
        let pasted_file = &pasted[0];
        assert!(PathBuf::from(pasted_file).exists());
        assert_eq!(fs::read_to_string(pasted_file).unwrap(), "hello clipboard");

        // Original still exists because it's a Copy
        assert!(PathBuf::from(&test_file).exists());

        // 3. Undo (Ctrl+Z)
        let undo_res = undo_last();
        assert!(undo_res.is_ok());
        // Pasted file should now be deleted by undo
        assert!(!PathBuf::from(pasted_file).exists());
        // Original still exists
        assert!(PathBuf::from(&test_file).exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_clipboard_cut_paste_undo() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let test_dir = format!("/tmp/ae_test_cut_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        let src_dir = format!("{}/src", test_dir);
        let dst_dir = format!("{}/dst", test_dir);
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();

        let test_file = format!("{}/cut_me.txt", src_dir);
        fs::write(&test_file, "cut content").unwrap();

        // 1. Cut to clipboard
        clipboard_cut(vec![test_file.clone()]).unwrap();
        assert!(clipboard_get().is_some());

        // 2. Paste into dst_dir
        let pasted = clipboard_paste(dst_dir.clone()).unwrap();
        assert_eq!(pasted.len(), 1);
        let pasted_file = &pasted[0];
        assert!(PathBuf::from(pasted_file).exists());
        // Original should be gone after Cut
        assert!(!PathBuf::from(&test_file).exists());

        // 3. Undo (Ctrl+Z)
        let undo_res = undo_last();
        assert!(undo_res.is_ok());
        // Pasted file moved back to original path!
        assert!(PathBuf::from(&test_file).exists());
        assert!(!PathBuf::from(pasted_file).exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_tracked_operations() {
        let test_dir = format!("/tmp/ae_test_tracked_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        fs::create_dir_all(&test_dir).unwrap();

        let f1 = format!("{}/new_file.txt", test_dir);
        tracked_create_file(f1.clone()).unwrap();
        assert!(PathBuf::from(&f1).exists());

        // Undo create
        undo_last().unwrap();
        assert!(!PathBuf::from(&f1).exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_replace_backup_and_undo() {
        let test_dir = format!("/tmp/ae_test_replace_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        let src_dir = format!("{}/src", test_dir);
        let dst_dir = format!("{}/dst", test_dir);
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();

        let src_file = format!("{}/doc.txt", src_dir);
        let dst_file = format!("{}/doc.txt", dst_dir);
        fs::write(&src_file, "NEW VERSION CONTENT").unwrap();
        fs::write(&dst_file, "OLD ORIGINAL CONTENT").unwrap();

        // Backup existing destination file
        let backup_path = backup_overwritten_item(&PathBuf::from(&dst_file)).unwrap();
        assert!(backup_path.exists());
        assert!(!PathBuf::from(&dst_file).exists());

        // Copy new version into place
        fs::copy(&src_file, &dst_file).unwrap();
        assert_eq!(fs::read_to_string(&dst_file).unwrap(), "NEW VERSION CONTENT");

        // Record UndoAction::Replace
        let action = UndoAction::Replace {
            dest: dst_file.clone(),
            backup_path: backup_path.to_string_lossy().to_string(),
            was_dir: false,
            is_copy: true,
            orig_src: None,
        };
        push_undo(action).unwrap();

        // Undo (Ctrl+Z)
        let res = undo_last();
        assert!(res.is_ok());

        // Now dst_file should have "OLD ORIGINAL CONTENT" restored!
        assert!(PathBuf::from(&dst_file).exists());
        assert_eq!(fs::read_to_string(&dst_file).unwrap(), "OLD ORIGINAL CONTENT");
        // And the source file remains intact
        assert_eq!(fs::read_to_string(&src_file).unwrap(), "NEW VERSION CONTENT");

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_undo_returns_restored_paths() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let test_dir = format!("/tmp/ae_test_undo_paths_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        let src_dir = format!("{}/src", test_dir);
        let dst_dir = format!("{}/dst", test_dir);
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();

        let test_file = format!("{}/item.txt", src_dir);
        fs::write(&test_file, "move me").unwrap();

        clipboard_cut(vec![test_file.clone()]).unwrap();
        let pasted = clipboard_paste(dst_dir.clone()).unwrap();
        assert_eq!(pasted.len(), 1);

        let res = undo_last().unwrap();
        assert_eq!(res.restored_paths, vec![test_file]);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_paste_shortcut_handling() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let test_dir = format!("/tmp/ae_test_paste_sc_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        fs::create_dir_all(&test_dir).unwrap();

        let f1 = format!("{}/doc1.txt", test_dir);
        let f2 = format!("{}/doc2.txt", test_dir);
        fs::write(&f1, "doc1 content").unwrap();
        fs::write(&f2, "doc2 content").unwrap();

        // Test 1: parse_uri_list_string with file:// URI scheme
        let uri_data = format!("file://{}\nfile://{}\n", f1, f2);
        let parsed = parse_uri_list_string(&uri_data);
        assert_eq!(parsed, vec![f1.clone(), f2.clone()]);

        // Test 2: parse_uri_list_string with GNOME clipboard prefix
        let gnome_data = format!("copy\nfile://{}\n", f1);
        let parsed_gnome = parse_uri_list_string(&gnome_data);
        assert_eq!(parsed_gnome, vec![f1.clone()]);

        // Test 3: parse_uri_list_string with plain paths & quoted paths
        let plain_quoted = format!("\"{}\"\n'{}'\n", f1, f2);
        let parsed_plain = parse_uri_list_string(&plain_quoted);
        assert_eq!(parsed_plain, vec![f1.clone(), f2.clone()]);

        // Test 4: clipboard_copy and clipboard_get integration
        clipboard_copy(vec![f1.clone()]).unwrap();
        let cb = clipboard_get();
        assert!(cb.is_some());
        let cb = cb.unwrap();
        assert_eq!(cb.op, ClipboardOp::Copy);
        assert_eq!(cb.paths, vec![f1.clone()]);

        // Test 5: file://localhost/ URI scheme and percent decoding
        let local_uri = format!("file://localhost{}\n", f1.replace(' ', "%20"));
        let parsed_local = parse_uri_list_string(&local_uri);
        assert_eq!(parsed_local, vec![f1.clone()]);

        // Test 6: clipboard_get_fresh returns updated state
        let fresh_cb = clipboard_get_fresh();
        assert!(fresh_cb.is_some());
        assert_eq!(fresh_cb.unwrap().paths, vec![f1]);

        // Test 7: parse_uri_list_with_op with cut prefix
        let cut_data = format!("cut\nfile://{}\n", f2);
        let (cut_paths, cut_op) = parse_uri_list_with_op(&cut_data);
        assert_eq!(cut_paths, vec![f2.clone()]);
        assert_eq!(cut_op, ClipboardOp::Cut);

        // Test 8: clipboard_clear clears both internal state and cache
        clipboard_clear().unwrap();
        assert!(clipboard_get().is_none());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_percent_decoding_and_turkish_chars() {
        let test_dir = format!("/tmp/ae_test_turkish_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        fs::create_dir_all(&test_dir).unwrap();

        let turkish_file = format!("{}/türkçe çalışma dosyası.txt", test_dir);
        fs::write(&turkish_file, "turkish chars test").unwrap();

        // Encode as file URI with %20 and UTF-8 hex
        let encoded_path = turkish_file
            .replace(' ', "%20")
            .replace('ü', "%C3%BC")
            .replace('k', "k")
            .replace('ç', "%C3%A7")
            .replace('e', "e")
            .replace('ı', "%C4%B1")
            .replace('ş', "%C5%9F");
        let uri = format!("file://{}", encoded_path);

        let parsed = parse_uri_list_string(&uri);
        assert_eq!(parsed, vec![turkish_file.clone()]);

        let _ = fs::remove_dir_all(&test_dir);
    }
}

