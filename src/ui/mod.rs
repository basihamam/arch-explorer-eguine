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
            "visible"      => "\u{f06e}",
            "hidden"       => "\u{f070}",
            "check" | "success" => "\u{f00c}",
            "ntfs"         => "\u{f17a}", //  FontAwesome Windows icon
            "windows"      => "\u{f17a}",
            "phone"        => "\u{f10b}", //  Mobile phone
            "android"      => "\u{f17b}", //  Android robot
            "exec"         => "\u{f489}", // terminal / executable gear
            "binary"       => "\u{f489}",
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
            _              => "\u{f15b}",
        },
        IconStyle::Emoji => match name {
            "folder"       => "📁",
            "folder_open"  => "📂",
            "image"        => "🖼",
            "video"        => "🎬",
            "audio"        => "🎵",
            "archive"      => "📦",
            "terminal"     => "💻",
            "exec"         => "⚙️",
            "binary"       => "⚙️",
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
            _              => "📄",
        },
        IconStyle::Minimal => match name {
            "folder"      => "[D]",
            "folder_open" => "[O]",
            "exec"        => "[!]",
            "binary"      => "[!]",
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
            _             => "[ ]",
        },
    }
}

// ── File-type icon string (returns String — fine for label/text uses) ─────────
pub fn file_icon(t: &AexTheme, e: &FileEntry) -> &'static str {
    if e.is_dir { return icon(t, "folder"); }
    let ext = std::path::Path::new(&e.name)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();

    // Windows executables & libraries get the dedicated Windows icon (NOT the console icon!)
    if matches!(ext.as_str(), "exe" | "msi" | "dll" | "bat" | "cmd" | "reg" | "scr") {
        return icon(t, "windows");
    }

    // Media
    if matches!(ext.as_str(), "png"|"jpg"|"jpeg"|"gif"|"webp"|"ico"|"bmp"|"svg"|"tiff"|"avif") {
        return icon(t, "image");
    }
    if matches!(ext.as_str(), "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts") {
        return icon(t, "video");
    }
    if matches!(ext.as_str(), "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma") {
        return icon(t, "audio");
    }
    if matches!(ext.as_str(), "zip"|"tar"|"gz"|"bz2"|"xz"|"zst"|"7z"|"rar"|"tgz"|"tbz2") {
        return icon(t, "archive");
    }
    if matches!(ext.as_str(), "iso"|"img"|"dmg"|"qcow2"|"vdi") {
        return icon(t, "disk");
    }

    // Documents
    if ext == "pdf" { return icon(t, "pdf"); }
    if matches!(ext.as_str(), "doc"|"docx"|"odt"|"rtf") { return icon(t, "doc"); }
    if matches!(ext.as_str(), "xls"|"xlsx"|"ods"|"csv"|"tsv") { return icon(t, "sheet"); }
    if matches!(ext.as_str(), "ppt"|"pptx"|"odp") { return icon(t, "slide"); }
    if matches!(ext.as_str(), "txt"|"log"|"nfo"|"md"|"markdown") { return icon(t, "text"); }

    // Code & configs
    if ext == "rs" { return icon(t, "rust"); }
    if ext == "py" { return icon(t, "python"); }
    if matches!(ext.as_str(), "js"|"mjs"|"cjs") { return icon(t, "js"); }
    if matches!(ext.as_str(), "ts"|"tsx") { return icon(t, "ts"); }
    if matches!(ext.as_str(), "html"|"htm") { return icon(t, "html"); }
    if matches!(ext.as_str(), "css"|"scss"|"sass"|"less") { return icon(t, "css"); }
    if ext == "json" { return icon(t, "json"); }
    if ext == "c" || ext == "h" { return icon(t, "c"); }
    if matches!(ext.as_str(), "cpp"|"hpp"|"cc"|"cxx") { return icon(t, "cpp"); }
    if matches!(ext.as_str(), "db"|"sqlite"|"sqlite3"|"sql") { return icon(t, "database"); }
    if matches!(ext.as_str(), "ttf"|"otf"|"woff"|"woff2") { return icon(t, "font"); }
    if matches!(ext.as_str(), "toml"|"yaml"|"yml"|"ini"|"conf"|"cfg") { return icon(t, "settings"); }
    if matches!(ext.as_str(), "sh"|"bash"|"zsh"|"fish") { return icon(t, "terminal"); }

    // Binary executables without extension
    if e.is_executable { return icon(t, "exec"); }

    icon(t, "file")
}

// ── Icon color by file type ───────────────────────────────────────────────────
pub fn icon_color_for_entry(t: &AexTheme, e: &FileEntry) -> egui::Color32 {
    use crate::theme::types::hex_to_color32;
    let c = &t.colors;
    if e.is_dir { return hex_to_color32(&c.accent); }
    let ext = std::path::Path::new(&e.name)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();

    if matches!(ext.as_str(), "exe" | "msi" | "dll" | "bat" | "cmd" | "reg") {
        return egui::Color32::from_rgb(0, 164, 239); // Microsoft Blue
    }
    match ext.as_str() {
        "png"|"jpg"|"jpeg"|"gif"|"webp"|"ico"|"bmp"|"svg" => hex_to_color32(&c.success),
        "mp4"|"mkv"|"avi"|"mov"|"webm"                    => hex_to_color32(&c.warning),
        "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma" => hex_to_color32(&c.info),
        "zip"|"tar"|"gz"|"bz2"|"xz"|"7z"|"rar"           => hex_to_color32(&c.accent_dim),
        "pdf"                                              => hex_to_color32(&c.danger),
        "rs"                                               => egui::Color32::from_rgb(222, 114, 60),
        "py"                                               => egui::Color32::from_rgb(75, 139, 190),
        "js"|"ts"                                          => egui::Color32::from_rgb(240, 219, 79),
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
