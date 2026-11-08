// open_with.rs — mime detection + .desktop parser + app launcher
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWithInfo {
    pub mime_type: String,
    pub default_app: Option<String>,
    pub apps: Vec<AppEntry>,
}

fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.insert(0, home.join(".local/share/applications"));
    }
    dirs
}

fn parse_desktop_file(path: &Path) -> Option<(AppEntry, Vec<String>)> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon: Option<String> = None;
    let mut comment: Option<String> = None;
    let mut categories: Vec<String> = vec![];
    let mut mime_types: Vec<String> = vec![];
    let mut no_display = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" { in_entry = true; continue; }
        if line.starts_with('[') && line != "[Desktop Entry]" { if in_entry { break; } continue; }
        if !in_entry || line.starts_with('#') || line.is_empty() { continue; }
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "Name"      => { if name.is_empty() { name = v.trim().into(); } }
                "Exec"      => { if exec.is_empty() { exec = v.trim().into(); } }
                "Icon"      => { icon = Some(v.trim().into()); }
                "Comment"   => { comment = Some(v.trim().into()); }
                "Categories"=> { categories = v.trim().split(';').filter(|s| !s.is_empty()).map(String::from).collect(); }
                "MimeType"  => { mime_types = v.trim().split(';').filter(|s| !s.is_empty()).map(String::from).collect(); }
                "NoDisplay" => {
                    let val = v.trim().eq_ignore_ascii_case("true");
                    // wine.desktop NoDisplay=true olsa bile dosya açıcı olarak geçerlidir!
                    let is_wine = path.file_name().and_then(|n| n.to_str()) == Some("wine.desktop");
                    if !is_wine {
                        no_display = val;
                    }
                }
                "Type"      => { if v.trim() != "Application" { return None; } }
                _ => {}
            }
        }
    }
    if name.is_empty() || exec.is_empty() || no_display { return None; }
    let id = path.file_name()?.to_string_lossy().to_string();
    Some((AppEntry { id, name, exec, icon, comment, categories }, mime_types))
}

fn build_mime_index() -> HashMap<String, Vec<AppEntry>> {
    let mut index: HashMap<String, Vec<AppEntry>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for dir in app_dirs() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") { continue; }
            let Some((app, mimes)) = parse_desktop_file(&path) else { continue };
            if seen.contains(&app.id) { continue; }
            seen.insert(app.id.clone());
            for mime in mimes { index.entry(mime).or_default().push(app.clone()); }
        }
    }
    index
}

// ── Cached MIME & Desktop Application Registry ─────────────────────────────────
static MIME_INDEX_CACHE: std::sync::RwLock<Option<HashMap<String, Vec<AppEntry>>>> = std::sync::RwLock::new(None);
static ICON_PATH_CACHE: std::sync::RwLock<Option<HashMap<String, Option<PathBuf>>>> = std::sync::RwLock::new(None);

fn get_cached_mime_index() -> HashMap<String, Vec<AppEntry>> {
    if let Ok(guard) = MIME_INDEX_CACHE.read() {
        if let Some(ref map) = *guard {
            return map.clone();
        }
    }
    let map = build_mime_index();
    if let Ok(mut guard) = MIME_INDEX_CACHE.write() {
        *guard = Some(map.clone());
    }
    map
}

pub fn get_open_with_info(path: String) -> Result<OpenWithInfo, String> {
    // 1. Önce dosya uzantısından anında tespit et (process spawn etmeden 0ms)
    let p = Path::new(&path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mime = if matches!(ext.as_str(), "exe" | "msi" | "bat" | "cmd") {
        "application/x-ms-dos-executable".to_string()
    } else {
        mime_guess::from_path(p)
            .first()
            .map(|m| m.to_string())
            .or_else(|| {
                // Sadece uzantısız dosyalarda xdg-mime fallback'i çalıştır
                let out = Command::new("xdg-mime").args(["query", "filetype", &path]).output().ok()?;
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            })
            .unwrap_or_else(|| "application/octet-stream".to_string())
    };

    let index = get_cached_mime_index();
    let mut apps = index.get(&mime).cloned().unwrap_or_default();

    // Eğer exe/msi/bat ise ve listede wine yoksa doğrudan wine ekle
    if matches!(ext.as_str(), "exe" | "msi" | "bat" | "cmd") {
        if !apps.iter().any(|a| a.id == "wine.desktop" || a.name.to_lowercase().contains("wine")) {
            if let Some(wine_entry) = find_desktop_entry_by_id("wine.desktop") {
                apps.insert(0, wine_entry);
            } else if Command::new("which").arg("wine").output().map(|o| o.status.success()).unwrap_or(false) {
                apps.insert(0, AppEntry {
                    id: "wine.desktop".to_string(),
                    name: "Wine Windows Program Loader".to_string(),
                    exec: "wine start /unix %f".to_string(),
                    icon: Some("wine".to_string()),
                    comment: Some("Windows uygulamalarını Wine ile çalıştır".to_string()),
                    categories: vec!["Utility".to_string()],
                });
            }
        }
    }

    // Default app'i bul
    let default_app = apps.first().map(|a| a.id.clone());

    Ok(OpenWithInfo { mime_type: mime, default_app, apps })
}

pub fn find_desktop_entry_by_id(id: &str) -> Option<AppEntry> {
    for dir in app_dirs() {
        let p = dir.join(id);
        if p.exists() {
            if let Some((app, _)) = parse_desktop_file(&p) {
                return Some(app);
            }
        }
    }
    None
}

pub fn open_with_app(path: String, desktop_id: String) -> Result<(), String> {
    let p = Path::new(&path);
    let parent = p.parent().unwrap_or_else(|| Path::new("/"));

    // Wine veya custom desktop için özel çalıştırma desteği
    if desktop_id == "wine.desktop" || desktop_id == "wine" {
        return open_with_wine(&path);
    }

    // İlk olarak .desktop dosyasını bulup içindeki Exec satırını güvenle parse et
    if let Some(app) = find_desktop_entry_by_id(&desktop_id) {
        if app.exec.contains("wine") {
            return open_with_wine(&path);
        }
        return open_with_exec(path, app.exec);
    }

    // gtk-launch fallback
    let status = Command::new("gtk-launch").arg(&desktop_id).arg(&path).current_dir(parent).spawn();
    if let Ok(_) = status {
        return Ok(());
    }

    // xdg-open fallback
    Command::new("xdg-open").arg(&path).current_dir(parent).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn open_with_wine(path: &str) -> Result<(), String> {
    use std::process::Stdio;
    let p = Path::new(path);
    let parent = p.parent().unwrap_or_else(|| Path::new("/"));
    // 1. wine start /unix <path> (arka planda sessizce başlat — terminal açma!)
    let res = Command::new("wine")
        .args(["start", "/unix", path])
        .current_dir(parent)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if res.is_ok() {
        return Ok(());
    }
    // 2. Doğrudan wine <path>
    Command::new("wine")
        .arg(path)
        .current_dir(parent)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Wine çalıştırma hatası: {}", e))?;
    Ok(())
}

pub fn set_default_app(mime_type: String, desktop_id: String) -> Result<(), String> {
    Command::new("xdg-mime").args(["default", &desktop_id, &mime_type])
        .status().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn reset_default_for_mime(mime_type: String) -> Result<(), String> {
    let mimeapps = dirs::config_dir().ok_or("no config dir")?.join("mimeapps.list");
    let content = fs::read_to_string(&mimeapps).unwrap_or_default();
    let filtered: Vec<&str> = content.lines()
        .filter(|l| !l.trim().starts_with(&format!("{}=", mime_type)))
        .collect();
    fs::write(&mimeapps, filtered.join("\n") + "\n").map_err(|e| e.to_string())
}

// ─── Custom app storage ───────────────────────────────────────────────────────

fn custom_apps_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("custom_apps.json")
}

pub fn load_custom_apps() -> Vec<AppEntry> {
    let p = custom_apps_path();
    if !p.exists() { return vec![]; }
    let data = fs::read_to_string(&p).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save_custom_app(name: String, exec: String) -> Result<AppEntry, String> {
    let mut apps = load_custom_apps();
    let id = format!("arch-explorer-custom-{}.desktop",
        name.to_lowercase().replace(' ', "-"));
    // Varsa güncelle, yoksa ekle
    let entry = AppEntry {
        id: id.clone(),
        name: name.clone(),
        exec: exec.clone(),
        icon: None,
        comment: Some("Custom — added via Arch Explorer".into()),
        categories: vec![],
    };
    if let Some(e) = apps.iter_mut().find(|a| a.id == id) {
        *e = entry.clone();
    } else {
        apps.push(entry.clone());
    }
    let p = custom_apps_path();
    if let Some(parent) = p.parent() { fs::create_dir_all(parent).ok(); }
    fs::write(&p, serde_json::to_string_pretty(&apps).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(entry)
}

pub fn remove_custom_app(id: &str) -> Result<(), String> {
    let mut apps = load_custom_apps();
    apps.retain(|a| a.id != id);
    let p = custom_apps_path();
    fs::write(&p, serde_json::to_string_pretty(&apps).unwrap_or_default())
        .map_err(|e| e.to_string())
}

/// Verilen exec string'i direkt çalıştır (custom app launcher)
pub fn open_with_exec(file_path: String, exec: String) -> Result<(), String> {
    let cmd = exec.replace("%f", &file_path)
                  .replace("%F", &file_path)
                  .replace("%u", &file_path)
                  .replace("%U", &file_path);
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return Err("Boş komut".into()); }
    Command::new(parts[0])
        .args(&parts[1..])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// İkon adından gerçek PNG dosyasını bul
pub fn find_app_icon(icon_name: &str) -> Option<std::path::PathBuf> {
    if icon_name.is_empty() { return None; }
    // Cache kontrolü
    if let Ok(guard) = ICON_PATH_CACHE.read() {
        if let Some(ref cache) = *guard {
            if let Some(res) = cache.get(icon_name) {
                return res.clone();
            }
        }
    }

    // Mutlak path
    let p = std::path::Path::new(icon_name);
    if p.is_absolute() && p.exists() {
        let res = Some(p.to_path_buf());
        if let Ok(mut guard) = ICON_PATH_CACHE.write() {
            guard.get_or_insert_with(HashMap::new).insert(icon_name.to_string(), res.clone());
        }
        return res;
    }

    let home = dirs::home_dir().unwrap_or_default();
    let sizes = ["48x48", "32x32", "64x64", "128x128", "256x256", "scalable"];
    let themes = ["hicolor", "Papirus", "breeze", "Adwaita"];
    let exts   = ["png", "svg", "xpm"];

    // Sistem icon dizinleri
    let search_dirs: Vec<std::path::PathBuf> = vec![
        home.join(".local/share/icons"),
        std::path::PathBuf::from("/usr/share/icons"),
        std::path::PathBuf::from("/usr/share/pixmaps"),
        home.join(".icons"),
    ];

    let mut found = None;
    'search: for base in &search_dirs {
        // pixmaps — doğrudan
        for ext in &exts {
            let p = base.join(format!("{}.{}", icon_name, ext));
            if p.exists() {
                found = Some(p);
                break 'search;
            }
        }
        // theme/size/apps
        for theme in &themes {
            for size in &sizes {
                for ext in &exts {
                    let p = base.join(theme).join(size).join("apps")
                                .join(format!("{}.{}", icon_name, ext));
                    if p.exists() {
                        found = Some(p);
                        break 'search;
                    }
                }
            }
        }
    }

    if let Ok(mut guard) = ICON_PATH_CACHE.write() {
        guard.get_or_insert_with(HashMap::new).insert(icon_name.to_string(), found.clone());
    }
    found
}

/// Hızlı open-with için: default + ilk 3 recommended
pub fn get_quick_apps(file_path: &str) -> (Option<AppEntry>, Vec<AppEntry>) {
    let Ok(info) = get_open_with_info(file_path.to_string()) else {
        return (None, vec![]);
    };
    let default = info.default_app.as_ref().and_then(|id| {
        info.apps.iter().find(|a| &a.id == id).cloned()
    });
    let recommended: Vec<AppEntry> = info.apps.iter()
        .filter(|a| Some(&a.id) != info.default_app.as_ref())
        .take(3)
        .cloned()
        .collect();
    (default, recommended)
}

/// Sistemdeki TÜM uygulamaları döner — rofi --drun gibi
/// .desktop dosyaları taranır, NoDisplay=true olanlar hariç tutulur
pub fn get_all_apps() -> Vec<AppEntry> {
    let home = dirs::home_dir().unwrap_or_default();

    let search_dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        home.join(".local/share/applications"),
        // Flatpak
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home.join(".local/share/flatpak/exports/share/applications"),
        // Snap
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    let mut apps: Vec<AppEntry> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for dir in &search_dirs {
        if !dir.exists() { continue; }
        let Ok(entries) = fs::read_dir(dir) else { continue; };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(ext) = path.extension() else { continue; };
            if ext != "desktop" { continue; }

            if let Some((app, _mimes)) = parse_desktop_file(&path) {
                if seen.insert(app.id.clone()) {
                    apps.push(app);
                }
            }
        }
    }

    // Custom kaydedilmişleri de ekle
    for custom in load_custom_apps() {
        if seen.insert(custom.id.clone()) {
            apps.push(custom);
        }
    }

    // İsme göre sırala
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}


