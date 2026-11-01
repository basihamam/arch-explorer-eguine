use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::{DateTime, Local};
use walkdir::WalkDir;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: String,
    pub created: String,
    pub permissions: String,
    pub mime_type: String,
    pub extension: String,
    pub hidden: bool,
    #[serde(default)]
    pub is_executable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub parent: Option<String>,
    pub total_items: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
pub struct DriveInfo {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub fs_type: String,
    pub device: String,
}

/// Gelişmiş bookmark — icon, pinned, sıralama vs.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub icon: String,          // "home", "downloads", "folder", "star" vs.
    #[serde(default)]
    pub pinned: bool,          // Sidebar'ın üstünde sabit dursun mu
    #[serde(default)]
    pub order: i32,            // Manuel sıralama
    #[serde(default)]
    pub color: String,         // Opsiyonel renk etiketi (#ff6b6b gibi)
}

/// Drag-drop taşıma isteği
#[derive(Debug, Serialize, Deserialize)]
pub struct DragDropRequest {
    pub sources: Vec<String>,   // Sürüklenen dosyaların path'leri
    pub dest_dir: String,       // Bırakılan dizin
    pub copy: bool,             // true = Ctrl basılıysa kopyala, false = taşı
}

/// Drag-drop sonucu
#[derive(Debug, Serialize, Deserialize)]
pub struct DragDropResult {
    pub moved: Vec<String>,
    pub failed: Vec<String>,
    pub errors: Vec<String>,
}

pub(crate) fn format_permissions(meta: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode();
    let mut perm = String::with_capacity(10);

    if meta.is_dir() { perm.push('d'); }
    else if meta.file_type().is_symlink() { perm.push('l'); }
    else { perm.push('-'); }

    for shift in [6u32, 3, 0] {
        perm.push(if mode & (1 << (shift + 2)) != 0 { 'r' } else { '-' });
        perm.push(if mode & (1 << (shift + 1)) != 0 { 'w' } else { '-' });
        perm.push(if mode & (1 << (shift + 0)) != 0 { 'x' } else { '-' });
    }
    perm
}

fn build_entry(path: &Path) -> Option<FileEntry> {
    let meta = fs::symlink_metadata(path).ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let is_symlink = meta.file_type().is_symlink();
    let real_meta = fs::metadata(path).unwrap_or(meta.clone());

    let modified = real_meta.modified().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();

    let created = real_meta.created().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();

    let extension = path.extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let mime_type = if real_meta.is_dir() {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string()
    };

    let is_executable = if real_meta.is_dir() {
        false
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            real_meta.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            matches!(extension.to_lowercase().as_str(), "exe" | "bat" | "cmd" | "sh")
        }
    };

    Some(FileEntry {
        name: name.clone(),
        path: path.to_string_lossy().to_string(),
        is_dir: real_meta.is_dir(),
        is_symlink,
        size: if real_meta.is_dir() { 0 } else { real_meta.len() },
        modified,
        created,
        permissions: format_permissions(&real_meta),
        mime_type,
        extension,
        hidden: name.starts_with('.'),
        is_executable,
    })
}

fn build_entry_from_direntry(de: &fs::DirEntry) -> Option<FileEntry> {
    let name = de.file_name().to_string_lossy().to_string();
    let path = de.path();
    let is_symlink = de.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);

    // If it's a symlink, de.metadata() might return the symlink itself on some platforms or follow it.
    // fs::metadata follows symlinks to the target. If target doesn't exist, fallback to symlink_metadata.
    let (meta, is_dir) = if is_symlink {
        match fs::metadata(&path) {
            Ok(target_meta) => {
                let target_is_dir = target_meta.is_dir();
                (target_meta, target_is_dir)
            }
            Err(_) => {
                let sm = fs::symlink_metadata(&path).ok().or_else(|| de.metadata().ok())?;
                (sm, false)
            }
        }
    } else {
        let m = de.metadata().ok()?;
        let d = m.is_dir();
        (m, d)
    };

    let modified = meta.modified().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();

    let created = meta.created().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();

    let extension = if is_dir {
        String::new()
    } else {
        path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default()
    };

    let mime_type = if is_dir {
        "inode/directory".to_string()
    } else {
        // Fast extension check for standard types avoids slow full mime DB in massive folders
        match extension.to_lowercase().as_str() {
            "png" => "image/png".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "gif" => "image/gif".to_string(),
            "webp" => "image/webp".to_string(),
            "svg" => "image/svg+xml".to_string(),
            "mp4" => "video/mp4".to_string(),
            "mkv" => "video/x-matroska".to_string(),
            "mp3" => "audio/mpeg".to_string(),
            "txt" => "text/plain".to_string(),
            "rs"  => "text/rust".to_string(),
            "json" => "application/json".to_string(),
            "toml" => "application/toml".to_string(),
            _ => mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string()
        }
    };

    let is_executable = if is_dir {
        false
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            matches!(extension.to_lowercase().as_str(), "exe" | "bat" | "cmd" | "sh")
        }
    };

    Some(FileEntry {
        name: name.clone(),
        path: path.to_string_lossy().to_string(),
        is_dir,
        is_symlink,
        size: if is_dir { 0 } else { meta.len() },
        modified,
        created,
        permissions: format_permissions(&meta),
        mime_type,
        extension,
        hidden: name.starts_with('.'),
        is_executable,
    })
}

pub fn resolve_trash_dir() -> PathBuf {
    let trash = dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".local/share"))
        .join("Trash")
        .join("files");
    let _ = fs::create_dir_all(&trash);
    if let Some(parent) = trash.parent() {
        let _ = fs::create_dir_all(parent.join("info"));
    }
    trash
}

pub fn list_directory(path: String, show_hidden: bool) -> Result<DirectoryListing, String> {
    let dir_path = if path == "trash:///" || path == "trash:" || path == "trash://" {
        resolve_trash_dir()
    } else {
        PathBuf::from(&path)
    };

    if !dir_path.exists() {
        return Err(format!("Path bulunamadı: {}", path));
    }
    if !dir_path.is_dir() {
        return Err(format!("Dizin değil: {}", path));
    }

    // Eğer bu bir sembolik bağ (symlink) ise, gerçek orijinal konumuna çözümle (canonicalize)
    // Böylece ~/cikti gibi symlink üzerinden girilince de orijinal konumdan girilmiş gibi tüm dosyalar eksiksiz görünür!
    let (dir_path, resolved_path) = match fs::canonicalize(&dir_path) {
        Ok(canon) => {
            let s = canon.to_string_lossy().to_string();
            (canon, s)
        }
        Err(_) => (dir_path.clone(), path.clone()),
    };

    let read_dir = fs::read_dir(&dir_path)
        .map_err(|e| format!("Dizin okunamadı: {}", e))?;

    let mut entries: Vec<FileEntry> = read_dir
        .filter_map(|e| e.ok())
        .filter_map(|e| build_entry_from_direntry(&e))
        .filter(|e| show_hidden || !e.hidden)
        .collect();

    // Klasörler önce, sonra alfabetik (30.000 dosya için optimize edilmiş hızlı sort_unstable_by)
    entries.sort_unstable_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    let total = entries.len();
    let parent = if path == "trash:///" || path == "trash:" {
        dirs::home_dir().map(|p| p.to_string_lossy().to_string())
    } else {
        dir_path.parent().map(|p| p.to_string_lossy().to_string())
    };

    Ok(DirectoryListing {
        path: resolved_path,
        entries,
        parent,
        total_items: total,
    })
}

// Global cached root/sudo password (kullanıcı oturumu boyunca hatırlanır, alt klasörlerde tekrar şifre sormaz)
static ROOT_PASSWORD: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_root_password(pw: String) {
    if let Ok(mut l) = ROOT_PASSWORD.lock() {
        *l = Some(pw);
    }
}

pub fn get_root_password() -> Option<String> {
    ROOT_PASSWORD.lock().ok()?.clone()
}

pub fn clear_root_password() {
    if let Ok(mut l) = ROOT_PASSWORD.lock() {
        *l = None;
    }
}

/// Root şifresi ile dosyayı oku (sudo -S cat ile)
pub fn read_file_bytes_elevated(path: &str) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let pw = get_root_password().ok_or_else(|| "Root password not cached".to_string())?;

    let mut child = Command::new("sudo")
        .args(["-S", "cat", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("sudo spawn hatası: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{}\n", pw).as_bytes());
    }

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        Err(stderr)
    }
}

/// Sudo şifresi varsa herhangi bir komutu root olarak çalıştır (örn: cp, mv, rm, mkdir)
pub fn sudo_run_elevated(cmd: &str, args: &[&str]) -> Result<(), String> {
    let pw = get_root_password().ok_or_else(|| "Root password not cached".to_string())?;

    // Safe batching: Yüz binlerce dosya silinirken Linux E2BIG ("Argument list too long") hatasını
    // engellemek için dosyaları 500'lük güvenli parçalara bölerek siler!
    if cmd == "rm" && args.len() > 500 {
        let flag = if args.first().map_or(false, |a| a.starts_with('-')) {
            Some(args[0])
        } else {
            None
        };
        let files = if flag.is_some() { &args[1..] } else { args };
        for chunk in files.chunks(500) {
            let mut sub_args: Vec<&str> = Vec::with_capacity(chunk.len() + 1);
            if let Some(f) = flag { sub_args.push(f); }
            sub_args.extend_from_slice(chunk);
            sudo_run_elevated_single(cmd, &sub_args, &pw)?;
        }
        return Ok(());
    }

    sudo_run_elevated_single(cmd, args, &pw)
}

fn sudo_run_elevated_single(cmd: &str, args: &[&str], pw: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut full_args = vec!["-S", cmd];
    full_args.extend_from_slice(args);

    let mut child = Command::new("sudo")
        .args(&full_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("sudo spawn hatası: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{}\n", pw).as_bytes());
    }

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        Err(stderr)
    }
}

/// Root yetkisiyle dizin listele (sudo python3 ile)
pub fn list_directory_with_sudo(path: String, show_hidden: bool, password: &str) -> Result<DirectoryListing, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let py_script = r#"
import os, json, sys, stat, datetime

raw_path = sys.argv[1]
target_path = os.path.realpath(raw_path) if os.path.exists(raw_path) else raw_path
show_hidden = sys.argv[2] == "1"
entries = []
try:
    with os.scandir(target_path) as it:
        for entry in it:
            if not show_hidden and entry.name.startswith('.'):
                continue
            try:
                st = entry.stat(follow_symlinks=False)
                is_dir = stat.S_ISDIR(st.st_mode)
                is_symlink = stat.S_ISLNK(st.st_mode)
                size = 0 if is_dir else st.st_size
                mtime = datetime.datetime.fromtimestamp(st.st_mtime).strftime('%Y-%m-%d %H:%M')
                created = datetime.datetime.fromtimestamp(st.st_ctime).strftime('%Y-%m-%d %H:%M')
                entries.append({
                    'name': entry.name,
                    'path': entry.path,
                    'is_dir': is_dir,
                    'is_symlink': is_symlink,
                    'size': size,
                    'modified': mtime,
                    'created': created,
                    'permissions': stat.filemode(st.st_mode),
                    'mime_type': 'inode/directory' if is_dir else 'application/octet-stream',
                    'extension': os.path.splitext(entry.name)[1].lstrip('.'),
                    'hidden': entry.name.startswith('.'),
                    'is_executable': bool(st.st_mode & 0o111) and not is_dir
                })
            except Exception:
                continue
    entries.sort(key=lambda x: (not x['is_dir'], x['name'].lower()))
    parent = os.path.dirname(target_path.rstrip('/'))
    if not parent: parent = '/'
    print(json.dumps({'status': 'ok', 'listing': {'path': target_path, 'entries': entries, 'parent': parent, 'total_items': len(entries)}}))
except Exception as e:
    print(json.dumps({'status': 'error', 'message': str(e)}))
"#;

    let mut child = Command::new("sudo")
        .args(["-S", "python3", "-c", py_script, &path, if show_hidden { "1" } else { "0" }])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("sudo spawn hatası: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{}\n", password).as_bytes());
    }

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    if !out.status.success() {
        let lower = stderr.to_lowercase();
        if lower.contains("incorrect password") || lower.contains("try again") || lower.contains("yanlış") {
            return Err("WRONG_PASSWORD".to_string());
        }
        return Err(format!("Sudo yetkilendirme hatası: {}", stderr.trim()));
    }

    let stdout_str = String::from_utf8_lossy(&out.stdout).to_string();
    #[derive(serde::Deserialize)]
    struct Resp {
        status: String,
        listing: Option<DirectoryListing>,
        message: Option<String>,
    }

    let resp: Resp = serde_json::from_str(&stdout_str)
        .map_err(|e| format!("Root dizin çıktısı çözümlenemedi (JSON parse hatası: {}): {}", e, stdout_str))?;

    if resp.status == "ok" {
        if let Some(l) = resp.listing {
            return Ok(l);
        }
    }

    Err(resp.message.unwrap_or_else(|| "Bilinmeyen yetki hatası".to_string()))
}

pub fn get_file_info(path: String) -> Result<FileEntry, String> {
    let p = PathBuf::from(&path);
    build_entry(&p).ok_or_else(|| format!("Stat edilemedi: {}", path))
}

pub fn create_dir(path: String) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

pub fn create_file(path: String) -> Result<(), String> {
    fs::File::create(&path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_item(path: String, permanent: bool) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let trash_files = resolve_trash_dir();
    let is_in_trash = path.starts_with("trash://")
        || path.starts_with("trash:")
        || p.starts_with(&trash_files)
        || path.contains("/.local/share/Trash");

    if permanent || is_in_trash {
        // Remove item permanently
        if p.is_dir() {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }

        // Clean up corresponding .trashinfo in Trash/info/ if deleting from Trash
        if let Some(file_name) = p.file_name() {
            if let Some(trash_parent) = trash_files.parent() {
                let info_file = trash_parent.join("info").join(format!("{}.trashinfo", file_name.to_string_lossy()));
                if info_file.exists() {
                    let _ = fs::remove_file(info_file);
                }
            }
        }
        Ok(())
    } else {
        let _ = resolve_trash_dir();
        // 1. Standart trash kütüphanesini dene
        if let Ok(()) = trash::delete(&path) {
            return Ok(());
        }

        // 2. gio trash fallback
        if let Ok(status) = Command::new("gio").args(["trash", &path]).status() {
            if status.success() {
                return Ok(());
            }
        }

        // 3. Manuel freedesktop Trash spesifikasyonuna uygun taşıma + .trashinfo oluşturma
        let file_name = p.file_name().ok_or_else(|| "Geçersiz dosya adı".to_string())?;
        let base_name = file_name.to_string_lossy().to_string();
        let target_in_trash = trash_files.join(&base_name);

        if let Some(trash_parent) = trash_files.parent() {
            let trash_info_dir = trash_parent.join("info");
            let _ = fs::create_dir_all(&trash_info_dir);
            let info_file = trash_info_dir.join(format!("{}.trashinfo", base_name));

            // .trashinfo oluştur
            let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
            let trash_info_content = format!("[Trash Info]\nPath={}\nDeletionDate={}\n", path, now);
            fs::write(&info_file, trash_info_content).map_err(|e| e.to_string())?;

            // Dosyayı trash/files altına taşı
            if let Err(e) = crate::backend::clipboard::move_item_inner(&p, &target_in_trash) {
                let _ = fs::remove_file(info_file);
                return Err(format!("Çöp kutusuna taşınamadı: {}", e));
            }
            Ok(())
        } else {
            Err("Çöp kutusu dizini bulunamadı".to_string())
        }
    }
}

fn percent_decode(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = s.as_bytes().iter().copied();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(c1), Some(c2)) = (h1, h2) {
                if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&[c1, c2]).unwrap_or(""), 16) {
                    bytes.push(val);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn restore_from_trash(target: &str) -> Result<String, String> {
    let trash_files = resolve_trash_dir();
    let trash_parent = trash_files.parent().ok_or_else(|| "Trash parent bulunamadı".to_string())?;
    let trash_info_dir = trash_parent.join("info");

    let p = Path::new(target);
    // Case 1: Target is an item inside trash (e.g. ~/.local/share/Trash/files/abc or trash:///abc)
    if target.starts_with("trash:") || p.starts_with(&trash_files) {
        let file_name = p.file_name().ok_or_else(|| "Geçersiz dosya adı".to_string())?;
        let info_file = trash_info_dir.join(format!("{}.trashinfo", file_name.to_string_lossy()));
        if !info_file.exists() {
            return Err("Dosyanın .trashinfo meta verisi bulunamadı".to_string());
        }
        let content = fs::read_to_string(&info_file).map_err(|e| e.to_string())?;
        for line in content.lines() {
            if let Some(raw_path) = line.strip_prefix("Path=") {
                let orig_path = percent_decode(raw_path.trim());
                let orig_p = Path::new(&orig_path);
                if let Some(parent) = orig_p.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let trash_file = trash_files.join(file_name);
                crate::backend::clipboard::move_item_inner(&trash_file, &orig_p).map_err(|e| format!("Geri yükleme taşınamadı: {}", e))?;
                let _ = fs::remove_file(info_file);
                return Ok(orig_path);
            }
        }
        return Err("Geçerli Path satırı bulunamadı".to_string());
    }

    // Case 2: Target is the original path that was deleted (e.g. from Undo)
    if trash_info_dir.exists() {
        if let Ok(entries) = fs::read_dir(&trash_info_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("trashinfo") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        for line in content.lines() {
                            if let Some(raw_path) = line.strip_prefix("Path=") {
                                let orig_path = percent_decode(raw_path.trim());
                                if orig_path == target {
                                    let stem = path.file_stem().unwrap_or_default();
                                    let trash_file = trash_files.join(stem);
                                    if trash_file.exists() {
                                        let orig_p = Path::new(&orig_path);
                                        if let Some(parent) = orig_p.parent() {
                                            let _ = fs::create_dir_all(parent);
                                        }
                                        crate::backend::clipboard::move_item_inner(&trash_file, orig_p).map_err(|e| format!("Geri yükleme taşınamadı: {}", e))?;
                                        let _ = fs::remove_file(&path);
                                        return Ok(orig_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err(format!("Çöp kutusunda eşleşen kayıt bulunamadı: {}", target))
}

pub fn empty_trash() -> Result<(), String> {
    let trash_files = resolve_trash_dir();
    if let Some(trash_parent) = trash_files.parent() {
        let trash_info_dir = trash_parent.join("info");
        if trash_files.exists() {
            if let Ok(entries) = fs::read_dir(&trash_files) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let _ = fs::remove_dir_all(&p);
                    } else {
                        let _ = fs::remove_file(&p);
                    }
                }
            }
        }
        if trash_info_dir.exists() {
            if let Ok(entries) = fs::read_dir(&trash_info_dir) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    } else {
        Err("Trash dizini bulunamadı".to_string())
    }
}

pub fn rename_item(old_path: String, new_path: String) -> Result<(), String> {
    if let Err(e) = fs::rename(&old_path, &new_path) {
        if get_root_password().is_some() {
            sudo_run_elevated("mv", &[&old_path, &new_path]).map_err(|e2| format!("{}: {}", e, e2))
        } else {
            Err(e.to_string())
        }
    } else {
        Ok(())
    }
}

pub fn copy_item(src: String, dst: String) -> Result<(), String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst);

    // Canonicalize or normalize to prevent self-copy
    let src_canon = std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
    let dst_canon = std::fs::canonicalize(&dst_path).unwrap_or_else(|_| dst_path.clone());

    if src_canon == dst_canon {
        return Ok(()); // Dropped onto itself, no-op
    }

    if src_path.is_dir() {
        if dst_canon.starts_with(&src_canon) {
            return Err("Cannot copy a directory into itself or a subdirectory".into());
        }
        copy_dir_recursive(&src_path, &dst_path)
            .map_err(|e| e.to_string())
    } else {
        fs::copy(&src, &dst).map(|_| ()).map_err(|e| e.to_string())
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    let src_canon = std::fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let dst_canon = std::fs::canonicalize(dst).unwrap_or_else(|_| dst.to_path_buf());
    if src_canon == dst_canon || dst_canon.starts_with(&src_canon) {
        return Ok(()); // Guard against recursive loops
    }

    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

pub fn move_item(src: String, dst: String) -> Result<(), String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst);

    let src_canon = std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
    let dst_canon = std::fs::canonicalize(&dst_path).unwrap_or_else(|_| dst_path.clone());

    if src_canon == dst_canon {
        return Ok(()); // Dropped onto itself, no-op
    }

    if src_path.is_dir() && dst_canon.starts_with(&src_canon) {
        return Err("Cannot move a directory into itself or a subdirectory".into());
    }

    fs::rename(&src, &dst).or_else(|_| {
        copy_item(src.clone(), dst.clone())?;
        let p = PathBuf::from(&src);
        if p.is_dir() {
            fs::remove_dir_all(&src).map_err(|e| e.to_string())
        } else {
            fs::remove_file(&src).map_err(|e| e.to_string())
        }
    })
}

/// Drag-drop işlemi — birden fazla dosyayı hedef dizine taşı/kopyala
/// Frontend'den sürükle-bırak eventi gelince bunu çağır
pub fn drag_drop_files(req: DragDropRequest) -> Result<DragDropResult, String> {
    let dest = PathBuf::from(&req.dest_dir);

    if !dest.is_dir() {
        return Err(format!("Hedef dizin bulunamadı: {}", req.dest_dir));
    }

    let mut moved = vec![];
    let mut failed = vec![];
    let mut errors = vec![];

    for src_path in &req.sources {
        let src = PathBuf::from(src_path);

        if !src.exists() {
            failed.push(src_path.clone());
            errors.push(format!("Kaynak bulunamadı: {}", src_path));
            continue;
        }

        // Aynı dizine bırakma — anlamsız, atla
        if src.parent().map(|p| p == dest).unwrap_or(false) && !req.copy {
            continue;
        }

        let file_name = match src.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => {
                failed.push(src_path.clone());
                errors.push(format!("Geçersiz dosya adı: {}", src_path));
                continue;
            }
        };

        // Çakışma varsa yeni isim üret
        let dest_path = resolve_name_conflict(&dest, &file_name);

        let result = if req.copy {
            copy_dir_recursive(&src, &dest_path)
                .or_else(|_| fs::copy(&src, &dest_path).map(|_| ()))
                .map_err(|e| e.to_string())
        } else {
            // Cross-device safe taşıma — or_else içinde tek tip String error
            (|| -> Result<(), String> {
                if fs::rename(&src, &dest_path).is_ok() {
                    return Ok(());
                }
                // Farklı filesystem: kopyala + sil
                copy_dir_recursive(&src, &dest_path)
                    .or_else(|_| fs::copy(&src, &dest_path).map(|_| ()))
                    .map_err(|e| e.to_string())?;
                if src.is_dir() {
                    fs::remove_dir_all(&src)
                } else {
                    fs::remove_file(&src)
                }
                .map_err(|e| e.to_string())
            })()
        };

        match result {
            Ok(_) => moved.push(dest_path.to_string_lossy().to_string()),
            Err(e) => {
                failed.push(src_path.clone());
                errors.push(format!("{}: {}", src_path, e));
            }
        }
    }

    Ok(DragDropResult { moved, failed, errors })
}

/// Sidebar bookmark'larına dizin sürükle-bırak ile ekle
pub fn drop_to_bookmark_sidebar(path: String) -> Result<Bookmark, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Sadece klasörler bookmark olabilir: {}", path));
    }

    let name = p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    let bm = Bookmark {
        name: name.clone(),
        path: path.clone(),
        icon: "folder".to_string(),
        pinned: false,
        order: get_next_bookmark_order(),
        color: String::new(),
    };

    add_bookmark_full(bm.clone())?;
    Ok(bm)
}

fn get_next_bookmark_order() -> i32 {
    let bms = get_bookmarks();
    bms.iter().map(|b| b.order).max().unwrap_or(0) + 1
}

/// Dosya yolu var mı kontrol et (frontend için utility)
pub fn path_exists(path: String) -> bool {
    PathBuf::from(&path).exists()
}

/// Bir path'in dizin mi dosya mı olduğunu döndür
pub fn path_is_dir(path: String) -> bool {
    PathBuf::from(&path).is_dir()
}

pub fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn get_drives() -> Vec<DriveInfo> {
    let output = Command::new("df")
        .args(["-h", "--output=source,target,size,used,avail,fstype"])
        .output();

    let Ok(out) = output else { return vec![]; };
    let text = String::from_utf8_lossy(&out.stdout);

    text.lines()
        .skip(1)
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 { return None; }
            let device = parts[0];
            let mount  = parts[1];

            if !device.starts_with("/dev/") { return None; }

            Some(DriveInfo {
                device: device.to_string(),
                mount_point: mount.to_string(),
                total: parse_human_size(parts[2]),
                used: parse_human_size(parts[3]),
                available: parse_human_size(parts[4]),
                fs_type: parts[5].to_string(),
                name: mount.split('/').last().unwrap_or(device).to_string(),
            })
        })
        .collect()
}

fn parse_human_size(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() || s == "-" { return 0; }
    let (num, suffix) = s.split_at(s.len().saturating_sub(1));
    let base: f64 = num.parse().unwrap_or(0.0);
    match suffix {
        "G" => (base * 1_073_741_824.0) as u64,
        "M" => (base * 1_048_576.0) as u64,
        "K" => (base * 1_024.0) as u64,
        "T" => (base * 1_099_511_627_776.0) as u64,
        _ => base as u64,
    }
}

pub fn search_files(
    root: String,
    query: String,
    max_results: Option<usize>,
) -> Vec<FileEntry> {
    let limit = max_results.unwrap_or(200);
    let q = query.to_lowercase();

    WalkDir::new(&root)
        .max_depth(8)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .contains(&q)
        })
        .take(limit)
        .filter_map(|e| build_entry(e.path()))
        .collect()
}

// ---- Bookmark CRUD ----

fn bookmarks_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("bookmarks.json")
}

pub fn get_bookmarks() -> Vec<Bookmark> {
    let path = bookmarks_path();
    if !path.exists() {
        return default_bookmarks();
    }
    let data = fs::read_to_string(&path).unwrap_or_default();
    let mut bms: Vec<Bookmark> = serde_json::from_str(&data)
        .unwrap_or_else(|_| default_bookmarks());

    // Pinned önce, sonra order'a göre sırala
    bms.sort_by(|a, b| {
        match (a.pinned, b.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.order.cmp(&b.order),
        }
    });

    bms
}

fn default_bookmarks() -> Vec<Bookmark> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
    vec![
        Bookmark {
            name: "Home".into(),
            path: home.to_string_lossy().to_string(),
            icon: "home".into(),
            pinned: true,
            order: 0,
            color: String::new(),
        },
        Bookmark {
            name: "Desktop".into(),
            path: home.join("Desktop").to_string_lossy().to_string(),
            icon: "desktop".into(),
            pinned: false,
            order: 1,
            color: String::new(),
        },
        Bookmark {
            name: "Downloads".into(),
            path: home.join("Downloads").to_string_lossy().to_string(),
            icon: "download".into(),
            pinned: false,
            order: 2,
            color: String::new(),
        },
        Bookmark {
            name: "Documents".into(),
            path: home.join("Documents").to_string_lossy().to_string(),
            icon: "document".into(),
            pinned: false,
            order: 3,
            color: String::new(),
        },
        Bookmark {
            name: "Pictures".into(),
            path: home.join("Pictures").to_string_lossy().to_string(),
            icon: "image".into(),
            pinned: false,
            order: 4,
            color: String::new(),
        },
        Bookmark {
            name: "Music".into(),
            path: home.join("Music").to_string_lossy().to_string(),
            icon: "music".into(),
            pinned: false,
            order: 5,
            color: String::new(),
        },
        Bookmark {
            name: "Videos".into(),
            path: home.join("Videos").to_string_lossy().to_string(),
            icon: "video".into(),
            pinned: false,
            order: 6,
            color: String::new(),
        },
        Bookmark {
            name: "Root".into(),
            path: "/".into(),
            icon: "server".into(),
            pinned: false,
            order: 7,
            color: String::new(),
        },
        Bookmark {
            name: "Media".into(),
            path: "/run/media".into(),
            icon: "hdd".into(),
            pinned: false,
            order: 8,
            color: String::new(),
        },
    ]
}

/// Basit bookmark ekleme (eski API uyumu)
pub fn add_bookmark(name: String, path: String) -> Result<(), String> {
    let order = get_next_bookmark_order();
    let bm = Bookmark {
        name,
        path,
        icon: "folder".to_string(),
        pinned: false,
        order,
        color: String::new(),
    };
    add_bookmark_full(bm)
}

/// Tam bookmark ekleme (icon, pinned, color ile)
pub fn add_bookmark_full(bm: Bookmark) -> Result<(), String> {
    let mut bms = get_bookmarks();
    // Aynı path zaten varsa güncelle
    if let Some(existing) = bms.iter_mut().find(|b| b.path == bm.path) {
        *existing = bm;
    } else {
        bms.push(bm);
    }
    save_bookmarks(&bms)
}

/// Bookmark'ı güncelle (rename, renk, pin vs.)
pub fn update_bookmark(path: String, name: Option<String>, icon: Option<String>, pinned: Option<bool>, color: Option<String>) -> Result<(), String> {
    let mut bms = get_bookmarks();
    let bm = bms.iter_mut()
        .find(|b| b.path == path)
        .ok_or_else(|| format!("Bookmark bulunamadı: {}", path))?;

    if let Some(n) = name   { bm.name   = n; }
    if let Some(i) = icon   { bm.icon   = i; }
    if let Some(p) = pinned { bm.pinned = p; }
    if let Some(c) = color  { bm.color  = c; }

    save_bookmarks(&bms)
}

/// Bookmark sırasını yeniden düzenle (drag-drop ile)
pub fn reorder_bookmarks(ordered_paths: Vec<String>) -> Result<(), String> {
    let mut bms = get_bookmarks();
    for (i, path) in ordered_paths.iter().enumerate() {
        if let Some(bm) = bms.iter_mut().find(|b| &b.path == path) {
            bm.order = i as i32;
        }
    }
    save_bookmarks(&bms)
}

pub fn remove_bookmark(path: String) -> Result<(), String> {
    let bm: Vec<Bookmark> = get_bookmarks()
        .into_iter()
        .filter(|b| b.path != path)
        .collect();
    save_bookmarks(&bm)
}

fn save_bookmarks(bm: &[Bookmark]) -> Result<(), String> {
    let path = bookmarks_path();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let data = serde_json::to_string_pretty(bm).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

// ---- Internal helpers ----

fn resolve_name_conflict(dir: &Path, name: &str) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() { return dest; }

    let stem = Path::new(name).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let ext = Path::new(name).extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for i in 2..=999 {
        let candidate = dir.join(format!("{} ({}){}", stem, i, ext));
        if !candidate.exists() { return candidate; }
    }

    dir.join(format!("{}_copy{}", stem, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_item_in_trash_is_permanent() {
        let trash_dir = resolve_trash_dir();
        let test_file = trash_dir.join("test_trash_permanent_del.txt");
        let _ = fs::write(&test_file, "temporary content");
        assert!(test_file.exists());

        // Create dummy trashinfo
        if let Some(parent) = trash_dir.parent() {
            let info_file = parent.join("info").join("test_trash_permanent_del.txt.trashinfo");
            let _ = fs::write(&info_file, "[Trash Info]\nPath=/dummy\n");
            assert!(info_file.exists());

            // Delete item from trash
            let res = delete_item(test_file.to_string_lossy().to_string(), false);
            assert!(res.is_ok());
            assert!(!test_file.exists());
            assert!(!info_file.exists());
        }
    }

    #[test]
    fn test_restore_from_trash() {
        let trash_dir = resolve_trash_dir();
        let orig_file = "/tmp/test_orig_restore_file.txt";
        let _ = fs::remove_file(orig_file);

        let trash_file = trash_dir.join("test_orig_restore_file.txt");
        let _ = fs::write(&trash_file, "restored content");
        assert!(trash_file.exists());

        if let Some(parent) = trash_dir.parent() {
            let info_file = parent.join("info").join("test_orig_restore_file.txt.trashinfo");
            let _ = fs::write(&info_file, format!("[Trash Info]\nPath={}\n", orig_file));
            assert!(info_file.exists());

            // 1. Restore by trash file path
            let res = restore_from_trash(&trash_file.to_string_lossy());
            assert!(res.is_ok(), "Restore failed with error: {:?}", res.as_ref().err());
            assert_eq!(res.unwrap(), orig_file);
            assert!(PathBuf::from(orig_file).exists());
            assert!(!trash_file.exists());
            assert!(!info_file.exists());

            let _ = fs::remove_file(orig_file);
        }
    }
}

