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
    if e.is_executable { return icon(t, "exec"); }
    let ext = std::path::Path::new(&e.name)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png"|"jpg"|"jpeg"|"gif"|"webp"|"ico"|"bmp"|"svg" => icon(t, "image"),
        "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"              => icon(t, "video"),
        "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma" => icon(t, "audio"),
        "zip"|"tar"|"gz"|"bz2"|"xz"|"zst"|"7z"|"rar"     => icon(t, "archive"),
        "pdf"                                              => icon(t, "pdf"),
        "rs"|"py"|"js"|"ts"|"go"|"c"|"cpp"|"h"|"lua"
        |"rb"|"sh"|"toml"|"json"|"yaml"|"yml"
        |"svelte"|"html"|"css"|"md"                       => icon(t, "code"),
        _                                                  => icon(t, "file"),
    }
}

// ── Icon color by file type ───────────────────────────────────────────────────
pub fn icon_color_for_entry(t: &AexTheme, e: &FileEntry) -> egui::Color32 {
    use crate::theme::types::hex_to_color32;
    let c = &t.colors;
    if e.is_dir { return hex_to_color32(&c.accent); }
    if e.is_executable { return hex_to_color32(&c.accent); }
    let ext = std::path::Path::new(&e.name)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png"|"jpg"|"jpeg"|"gif"|"webp"|"ico"|"bmp"|"svg" => hex_to_color32(&c.success),
        "mp4"|"mkv"|"avi"|"mov"|"webm"                    => hex_to_color32(&c.warning),
        "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma" => hex_to_color32(&c.info),
        "zip"|"tar"|"gz"|"bz2"|"xz"|"7z"|"rar"           => hex_to_color32(&c.accent_dim),
        "pdf"                                              => hex_to_color32(&c.danger),
        "rs"|"py"|"js"|"ts"|"go"                          => hex_to_color32(&c.info),
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
