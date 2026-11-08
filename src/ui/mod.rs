// src/ui/mod.rs
pub mod sidebar;
pub mod toolbar;
pub mod file_grid;
pub mod context_menu;
pub mod tab_bar;
pub mod titlebar;
pub mod statusbar;
pub mod preview_panel;
pub mod mount_panel;
pub mod settings;
pub mod bulk_rename;
pub mod notifications;
pub mod devices_view;
pub mod commands_panel;
pub mod properties;
pub mod network;
pub mod open_with;
pub mod terminal_panel;

use crate::theme::types::{AexTheme, IconStyle};
use crate::backend::fs_ops::FileEntry;

// ── Icon helper — returns &'static str so callers taking &str are happy ──────
pub fn icon(t: &AexTheme, name: &str) -> &'static str {
    match &t.icons.style {
        IconStyle::Nerd => match name {
            "folder"       => "\u{f07b}",
            "folder_open"  => "\u{f07c}",
            "file"         => "\u{f15b}",
            "image"        => "\u{f1c5}",
            "video"        => "\u{f03d}",
            "audio"        => "\u{f001}",
            "archive"      => "\u{f1c6}",
            "code"         => "\u{f121}",
            "pdf"          => "\u{f1c1}",
            "text"         => "\u{f15c}",
            "terminal"     => "\u{f120}",
            "copy"         => "\u{f0c5}",
            "cut"          => "\u{f0c4}",
            "paste"        => "\u{f0ea}",
            "rename"       => "\u{f246}",
            "delete"       => "\u{f1f8}",
            "trash"        => "\u{f1f8}",
            "error"        => "\u{f071}",
            "star"         => "\u{f005}",
            "info"         => "\u{f05a}",
            "close"        => "\u{f00d}",
            "new_folder"   => "\u{f65d}",
            "new_file"     => "\u{f15b}",
            "home"         => "\u{f015}",
            "disk"         => "\u{f0a0}",
            "drive"        => "\u{f0a0}",
            "link"         => "\u{f0c1}",
            "symlink"      => "\u{f0c1}",
            "open_with"    => "\u{f35d}",
            "settings"     => "\u{f013}",
            "search"       => "\u{f002}",
            "refresh"      => "\u{f021}",
            "back"         => "\u{f060}",
            "forward"      => "\u{f061}",
            "up"           => "\u{f062}",
            "preview"      => "\u{f15c}",
            "network"      => "\u{f6ff}",
            "plugin"       => "\u{f12e}",
            "mount"        => "\u{f0a0}",
            "list"         => "\u{f03a}",
            "grid"         => "\u{f00a}",
            "compact"      => "\u{f0db}",
            "more" | "dots" => "\u{f142}",
            "visible"      => "\u{f06e}",
            "hidden"       => "\u{f070}",
            "check" | "success" => "\u{f00c}",
            "ntfs"         => "\u{f17a}", //  FontAwesome Windows icon
            "windows"      => "\u{f17a}",
            "phone"        => "\u{f10b}", //  Mobile phone
            "android"      => "\u{f17b}", //  Android robot
            "exec"         => "\u{f489}", // terminal / executable gear
            "binary"       => "\u{f489}",
            "exe"          => "\u{f013}", // dedicated executable gear icon
            "dll"          => "\u{f085}", // cogs / library icon
            "msi"          => "\u{f1b2}", // package / installer box
            "bat" | "cmd"  => "\u{f120}", // batch / terminal script
            "reg"          => "\u{f084}", // registry key icon
            "doc"          => "\u{f1c2}",
            "sheet"        => "\u{f1c3}",
            "slide"        => "\u{f1c4}",
            "rust"         => "\u{e7a8}",
            "python"       => "\u{e73c}",
            "js"           => "\u{e74e}",
            "ts"           => "\u{e628}",
            "html"         => "\u{e736}",
            "css"          => "\u{e749}",
            "json"         => "\u{e60b}",
            "c"            => "\u{e61e}",
            "cpp"          => "\u{e61d}",
            "database"     => "\u{f1c0}",
            "font"         => "\u{f031}",
            _              => "\u{f15b}", // fallback dog-eared paper icon 
        },
        IconStyle::Emoji => match name {
            "folder"       => "📁",
            "folder_open"  => "📂",
            "file"         => "📄", // dog-eared paper
            "image"        => "🖼",
            "video"        => "🎬",
            "audio"        => "🎵",
            "archive"      => "📦",
            "terminal"     => "💻",
            "exec"         => "⚙️",
            "binary"       => "⚙️",
            "exe"          => "⚙️",
            "dll"          => "🧩",
            "msi"          => "📦",
            "bat" | "cmd"  => "📜",
            "reg"          => "🔑",
            "delete"       => "🗑",
            "trash"        => "🗑",
            "star"         => "⭐",
            "info"         => "ℹ",
            "close"        => "✕",
            "settings"     => "⚙",
            "search"       => "🔍",
            "home"         => "🏠",
            "drive"        => "💾",
            "ntfs"         => "⊞", // Windows window symbol
            "windows"      => "⊞",
            "phone"        => "📱",
            "android"      => "🤖",
            "copy"         => "📋",
            "rename"       => "✏",
            "check" | "success" => "✓",
            "compact"      => "📑",
            "more" | "dots" => "⋮",
            _              => "📄", // dog-eared paper
        },
        IconStyle::Minimal => match name {
            "compact"     => "[C]",
            "more" | "dots" => "[:]",
            "folder"      => "[D]",
            "folder_open" => "[O]",
            "file"        => "[F]",
            "exec"        => "[!]",
            "binary"      => "[!]",
            "exe"         => "[X]",
            "dll"         => "[L]",
            "msi"         => "[P]",
            "bat" | "cmd" => "[S]",
            "reg"         => "[K]",
            "delete"      => "[X]",
            "trash"       => "[X]",
            "copy"        => "[C]",
            "cut"         => "[T]",
            "paste"       => "[P]",
            "close"       => "[X]",
            "search"      => "[?]",
            "back"        => "[<]",
            "forward"     => "[>]",
            "up"          => "[^]",
            "ntfs"        => "[W]",
            "windows"     => "[W]",
            "check" | "success" => "[V]",
            _             => "[F]", // fallback dog-eared paper [F]
        },
    }
}

// ── File-type icon string (returns &'static str) ──────────────────────────────
pub fn file_icon(t: &AexTheme, e: &FileEntry) -> &'static str {
    if e.is_dir { return icon(t, "folder"); }
    let ext = std::path::Path::new(&e.name)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    let ext_str = if ext.is_empty() { e.extension.to_lowercase() } else { ext };

    // Specific Windows / executable types (distinct icons!)
    if ext_str == "exe" || ext_str == "scr" {
        return icon(t, "exe");
    }
    if ext_str == "dll" {
        return icon(t, "dll");
    }
    if ext_str == "msi" {
        return icon(t, "msi");
    }
    if ext_str == "bat" || ext_str == "cmd" {
        return icon(t, "bat");
    }
    if ext_str == "reg" {
        return icon(t, "reg");
    }

    // Media
    if matches!(ext_str.as_str(), "png"|"jpg"|"jpeg"|"gif"|"webp"|"ico"|"bmp"|"svg"|"tiff"|"avif") {
        return icon(t, "image");
    }
    if matches!(ext_str.as_str(), "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts") {
        return icon(t, "video");
    }
    if matches!(ext_str.as_str(), "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma") {
        return icon(t, "audio");
    }
    if matches!(ext_str.as_str(), "zip"|"tar"|"gz"|"bz2"|"xz"|"zst"|"7z"|"rar"|"tgz"|"tbz2") {
        return icon(t, "archive");
    }
    if matches!(ext_str.as_str(), "iso"|"img"|"dmg"|"qcow2"|"vdi") {
        return icon(t, "disk");
    }

    // Documents
    if ext_str == "pdf" { return icon(t, "pdf"); }
    if matches!(ext_str.as_str(), "doc"|"docx"|"odt"|"rtf") { return icon(t, "doc"); }
    if matches!(ext_str.as_str(), "xls"|"xlsx"|"ods"|"csv"|"tsv") { return icon(t, "sheet"); }
    if matches!(ext_str.as_str(), "ppt"|"pptx"|"odp") { return icon(t, "slide"); }
    if matches!(ext_str.as_str(), "txt"|"log"|"nfo"|"md"|"markdown") { return icon(t, "text"); }

    // Code & configs
    if ext_str == "rs" { return icon(t, "rust"); }
    if ext_str == "py" { return icon(t, "python"); }
    if matches!(ext_str.as_str(), "js"|"mjs"|"cjs") { return icon(t, "js"); }
    if matches!(ext_str.as_str(), "ts"|"tsx") { return icon(t, "ts"); }
    if matches!(ext_str.as_str(), "html"|"htm") { return icon(t, "html"); }
    if matches!(ext_str.as_str(), "css"|"scss"|"sass"|"less") { return icon(t, "css"); }
    if ext_str == "json" { return icon(t, "json"); }
    if ext_str == "c" || ext_str == "h" { return icon(t, "c"); }
    if matches!(ext_str.as_str(), "cpp"|"hpp"|"cc"|"cxx") { return icon(t, "cpp"); }
    if matches!(ext_str.as_str(), "db"|"sqlite"|"sqlite3"|"sql") { return icon(t, "database"); }
    if matches!(ext_str.as_str(), "ttf"|"otf"|"woff"|"woff2") { return icon(t, "font"); }
    if matches!(ext_str.as_str(), "toml"|"yaml"|"yml"|"ini"|"conf"|"cfg") { return icon(t, "settings"); }
    if matches!(ext_str.as_str(), "sh"|"bash"|"zsh"|"fish") { return icon(t, "terminal"); }

    // Binary executables without extension
    if e.is_executable { return icon(t, "exec"); }

    // Unregistered / unknown extensions: default dog-eared paper icon!
    icon(t, "file")
}

// ── Icon color by file type ───────────────────────────────────────────────────
pub fn icon_color_for_entry(t: &AexTheme, e: &FileEntry) -> egui::Color32 {
    use crate::theme::types::hex_to_color32;
    let c = &t.colors;
    if e.is_dir { return hex_to_color32(&c.accent); }
    let ext = std::path::Path::new(&e.name)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    let ext_str = if ext.is_empty() { e.extension.to_lowercase() } else { ext };

    match ext_str.as_str() {
        "exe" | "scr"                                      => egui::Color32::from_rgb(0, 180, 240),   // Cyan / Blue
        "dll"                                              => egui::Color32::from_rgb(180, 100, 240), // Purple / Magenta
        "msi"                                              => egui::Color32::from_rgb(245, 140, 40),  // Orange / Amber
        "bat" | "cmd"                                      => egui::Color32::from_rgb(80, 200, 120),  // Green script
        "reg"                                              => egui::Color32::from_rgb(40, 200, 190),  // Teal / Yellow
        "png"|"jpg"|"jpeg"|"gif"|"webp"|"ico"|"bmp"|"svg"  => hex_to_color32(&c.success),
        "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts" => hex_to_color32(&c.warning),
        "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma"  => hex_to_color32(&c.info),
        "zip"|"tar"|"gz"|"bz2"|"xz"|"zst"|"7z"|"rar"      => hex_to_color32(&c.accent_dim),
        "pdf"                                              => hex_to_color32(&c.danger),
        "rs"                                               => egui::Color32::from_rgb(222, 114, 60),
        "py"                                               => egui::Color32::from_rgb(75, 139, 190),
        "js"|"ts"                                          => egui::Color32::from_rgb(240, 219, 79),
        "sh"|"bash"|"zsh"|"fish"                           => egui::Color32::from_rgb(80, 200, 120),
        _ if e.is_executable                               => hex_to_color32(&c.accent),
        _                                                  => hex_to_color32(&c.text_secondary),
    }
}

// ── Human-readable size ───────────────────────────────────────────────────────
pub fn human_size(bytes: u64) -> String {
    const K: u64 = 1024;
    if bytes < K            { format!("{} B",     bytes) }
    else if bytes < K*K     { format!("{:.1} KB", bytes as f64 / K as f64) }
    else if bytes < K*K*K   { format!("{:.1} MB", bytes as f64 / (K*K) as f64) }
    else if bytes < K*K*K*K { format!("{:.2} GB", bytes as f64 / (K*K*K) as f64) }
    else                    { format!("{:.2} TB", bytes as f64 / (K*K*K*K) as f64) }
}
