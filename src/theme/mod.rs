// src/theme/mod.rs
// AEX Theme Engine — load, parse, merge with defaults, export to egui::Style

pub mod types;

pub use types::AexTheme;

use std::path::{Path, PathBuf};
use std::fs;

// ─────────────────────────────────────────────────────────────────────────────
// Theme search paths (öncelik sırasıyla)
// ─────────────────────────────────────────────────────────────────────────────

pub fn theme_search_paths() -> Vec<PathBuf> {
    let mut paths = vec![];

    // 1. ~/.config/arch-explorer/themes/
    if let Some(cfg) = dirs::config_dir() {
        paths.push(cfg.join("arch-explorer").join("themes"));
    }

    // 2. ~/.local/share/arch-explorer/themes/
    if let Some(data) = dirs::data_local_dir() {
        paths.push(data.join("arch-explorer").join("themes"));
    }

    // 3. /usr/share/arch-explorer/themes/ (sistem geneli)
    paths.push(PathBuf::from("/usr/share/arch-explorer/themes"));

    // 4. binary yanındaki themes/ klasörü (geliştirme için)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("themes"));
        }
    }

    paths
}

// ─────────────────────────────────────────────────────────────────────────────
// .aex dosyası yükle
// ─────────────────────────────────────────────────────────────────────────────

pub fn load_theme_file(path: &Path) -> anyhow::Result<AexTheme> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Theme dosyası okunamadı {}: {}", path.display(), e))?;

    let theme: AexTheme = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Theme parse hatası {}: {}", path.display(), e))?;

    Ok(theme)
}

/// İsme göre tema yükle — arama yollarında "name.aex" dosyasını bulur
pub fn load_theme_by_name(name: &str) -> Option<AexTheme> {
    let filename = if name.ends_with(".aex") {
        name.to_string()
    } else {
        format!("{}.aex", name)
    };

    for dir in theme_search_paths() {
        let path = dir.join(&filename);
        if path.exists() {
            if let Ok(theme) = load_theme_file(&path) {
                log::info!("Tema yüklendi: {} ({})", name, path.display());
                return Some(theme);
            }
        }
    }

    log::warn!("Tema bulunamadı: {} — built-in default kullanılıyor", name);
    None
}

/// Tüm mevcut .aex dosyalarını listele
pub fn list_available_themes() -> Vec<ThemeMeta> {
    let mut themes = vec![];
    let mut seen = std::collections::HashSet::new();

    // Built-in temalar — HashMap sırası random, BTreeMap ile alfabetik al
    let mut builtins: Vec<_> = defaults::builtin_themes().into_iter().collect();
    builtins.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, theme) in builtins {
        if seen.insert(name.clone()) {
            themes.push(ThemeMeta {
                name: theme.meta.name.clone(),
                id: name,
                author: theme.meta.author.clone(),
                description: theme.meta.description.clone(),
                path: None,
                builtin: true,
            });
        }
    }

    // Dosya sistemi taraması
    for dir in theme_search_paths() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("aex") { continue; }

            let id = path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if seen.insert(id.clone()) {
                if let Ok(theme) = load_theme_file(&path) {
                    themes.push(ThemeMeta {
                        name: theme.meta.name.clone(),
                        id,
                        author: theme.meta.author.clone(),
                        description: theme.meta.description.clone(),
                        path: Some(path),
                        builtin: false,
                    });
                }
            }
        }
    }

    themes
}

#[derive(Debug, Clone)]
pub struct ThemeMeta {
    pub name:        String,
    pub id:          String,
    pub author:      String,
    pub description: String,
    pub path:        Option<PathBuf>,
    pub builtin:     bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// ThemeEngine — runtime'da aktif tema + hot-reload
// ─────────────────────────────────────────────────────────────────────────────

pub struct ThemeEngine {
    pub active: AexTheme,
    pub active_id: String,
    pub available: Vec<ThemeMeta>,
    last_check: std::time::Instant,
    watch_path: Option<PathBuf>,
}

impl ThemeEngine {
    /// Ayar dosyasından tema adını oku ve yükle, yoksa default
    pub fn init() -> Self {
        let active_id = load_saved_theme_name().unwrap_or_else(|| "default".into());
        let active = load_theme_by_name(&active_id)
            .unwrap_or_else(|| defaults::default_theme());
        let available = list_available_themes();

        Self {
            active,
            active_id,
            available,
            last_check: std::time::Instant::now(),
            watch_path: None,
        }
    }

    /// Tema değiştir
    pub fn switch_to(&mut self, id: &str, ctx: &egui::Context) {
        let theme = load_theme_by_name(id)
            .unwrap_or_else(|| defaults::default_theme());

        // egui style'ı güncelle
        ctx.set_style(egui_bridge::aex_to_egui_style(&theme));

        // window opacity — eframe viewport'a iletilir
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true));

        self.active = theme;
        self.active_id = id.to_string();
        save_theme_name(id);

        log::info!("Tema değiştirildi: {}", id);
    }

    /// Hot-reload: .aex dosyası değiştiyse yeniden yükle (2 saniyede bir kontrol)
    pub fn tick_hot_reload(&mut self, ctx: &egui::Context) {
        if self.last_check.elapsed().as_secs() < 2 { return; } // FIX: was `!x >= 2` (bitwise NOT, always skipped)
        self.last_check = std::time::Instant::now();

        if let Some(path) = &self.watch_path.clone() {
            if let Ok(theme) = load_theme_file(path) {
                ctx.set_style(egui_bridge::aex_to_egui_style(&theme));
                self.active = theme;
                log::debug!("Hot-reload: tema güncellendi");
            }
        }
    }

    /// Mevcut tema listesini yenile
    pub fn refresh_available(&mut self) {
        self.available = list_available_themes();
    }

    /// Aktif tema üzerinde bir rengi değiştir (runtime override — dosyayı değiştirmez)
    pub fn override_color(&mut self, field: &str, hex: &str, ctx: &egui::Context) {
        match field {
            "accent"       => self.active.colors.accent = hex.into(),
            "bg_base"      => self.active.colors.bg_base = hex.into(),
            "text_primary" => self.active.colors.text_primary = hex.into(),
            _ => { log::warn!("Bilinmeyen renk alanı: {}", field); return; }
        }
        ctx.set_style(egui_bridge::aex_to_egui_style(&self.active));
    }

    /// Aktif tema ayarlarını (opaklık, efektler, renkler) diske ~/.config/arch-explorer/themes/{id}.aex olarak kaydeder
    pub fn save_active_theme(&self) {
        let toml_str = match toml::to_string_pretty(&self.active) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Tema serialize edilemedi: {}", e);
                return;
            }
        };

        let user_themes_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("arch-explorer")
            .join("themes");

        if let Err(e) = fs::create_dir_all(&user_themes_dir) {
            log::error!("Tema klasörü oluşturulamadı: {}", e);
            return;
        }

        let theme_file = user_themes_dir.join(format!("{}.aex", self.active_id));
        if let Err(e) = fs::write(&theme_file, toml_str) {
            log::error!("Tema dosyası kaydedilemedi {}: {}", theme_file.display(), e);
        } else {
            log::info!("Tema ayarları diske kaydedildi: {}", theme_file.display());
            save_theme_name(&self.active_id);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Persistence helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("settings.toml")
}

fn load_saved_theme_name() -> Option<String> {
    let content = fs::read_to_string(settings_path()).ok()?;
    let table: toml::Table = toml::from_str(&content).ok()?;
    table.get("theme")?.as_str().map(String::from)
}

fn save_theme_name(name: &str) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut table: toml::Table = fs::read_to_string(&path)
        .ok()
        .and_then(|c| toml::from_str(&c).ok())
        .unwrap_or_default();

    table.insert("theme".to_string(), toml::Value::String(name.to_string()));

    if let Ok(serialized) = toml::to_string_pretty(&table) {
        fs::write(&path, serialized).ok();
    }
}

pub fn load_saved_view_scale() -> Option<f32> {
    let content = fs::read_to_string(settings_path()).ok()?;
    let table: toml::Table = toml::from_str(&content).ok()?;
    let val = table.get("view_scale")?;
    if let Some(f) = val.as_float() {
        Some(f as f32)
    } else if let Some(i) = val.as_integer() {
        Some(i as f32)
    } else {
        None
    }
}

pub fn save_view_scale(scale: f32) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut table: toml::Table = fs::read_to_string(&path)
        .ok()
        .and_then(|c| toml::from_str(&c).ok())
        .unwrap_or_default();

    table.insert("view_scale".to_string(), toml::Value::Float(scale as f64));

    if let Ok(serialized) = toml::to_string_pretty(&table) {
        fs::write(&path, serialized).ok();
    }
}

pub fn load_dir_view_modes() -> std::collections::HashMap<String, String> {
    let content = fs::read_to_string(settings_path()).ok().unwrap_or_default();
    let table: toml::Table = toml::from_str(&content).ok().unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    if let Some(tbl) = table.get("dir_view_modes").and_then(|v| v.as_table()) {
        for (k, v) in tbl {
            if let Some(s) = v.as_str() {
                map.insert(k.clone(), s.to_string());
            }
        }
    }
    map
}

pub fn save_dir_view_mode(dir_path: &str, mode: &str) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut table: toml::Table = fs::read_to_string(&path)
        .ok()
        .and_then(|c| toml::from_str(&c).ok())
        .unwrap_or_default();

    let clean = dir_path.trim_end_matches('/');
    let key = if clean.is_empty() { "/" } else { clean };

    let mut modes_table = table.get("dir_view_modes")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    modes_table.insert(key.to_string(), toml::Value::String(mode.to_string()));
    table.insert("dir_view_modes".to_string(), toml::Value::Table(modes_table));

    if let Ok(serialized) = toml::to_string_pretty(&table) {
        fs::write(&path, serialized).ok();
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// src/theme/defaults.rs inline (built-in temalar)
// ─────────────────────────────────────────────────────────────────────────────

pub mod defaults {
    use super::types::AexTheme;
    use std::collections::HashMap;

    pub fn default_theme() -> AexTheme {
        AexTheme::default()
    }

    pub fn catppuccin_mocha() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Catppuccin Mocha".into();
        t.meta.author = "catppuccin".into();
        t.meta.description = "Soothing pastel theme for the high-spirited".into();
        t.colors.bg_base     = "#1e1e2e".into();
        t.colors.bg_mantle   = "#181825".into();
        t.colors.bg_crust    = "#11111b".into();
        t.colors.bg_surface0 = "#313244".into();
        t.colors.bg_surface1 = "#45475a".into();
        t.colors.bg_surface2 = "#585b70".into();
        t.colors.accent      = "#cba6f7".into();
        t.colors.accent_dim  = "#9874c9".into();
        t.colors.accent_glow = "#cba6f730".into();
        t.colors.text_primary   = "#cdd6f4".into();
        t.colors.text_secondary = "#bac2de".into();
        t.colors.text_hint      = "#6c7086".into();
        t.colors.text_disabled  = "#45475a".into();
        t.colors.success = "#a6e3a1".into();
        t.colors.warning = "#f9e2af".into();
        t.colors.danger  = "#f38ba8".into();
        t.colors.info    = "#89dceb".into();
        t.colors.border       = "#45475a".into();
        t.colors.border_focus = "#cba6f7".into();
        t.effects.window_opacity  = 0.87;
        t.effects.sidebar_opacity = 0.76;
        t.effects.panel_opacity   = 0.89;
        t.effects.card_opacity    = 0.82;
        t.effects.toolbar_opacity = 0.81;
        t
    }

    pub fn nord() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Nord".into();
        t.meta.author = "arcticicestudio".into();
        t.meta.description = "An arctic, north-bluish color palette".into();
        t.colors.bg_base     = "#2e3440".into();
        t.colors.bg_mantle   = "#242933".into();
        t.colors.bg_crust    = "#1c2028".into();
        t.colors.bg_surface0 = "#3b4252".into();
        t.colors.bg_surface1 = "#434c5e".into();
        t.colors.bg_surface2 = "#4c566a".into();
        t.colors.accent      = "#88c0d0".into();
        t.colors.accent_dim  = "#5e97ad".into();
        t.colors.accent_glow = "#88c0d030".into();
        t.colors.text_primary   = "#eceff4".into();
        t.colors.text_secondary = "#d8dee9".into();
        t.colors.text_hint      = "#616e88".into();
        t.colors.text_disabled  = "#434c5e".into();
        t.colors.success = "#a3be8c".into();
        t.colors.warning = "#ebcb8b".into();
        t.colors.danger  = "#bf616a".into();
        t.colors.info    = "#81a1c1".into();
        t.colors.border       = "#3b4252".into();
        t.colors.border_focus = "#88c0d0".into();
        t.effects.window_opacity  = 0.92;
        t.effects.sidebar_opacity = 0.83;
        t.effects.panel_opacity   = 0.94;
        t.effects.card_opacity    = 0.88;
        t.effects.toolbar_opacity = 0.86;
        t
    }

    pub fn gruvbox() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Gruvbox Dark".into();
        t.meta.author = "morhetz".into();
        t.meta.description = "Retro groove color scheme".into();
        t.colors.bg_base     = "#1d2021".into();
        t.colors.bg_mantle   = "#282828".into();
        t.colors.bg_crust    = "#141617".into();
        t.colors.bg_surface0 = "#3c3836".into();
        t.colors.bg_surface1 = "#504945".into();
        t.colors.bg_surface2 = "#665c54".into();
        t.colors.accent      = "#d79921".into();
        t.colors.accent_dim  = "#b57614".into();
        t.colors.accent_glow = "#d7992130".into();
        t.colors.text_primary   = "#ebdbb2".into();
        t.colors.text_secondary = "#d5c4a1".into();
        t.colors.text_hint      = "#928374".into();
        t.colors.text_disabled  = "#504945".into();
        t.colors.success = "#b8bb26".into();
        t.colors.warning = "#fabd2f".into();
        t.colors.danger  = "#fb4934".into();
        t.colors.info    = "#83a598".into();
        t.colors.border       = "#3c3836".into();
        t.colors.border_focus = "#d79921".into();
        t.effects.window_opacity  = 0.96;
        t.effects.sidebar_opacity = 0.90;
        t.effects.panel_opacity   = 0.97;
        t.effects.card_opacity    = 0.93;
        t.effects.toolbar_opacity = 0.92;
        t
    }

    pub fn rose_pine() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Rosé Pine".into();
        t.meta.author = "rose-pine".into();
        t.meta.description = "All natural pine, faux fur and a bit of soho vibes".into();
        t.colors.bg_base     = "#191724".into();
        t.colors.bg_mantle   = "#1f1d2e".into();
        t.colors.bg_crust    = "#26233a".into();
        t.colors.bg_surface0 = "#2a2837".into();
        t.colors.bg_surface1 = "#403d52".into();
        t.colors.bg_surface2 = "#524f67".into();
        t.colors.accent      = "#c4a7e7".into();
        t.colors.accent_dim  = "#9678bf".into();
        t.colors.accent_glow = "#c4a7e730".into();
        t.colors.text_primary   = "#e0def4".into();
        t.colors.text_secondary = "#908caa".into();
        t.colors.text_hint      = "#6e6a86".into();
        t.colors.text_disabled  = "#403d52".into();
        t.colors.success = "#31748f".into();
        t.colors.warning = "#f6c177".into();
        t.colors.danger  = "#eb6f92".into();
        t.colors.info    = "#9ccfd8".into();
        t.colors.border       = "#403d52".into();
        t.colors.border_focus = "#c4a7e7".into();
        t.effects.window_opacity  = 0.84;
        t.effects.sidebar_opacity = 0.73;
        t.effects.panel_opacity   = 0.88;
        t.effects.card_opacity    = 0.82;
        t.effects.toolbar_opacity = 0.78;
        t
    }

    pub fn rose_dark() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Rose Dark".into();
        t.meta.author = "arch-explorer".into();
        t.meta.description = "Neon rose and dark purple palette matching the design".into();
        t.colors.bg_base     = "#0f0f17".into();
        t.colors.bg_mantle   = "#13131e".into();
        t.colors.bg_crust    = "#0a0a10".into();
        t.colors.bg_surface0 = "#1b1b2a".into();
        t.colors.bg_surface1 = "#27273d".into();
        t.colors.bg_surface2 = "#4e2738".into();
        t.colors.accent      = "#ff6b9d".into();
        t.colors.accent_dim  = "#cc4f7c".into();
        t.colors.accent_glow = "#ff6b9d30".into();
        t.colors.text_primary   = "#f0eeff".into();
        t.colors.text_secondary = "#9a9ab2".into();
        t.colors.text_hint      = "#60607a".into();
        t.colors.text_disabled  = "#3a3a52".into();
        t.colors.success = "#5aff8e".into();
        t.colors.warning = "#ffd166".into();
        t.colors.danger  = "#ff5f6d".into();
        t.colors.info    = "#8264ff".into();
        t.colors.border       = "#232336".into();
        t.colors.border_focus = "#ff6b9d".into();
        t.effects.window_opacity  = 0.85;
        t.effects.sidebar_opacity = 0.75;
        t.effects.panel_opacity   = 0.90;
        t.effects.card_opacity    = 0.85;
        t.effects.toolbar_opacity = 0.80;
        t
    }

    pub fn tokyo_night() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Tokyo Night".into();
        t.meta.author = "folke".into();
        t.meta.description = "A clean, dark Neovim theme that celebrates the lights of Downtown Tokyo".into();
        t.colors.bg_base     = "#1a1b26".into();
        t.colors.bg_mantle   = "#16161e".into();
        t.colors.bg_crust    = "#13141c".into();
        t.colors.bg_surface0 = "#24283b".into();
        t.colors.bg_surface1 = "#2f3549".into();
        t.colors.bg_surface2 = "#383e5a".into();
        t.colors.accent      = "#7aa2f7".into();
        t.colors.accent_dim  = "#3d59a1".into();
        t.colors.accent_glow = "#7aa2f730".into();
        t.colors.text_primary   = "#c0caf5".into();
        t.colors.text_secondary = "#9aa5ce".into();
        t.colors.text_hint      = "#565f89".into();
        t.colors.text_disabled  = "#414868".into();
        t.colors.success = "#9ece6a".into();
        t.colors.warning = "#e0af68".into();
        t.colors.danger  = "#f7768e".into();
        t.colors.info    = "#7dcfff".into();
        t.colors.border       = "#292e42".into();
        t.colors.border_focus = "#7aa2f7".into();
        t.effects.window_opacity  = 0.90;
        t.effects.sidebar_opacity = 0.80;
        t.effects.panel_opacity   = 0.92;
        t.effects.card_opacity    = 0.86;
        t.effects.toolbar_opacity = 0.84;
        t
    }

    pub fn dracula() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Dracula".into();
        t.meta.author = "dracula".into();
        t.meta.description = "Famous dark theme for hackers and vampires".into();
        t.colors.bg_base     = "#282a36".into();
        t.colors.bg_mantle   = "#21222c".into();
        t.colors.bg_crust    = "#191a21".into();
        t.colors.bg_surface0 = "#343746".into();
        t.colors.bg_surface1 = "#44475a".into();
        t.colors.bg_surface2 = "#6272a4".into();
        t.colors.accent      = "#bd93f9".into();
        t.colors.accent_dim  = "#ff79c6".into();
        t.colors.accent_glow = "#bd93f930".into();
        t.colors.text_primary   = "#f8f8f2".into();
        t.colors.text_secondary = "#bfbfbf".into();
        t.colors.text_hint      = "#6272a4".into();
        t.colors.text_disabled  = "#44475a".into();
        t.colors.success = "#50fa7b".into();
        t.colors.warning = "#f1fa8c".into();
        t.colors.danger  = "#ff5555".into();
        t.colors.info    = "#8be9fd".into();
        t.colors.border       = "#44475a".into();
        t.colors.border_focus = "#bd93f9".into();
        t.effects.window_opacity  = 0.95;
        t.effects.sidebar_opacity = 0.88;
        t.effects.panel_opacity   = 0.96;
        t.effects.card_opacity    = 0.90;
        t.effects.toolbar_opacity = 0.90;
        t
    }

    pub fn cyberpunk() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Cyberpunk Neon".into();
        t.meta.author = "arch-explorer".into();
        t.meta.description = "High contrast futuristic neon theme".into();
        t.colors.bg_base     = "#0b0c16".into();
        t.colors.bg_mantle   = "#070710".into();
        t.colors.bg_crust    = "#04040a".into();
        t.colors.bg_surface0 = "#1a1c2e".into();
        t.colors.bg_surface1 = "#262947".into();
        t.colors.bg_surface2 = "#3b1b3b".into();
        t.colors.accent      = "#00ffcc".into();
        t.colors.accent_dim  = "#ff0055".into();
        t.colors.accent_glow = "#00ffcc30".into();
        t.colors.text_primary   = "#f0f6fc".into();
        t.colors.text_secondary = "#8b949e".into();
        t.colors.text_hint      = "#484f58".into();
        t.colors.text_disabled  = "#30363d".into();
        t.colors.success = "#00ff66".into();
        t.colors.warning = "#ffee00".into();
        t.colors.danger  = "#ff0055".into();
        t.colors.info    = "#00e5ff".into();
        t.colors.border       = "#1e2238".into();
        t.colors.border_focus = "#00ffcc".into();
        t.effects.window_opacity  = 0.75;
        t.effects.sidebar_opacity = 0.65;
        t.effects.panel_opacity   = 0.80;
        t.effects.card_opacity    = 0.72;
        t.effects.toolbar_opacity = 0.70;
        t
    }

    pub fn solarized_dark() -> AexTheme {
        let mut t = AexTheme::default();
        t.meta.name = "Solarized Dark".into();
        t.meta.author = "Ethan Schoonover".into();
        t.meta.description = "Precision colors for machines and people".into();
        t.colors.bg_base     = "#002b36".into();
        t.colors.bg_mantle   = "#073642".into();
        t.colors.bg_crust    = "#00212b".into();
        t.colors.bg_surface0 = "#0a4250".into();
        t.colors.bg_surface1 = "#0e5060".into();
        t.colors.bg_surface2 = "#146274".into();
        t.colors.accent      = "#268bd2".into();
        t.colors.accent_dim  = "#2aa198".into();
        t.colors.accent_glow = "#268bd230".into();
        t.colors.text_primary   = "#93a1a1".into();
        t.colors.text_secondary = "#839496".into();
        t.colors.text_hint      = "#586e75".into();
        t.colors.text_disabled  = "#073642".into();
        t.colors.success = "#859900".into();
        t.colors.warning = "#b58900".into();
        t.colors.danger  = "#dc322f".into();
        t.colors.info    = "#6c71c4".into();
        t.colors.border       = "#0a4250".into();
        t.colors.border_focus = "#268bd2".into();
        t.effects.window_opacity  = 0.89;
        t.effects.sidebar_opacity = 0.79;
        t.effects.panel_opacity   = 0.91;
        t.effects.card_opacity    = 0.85;
        t.effects.toolbar_opacity = 0.83;
        t
    }

    pub fn builtin_themes() -> HashMap<String, AexTheme> {
        let mut map = HashMap::new();
        map.insert("default".into(),           default_theme());
        map.insert("rose-dark".into(),         rose_dark());
        map.insert("tokyo-night".into(),       tokyo_night());
        map.insert("dracula".into(),           dracula());
        map.insert("cyberpunk".into(),         cyberpunk());
        map.insert("solarized-dark".into(),    solarized_dark());
        map.insert("catppuccin-mocha".into(),  catppuccin_mocha());
        map.insert("nord".into(),              nord());
        map.insert("gruvbox".into(),           gruvbox());
        map.insert("rose-pine".into(),         rose_pine());
        map
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// src/theme/egui_bridge.rs inline
// ─────────────────────────────────────────────────────────────────────────────

pub mod egui_bridge {
    use super::types;
    use super::types::AexTheme;
    use egui::{style::*, Color32, FontId, FontFamily, Margin, Rounding, Shadow, Stroke, Vec2};

    /// AexTheme → egui::Style dönüşümü
    /// Bu fonksiyon her tema değişiminde çağrılır
    pub fn aex_to_egui_style(t: &AexTheme) -> std::sync::Arc<egui::Style> {
        let c = &t.colors;
        let s = &t.shape;
        let sp = &t.spacing;

        let mut style = egui::Style::default();

        // ── Visuals ──────────────────────────────────────────────────────
        let mut vis = Visuals::dark();

        vis.override_text_color = Some(c.text_primary());
        vis.hyperlink_color     = c.accent();
        vis.faint_bg_color      = c.bg_surface0();
        vis.extreme_bg_color    = c.bg_crust();
        vis.code_bg_color       = c.bg_mantle();
        vis.warn_fg_color       = c.warning();
        vis.error_fg_color      = c.danger();
        vis.selection           = Selection {
            bg_fill:     Color32::from_rgba_unmultiplied(
                c.accent().r(), c.accent().g(), c.accent().b(), 60
            ),
            stroke:      Stroke::new(1.0_f32, c.accent()),
        };

        // Panel backgrounds
        vis.panel_fill   = if t.effects.glass_panels {
            // Semi-transparent for glass effect (Hyprland blur yapar gerisini)
            Color32::from_rgba_unmultiplied(
                c.bg_mantle().r(), c.bg_mantle().g(), c.bg_mantle().b(),
                (t.effects.panel_opacity * 255.0) as u8,
            )
        } else {
            c.bg_mantle()
        };

        vis.window_fill  = if t.effects.glass_panels {
            Color32::from_rgba_unmultiplied(
                c.bg_base().r(), c.bg_base().g(), c.bg_base().b(),
                (t.effects.window_opacity * 255.0) as u8,
            )
        } else {
            c.bg_base()
        };
        vis.window_stroke = Stroke::new(1.0_f32, c.border());
        vis.window_rounding = Rounding::same(s.radius_lg);
        vis.window_shadow  = if t.effects.shadow_enabled {
            Shadow {
                offset: Vec2::new(t.effects.shadow_offset_x, t.effects.shadow_offset_y),
                blur:   t.effects.shadow_blur,
                spread: 0.0,
                color:  types::hex_to_color32(&t.effects.shadow_color),
            }
        } else {
            Shadow::NONE
        };

        // Widgets (normal state)
        vis.widgets.noninteractive = WidgetVisuals {
            bg_fill:   c.bg_surface0(),
            weak_bg_fill: c.bg_mantle(),
            bg_stroke: Stroke::new(1.0_f32, c.border()),
            rounding:  Rounding::same(s.radius_md),
            fg_stroke: Stroke::new(1.0_f32, c.text_secondary()),
            expansion: 0.0,
        };

        // Inactive (hoverable)
        vis.widgets.inactive = WidgetVisuals {
            bg_fill:   c.bg_surface0(),
            weak_bg_fill: c.bg_mantle(),
            bg_stroke: Stroke::new(1.0_f32, c.border()),
            rounding:  Rounding::same(s.radius_md),
            fg_stroke: Stroke::new(1.0_f32, c.text_primary()),
            expansion: 0.0,
        };

        // Hovered
        vis.widgets.hovered = WidgetVisuals {
            bg_fill:   c.bg_surface1(),
            weak_bg_fill: c.bg_surface0(),
            bg_stroke: Stroke::new(1.0_f32, c.border_focus()),
            rounding:  Rounding::same(s.radius_md),
            fg_stroke: Stroke::new(1.5_f32, c.text_primary()),
            expansion: 1.0,
        };

        // Active (pressed)
        vis.widgets.active = WidgetVisuals {
            bg_fill:   c.bg_surface2(),
            weak_bg_fill: c.bg_surface1(),
            bg_stroke: Stroke::new(1.0_f32, c.accent()),
            rounding:  Rounding::same(s.radius_md),
            fg_stroke: Stroke::new(2.0_f32, c.accent()),
            expansion: 1.0,
        };

        // Open (dropdown vb.)
        vis.widgets.open = WidgetVisuals {
            bg_fill:   c.bg_surface1(),
            weak_bg_fill: c.bg_surface0(),
            bg_stroke: Stroke::new(1.0_f32, c.accent()),
            rounding:  Rounding::same(s.radius_md),
            fg_stroke: Stroke::new(1.5_f32, c.accent()),
            expansion: 0.0,
        };

        style.visuals = vis;

        // ── Spacing ──────────────────────────────────────────────────────
        style.spacing.item_spacing    = Vec2::new(sp.sm, sp.sm);
        style.spacing.button_padding  = Vec2::new(sp.md, sp.sm);
        style.spacing.menu_margin     = Margin::same(sp.sm);
        style.spacing.indent          = sp.xl;
        style.spacing.interact_size   = Vec2::new(sp.xxl * 2.0, t.layout.item_height);
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.handle_min_length = 24.0;
        style.spacing.scroll.bar_inner_margin = 2.0;
        style.spacing.scroll.bar_outer_margin = 2.0;

        // ── Text Styles ───────────────────────────────────────────────────
        style.text_styles = {
            use egui::TextStyle::*;
            [
                (Small,   FontId::new(t.typography.size_xs,   FontFamily::Proportional)),
                (Body,    FontId::new(t.typography.size_base, FontFamily::Proportional)),
                (Button,  FontId::new(t.typography.size_base, FontFamily::Proportional)),
                (Heading, FontId::new(t.typography.size_xl,   FontFamily::Proportional)),
                (Monospace, FontId::new(t.typography.size_base, FontFamily::Monospace)),
            ]
            .into()
        };

        std::sync::Arc::new(style)
    }

    /// Accent rengiyle primary button style — manual çizimler için
    pub fn accent_button_bg(t: &AexTheme, hovered: bool, active: bool) -> Color32 {
        if active       { t.colors.accent_dim() }
        else if hovered { t.colors.accent() }
        else            { t.colors.accent_dim() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rose_dark_theme() {
        let builtins = defaults::builtin_themes();
        let rose = builtins.get("rose-dark").expect("rose-dark theme must be present");
        assert_eq!(rose.meta.name, "Rose Dark");
        assert_eq!(rose.colors.accent, "#ff6b9d");
        assert_eq!(rose.colors.bg_base, "#0f0f17");
        assert_eq!(rose.effects.window_opacity, 0.85);
        assert_eq!(rose.effects.sidebar_opacity, 0.75);
        assert_eq!(rose.effects.card_opacity, 0.85);
        assert_eq!(rose.effects.toolbar_opacity, 0.80);
        assert_eq!(rose.effects.panel_opacity, 0.90);
    }

    #[test]
    fn test_transparency_effects_and_style() {
        let theme = defaults::rose_dark();
        let style = egui_bridge::aex_to_egui_style(&theme);
        assert_eq!(style.visuals.window_fill.a(), (0.85 * 255.0) as u8);
        assert_eq!(style.visuals.panel_fill.a(), (0.90 * 255.0) as u8);
    }
}
