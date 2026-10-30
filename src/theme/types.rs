// src/theme/types.rs
// AEX (Arch Explorer) theme format — tam struct tanımları
// Bu dosya ileride aex-theme crate'ine taşınacak

use serde::{Deserialize, Serialize};
use egui::Color32;

// ─────────────────────────────────────────────────────────────────────────────
// Yardımcı: hex string → Color32
// ─────────────────────────────────────────────────────────────────────────────

pub fn hex_to_color32(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);

    match hex.len() {
        6 => Color32::from_rgb(
            parse(&hex[0..2]),
            parse(&hex[2..4]),
            parse(&hex[4..6]),
        ),
        8 => Color32::from_rgba_premultiplied(
            parse(&hex[0..2]),
            parse(&hex[2..4]),
            parse(&hex[4..6]),
            parse(&hex[6..8]),
        ),
        3 => {
            // shorthand #rgb → #rrggbb
            let r = parse(&hex[0..1].repeat(2));
            let g = parse(&hex[1..2].repeat(2));
            let b = parse(&hex[2..3].repeat(2));
            Color32::from_rgb(r, g, b)
        }
        _ => Color32::from_rgb(128, 128, 128), // fallback gri
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ana AEX Theme struct — tam .aex dosyasını temsil eder
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexTheme {
    #[serde(default)]
    pub meta: AexMeta,

    #[serde(default)]
    pub colors: AexColors,

    #[serde(default)]
    pub typography: AexTypography,

    #[serde(default)]
    pub spacing: AexSpacing,

    #[serde(default)]
    pub shape: AexShape,

    #[serde(default)]
    pub effects: AexEffects,

    #[serde(default)]
    pub layout: AexLayout,

    #[serde(default)]
    pub icons: AexIcons,

    #[serde(default)]
    pub animations: AexAnimations,

    #[serde(default)]
    pub sidebar: AexSidebar,

    #[serde(default)]
    pub file_grid: AexFileGrid,

    #[serde(default)]
    pub context_menu: AexContextMenu,

    #[serde(default)]
    pub keybinds: AexKeybinds,
}

// ─────────────────────────────────────────────────────────────────────────────
// Meta
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexMeta {
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
}

impl Default for AexMeta {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            author: "arch-explorer".into(),
            version: "1.0.0".into(),
            description: "Built-in dark theme".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Colors — her şey hex string, Color32'ye bridge egui_bridge.rs'de
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexColors {
    // Backgrounds
    pub bg_base:     String,   // en derin arka plan
    pub bg_mantle:   String,   // panel arka planı
    pub bg_crust:    String,   // en koyu
    pub bg_surface0: String,   // kart/item
    pub bg_surface1: String,   // hover
    pub bg_surface2: String,   // selected/active

    // Accent
    pub accent:      String,
    pub accent_dim:  String,
    pub accent_glow: String,   // accent + alpha

    // Text
    pub text_primary:   String,
    pub text_secondary: String,
    pub text_hint:      String,
    pub text_disabled:  String,

    // Semantic
    pub success: String,
    pub warning: String,
    pub danger:  String,
    pub info:    String,

    // Borders
    pub border:       String,
    pub border_focus: String,
}

impl Default for AexColors {
    fn default() -> Self {
        // Arch Explorer Dark / Rose Dark (matches screenshot design)
        Self {
            bg_base:     "#0f0f17".into(),
            bg_mantle:   "#13131e".into(),
            bg_crust:    "#0a0a10".into(),
            bg_surface0: "#1b1b2a".into(),
            bg_surface1: "#27273d".into(),
            bg_surface2: "#4e2738".into(),
            accent:      "#ff6b9d".into(),
            accent_dim:  "#cc4f7c".into(),
            accent_glow: "#ff6b9d30".into(),
            text_primary:   "#f0eeff".into(),
            text_secondary: "#9a9ab2".into(),
            text_hint:      "#60607a".into(),
            text_disabled:  "#3a3a52".into(),
            success: "#5aff8e".into(),
            warning: "#ffd166".into(),
            danger:  "#ff5f6d".into(),
            info:    "#8264ff".into(),
            border:       "#232336".into(),
            border_focus: "#ff6b9d".into(),
        }
    }
}

// Color32 accessor'ları — UI kodu için kolaylık
impl AexColors {
    pub fn bg_base(&self)     -> Color32 { hex_to_color32(&self.bg_base) }
    pub fn bg_mantle(&self)   -> Color32 { hex_to_color32(&self.bg_mantle) }
    pub fn bg_crust(&self)    -> Color32 { hex_to_color32(&self.bg_crust) }
    pub fn bg_surface0(&self) -> Color32 { hex_to_color32(&self.bg_surface0) }
    pub fn bg_surface1(&self) -> Color32 { hex_to_color32(&self.bg_surface1) }
    pub fn bg_surface2(&self) -> Color32 { hex_to_color32(&self.bg_surface2) }
    pub fn accent(&self)      -> Color32 { hex_to_color32(&self.accent) }
    pub fn accent_dim(&self)  -> Color32 { hex_to_color32(&self.accent_dim) }
    pub fn accent_glow(&self) -> Color32 { hex_to_color32(&self.accent_glow) }
    pub fn text_primary(&self)   -> Color32 { hex_to_color32(&self.text_primary) }
    pub fn text_secondary(&self) -> Color32 { hex_to_color32(&self.text_secondary) }
    pub fn text_hint(&self)      -> Color32 { hex_to_color32(&self.text_hint) }
    pub fn text_disabled(&self)  -> Color32 { hex_to_color32(&self.text_disabled) }
    pub fn success(&self) -> Color32 { hex_to_color32(&self.success) }
    pub fn warning(&self) -> Color32 { hex_to_color32(&self.warning) }
    pub fn danger(&self)  -> Color32 { hex_to_color32(&self.danger) }
    pub fn info(&self)    -> Color32 { hex_to_color32(&self.info) }
    pub fn border(&self)       -> Color32 { hex_to_color32(&self.border) }
    pub fn border_focus(&self) -> Color32 { hex_to_color32(&self.border_focus) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Typography
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexTypography {
    pub font_ui:       String,
    pub font_mono:     String,
    pub font_fallback: String,
    pub size_xs:   f32,
    pub size_sm:   f32,
    pub size_base: f32,
    pub size_lg:   f32,
    pub size_xl:   f32,
    pub size_2xl:  f32,
}

impl Default for AexTypography {
    fn default() -> Self {
        Self {
            font_ui:       "Inter".into(),
            font_mono:     "JetBrainsMono Nerd Font".into(),
            font_fallback: "sans-serif".into(),
            size_xs:   11.5,
            size_sm:   12.5,
            size_base: 14.0,
            size_lg:   16.0,
            size_xl:   19.0,
            size_2xl:  25.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spacing
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexSpacing {
    pub xs:  f32,   // 2
    pub sm:  f32,   // 4
    pub md:  f32,   // 8
    pub lg:  f32,   // 12
    pub xl:  f32,   // 16
    pub xxl: f32,   // 24
}

impl Default for AexSpacing {
    fn default() -> Self {
        Self { xs: 2.0, sm: 4.0, md: 8.0, lg: 12.0, xl: 16.0, xxl: 24.0 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shape
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexShape {
    pub radius_none: f32,
    pub radius_sm:   f32,
    pub radius_md:   f32,
    pub radius_lg:   f32,
    pub radius_xl:   f32,
    pub radius_full: f32,
}

impl Default for AexShape {
    fn default() -> Self {
        Self {
            radius_none: 0.0,
            radius_sm:   4.0,
            radius_md:   8.0,
            radius_lg:   12.0,
            radius_xl:   16.0,
            radius_full: 9999.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Effects
// ─────────────────────────────────────────────────────────────────────────────

fn default_sidebar_opacity() -> f32 { 0.75 }
fn default_panel_opacity() -> f32 { 0.90 }
fn default_card_opacity() -> f32 { 0.85 }
fn default_toolbar_opacity() -> f32 { 0.80 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexEffects {
    pub window_opacity: f32,   // 0.0–1.0
    #[serde(default = "default_sidebar_opacity")]
    pub sidebar_opacity: f32,  // 0.0–1.0
    #[serde(default = "default_panel_opacity")]
    pub panel_opacity:   f32,  // 0.0–1.0
    #[serde(default = "default_card_opacity")]
    pub card_opacity:    f32,  // 0.0–1.0
    #[serde(default = "default_toolbar_opacity")]
    pub toolbar_opacity: f32,  // 0.0–1.0
    pub blur_radius:    u32,   // Hyprland blur hint
    pub glass_panels:   bool,  // panel'ler semi-transparent
    pub shadow_enabled: bool,
    pub shadow_offset_x: f32,
    pub shadow_offset_y: f32,
    pub shadow_blur:    f32,
    pub shadow_color:   String,
}

impl Default for AexEffects {
    fn default() -> Self {
        Self {
            window_opacity:  0.85,
            sidebar_opacity: 0.75,
            panel_opacity:   0.90,
            card_opacity:    0.85,
            toolbar_opacity: 0.80,
            blur_radius:     16,
            glass_panels:    true,
            shadow_enabled:  true,
            shadow_offset_x: 0.0,
            shadow_offset_y: 4.0,
            shadow_blur:     16.0,
            shadow_color:    "#00000060".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout
// ─────────────────────────────────────────────────────────────────────────────

fn default_show_breadcrumb() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexLayout {
    pub sidebar_width:    f32,
    pub sidebar_visible:  bool,
    pub preview_width:    f32,
    pub preview_visible:  bool,
    pub toolbar_height:   f32,
    pub statusbar_height: f32,
    pub item_height:      f32,   // list view row
    pub grid_item_size:   f32,   // grid view item
    pub grid_gap:         f32,
    #[serde(default = "default_show_breadcrumb")]
    pub show_breadcrumb:  bool,
}

impl Default for AexLayout {
    fn default() -> Self {
        Self {
            sidebar_width:    220.0,
            sidebar_visible:  true,
            preview_width:    280.0,
            preview_visible:  true,
            toolbar_height:   44.0,
            statusbar_height: 26.0,
            item_height:      32.0,
            grid_item_size:   100.0,
            grid_gap:         10.0,
            show_breadcrumb:  true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Icons
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IconStyle {
    Nerd,    // Nerd Font glyphs — Arch kullanıcısı zaten var
    Emoji,   // sistem emoji
    Minimal, // ASCII
}

impl Default for IconStyle {
    fn default() -> Self { Self::Nerd }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexIcons {
    pub style:   IconStyle,
    pub size_sm: f32,
    pub size_md: f32,
    pub size_lg: f32,
}

impl Default for AexIcons {
    fn default() -> Self {
        Self { style: IconStyle::Nerd, size_sm: 14.0, size_md: 16.0, size_lg: 20.0 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Animations
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EasingStyle {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Spring,
}

impl Default for EasingStyle {
    fn default() -> Self { Self::EaseOut }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexAnimations {
    pub enabled:          bool,
    pub fps_target:       u32,
    pub duration_fast:    u32,   // ms
    pub duration_mid:     u32,
    pub duration_slow:    u32,
    pub easing:           EasingStyle,
    pub spring_stiffness: f32,
    pub spring_damping:   f32,
}

impl Default for AexAnimations {
    fn default() -> Self {
        Self {
            enabled:          true,
            fps_target:       60,
            duration_fast:    120,
            duration_mid:     200,
            duration_slow:    350,
            easing:           EasingStyle::EaseOut,
            spring_stiffness: 200.0,
            spring_damping:   20.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sidebar
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SeparatorStyle {
    Line,
    Gap,
    None,
}

impl Default for SeparatorStyle {
    fn default() -> Self { Self::Line }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexSidebar {
    pub show_drives:    bool,
    pub show_bookmarks: bool,
    pub show_tags:      bool,
    pub separator_style: SeparatorStyle,
}

impl Default for AexSidebar {
    fn default() -> Self {
        Self {
            show_drives:    true,
            show_bookmarks: true,
            show_tags:      false,
            separator_style: SeparatorStyle::Line,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FileGrid
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    Grid,
    List,
    Compact,
    Columns,
}

impl Default for ViewMode {
    fn default() -> Self { Self::Grid }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    Name,
    Size,
    Modified,
    Type,
}

impl Default for SortBy {
    fn default() -> Self { Self::Name }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexFileGrid {
    pub default_view:      ViewMode,
    pub show_hidden:       bool,
    pub sort_by:           SortBy,
    pub sort_asc:          bool,
    pub group_dirs_first:  bool,
    pub show_extensions:   bool,
    pub show_file_size:    bool,
    pub show_modified:     bool,
    pub thumbnail_size:    u32,
    pub thumbnail_enabled: bool,
}

impl Default for AexFileGrid {
    fn default() -> Self {
        Self {
            default_view:      ViewMode::Grid,
            show_hidden:       false,
            sort_by:           SortBy::Name,
            sort_asc:          true,
            group_dirs_first:  true,
            show_extensions:   true,
            show_file_size:    true,
            show_modified:     false,
            thumbnail_size:    64,
            thumbnail_enabled: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Context Menu
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexContextMenu {
    pub show_shortcuts: bool,
    pub show_icons:     bool,
    pub item_height:    f32,
}

impl Default for AexContextMenu {
    fn default() -> Self {
        Self { show_shortcuts: true, show_icons: true, item_height: 28.0 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Keybinds
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AexKeybinds {
    pub back:           String,
    pub forward:        String,
    pub go_up:          String,
    pub go_home:        String,
    pub new_folder:     String,
    pub new_file:       String,
    pub copy:           String,
    pub cut:            String,
    pub paste:          String,
    pub undo:           String,
    pub rename:         String,
    pub delete:         String,
    pub delete_perm:    String,
    pub select_all:     String,
    pub search:         String,
    pub toggle_hidden:  String,
    pub toggle_preview: String,
    pub settings:       String,
    pub refresh:        String,
    pub open_terminal:  String,
}

impl Default for AexKeybinds {
    fn default() -> Self {
        Self {
            back:           "alt+left".into(),
            forward:        "alt+right".into(),
            go_up:          "alt+up".into(),
            go_home:        "alt+home".into(),
            new_folder:     "ctrl+shift+n".into(),
            new_file:       "ctrl+n".into(),
            copy:           "ctrl+c".into(),
            cut:            "ctrl+x".into(),
            paste:          "ctrl+v".into(),
            undo:           "ctrl+z".into(),
            rename:         "f2".into(),
            delete:         "delete".into(),
            delete_perm:    "shift+delete".into(),
            select_all:     "ctrl+a".into(),
            search:         "ctrl+f".into(),
            toggle_hidden:  "ctrl+h".into(),
            toggle_preview: "f3".into(),
            settings:       "ctrl+comma".into(),
            refresh:        "f5".into(),
            open_terminal:  "ctrl+t".into(),
        }
    }
}

// AexTheme için Default impl — tüm sub-struct Default'larını çağırır
impl Default for AexTheme {
    fn default() -> Self {
        Self {
            meta:         AexMeta::default(),
            colors:       AexColors::default(),
            typography:   AexTypography::default(),
            spacing:      AexSpacing::default(),
            shape:        AexShape::default(),
            effects:      AexEffects::default(),
            layout:       AexLayout::default(),
            icons:        AexIcons::default(),
            animations:   AexAnimations::default(),
            sidebar:      AexSidebar::default(),
            file_grid:    AexFileGrid::default(),
            context_menu: AexContextMenu::default(),
            keybinds:     AexKeybinds::default(),
        }
    }
}
