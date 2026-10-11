// ─────────────────────────────────────────────────────────────────────────────
// src/state/mod.rs
// AppState — Arch Explorer'ın merkezi state makinesi
// Explorer.js'in Rust karşılığı. Her şey buradan geçer.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashSet, VecDeque};
use std::time::Instant;
use crate::backend::fs_ops::{FileEntry, DirectoryListing, Bookmark, DriveInfo};
use crate::backend::mounts::BlockDevice;

// ─── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
    Compact,
    Columns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Modified,
    Permissions,
    Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivePanel {
    None,
    Preview,
    Mount,
    Settings,
    OpenWith(String),
    Properties(String),
    Compress,
    Commands,
    Network,
    DevicesView,
    BulkRename,
    Checksum(String, String, String), // path, md5, sha256
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotifyKind {
    Success,
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PreviewBehaviorMode {
    AlwaysOpen,
    AutoOnSelect,
    Custom,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewSettings {
    pub mode: PreviewBehaviorMode,
    pub custom_image: bool,
    pub custom_video: bool,
    pub custom_audio: bool,
    pub custom_text: bool,
    pub custom_doc: bool,
    pub custom_dir: bool,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            mode: PreviewBehaviorMode::AutoOnSelect,
            custom_image: true,
            custom_video: true,
            custom_audio: true,
            custom_text: true,
            custom_doc: true,
            custom_dir: false,
        }
    }
}

pub fn preview_settings_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("preview_settings.json")
}

pub fn load_preview_settings() -> PreviewSettings {
    let p = preview_settings_path();
    if !p.exists() {
        return PreviewSettings::default();
    }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_preview_settings_to_disk(cfg: &PreviewSettings) {
    let p = preview_settings_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(p, json);
    }
}

// ── Root Settings ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RootSettings {
    pub always_root: bool,
}

impl Default for RootSettings {
    fn default() -> Self {
        Self { always_root: false }
    }
}

pub fn root_settings_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("root_settings.json")
}

pub fn load_root_settings() -> RootSettings {
    let p = root_settings_path();
    if !p.exists() {
        return RootSettings::default();
    }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_root_settings_to_disk(cfg: &RootSettings) {
    let p = root_settings_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(p, json);
    }
}

// ── List View Columns Settings ────────────────────────────────────────────────

fn default_col_order() -> Vec<String> {
    vec!["size".to_string(), "perm".to_string(), "modified".to_string()]
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListViewColumns {
    pub size_w: f32,
    pub perm_w: f32,
    pub modified_w: f32,
    #[serde(default = "default_col_order")]
    pub order: Vec<String>,
}

impl Default for ListViewColumns {
    fn default() -> Self {
        Self {
            size_w: 120.0,
            perm_w: 110.0,
            modified_w: 150.0,
            order: default_col_order(),
        }
    }
}

impl ListViewColumns {
    pub fn move_col_left(&mut self, col: &str) {
        if self.order.is_empty() { self.order = default_col_order(); }
        if let Some(idx) = self.order.iter().position(|c| c == col) {
            if idx > 0 {
                self.order.swap(idx, idx - 1);
            }
        }
    }

    pub fn move_col_right(&mut self, col: &str) {
        if self.order.is_empty() { self.order = default_col_order(); }
        if let Some(idx) = self.order.iter().position(|c| c == col) {
            if idx + 1 < self.order.len() {
                self.order.swap(idx, idx + 1);
            }
        }
    }
}

pub fn list_columns_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("list_columns.json")
}

pub fn load_list_columns() -> ListViewColumns {
    let p = list_columns_path();
    if !p.exists() {
        return ListViewColumns::default();
    }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_list_columns_to_disk(cfg: &ListViewColumns) {
    let p = list_columns_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(p, json);
    }
}

// ── Performance & Memory Optimization Settings ───────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerfSettings {
    /// Limit background thumbnail generation in huge folders (e.g. max 500 active textures)
    pub max_cached_textures: usize,
    /// Fast directory loading: skip detailed mime_guess in huge folders (> 3,000 files)
    pub fast_directory_load: bool,
    /// Low memory mode: unload offscreen textures immediately
    pub low_memory_mode: bool,
}

impl Default for PerfSettings {
    fn default() -> Self {
        Self {
            max_cached_textures: 300,
            fast_directory_load: true,
            low_memory_mode: false,
        }
    }
}

pub fn perf_settings_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("perf_settings.json")
}

pub fn load_perf_settings() -> PerfSettings {
    let p = perf_settings_path();
    if !p.exists() {
        return PerfSettings::default();
    }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_perf_settings_to_disk(cfg: &PerfSettings) {
    let p = perf_settings_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(p, json);
    }
}

#[derive(Debug, Clone)]
pub struct RootAuthRequest {
    pub title: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub reload: bool,
    pub auth_type: Option<String>,
}

// ─── ContextMenuState ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
    pub target: Option<FileEntry>,
}

// ─── NavigationState ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct NavigationState {
    pub current_path: String,
    pub history: VecDeque<String>,
    pub history_idx: usize,
}

impl NavigationState {
    fn new() -> Self {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());

        let mut history = VecDeque::new();
        history.push_back(home.clone());

        Self {
            current_path: home,
            history,
            history_idx: 0,
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_idx > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_idx + 1 < self.history.len()
    }

    pub fn can_go_up(&self) -> bool {
        self.current_path != "/"
    }

    /// Yeni path'e git — history'ye ekle, ileri geçmişi sil
    pub fn push(&mut self, path: String) {
        // İleri geçmişi sil
        while self.history.len() > self.history_idx + 1 {
            self.history.pop_back();
        }
        self.history.push_back(path.clone());
        self.history_idx = self.history.len() - 1;
        self.current_path = path;

        // History çok büyümesin
        if self.history.len() > 100 {
            self.history.pop_front();
            self.history_idx = self.history_idx.saturating_sub(1);
        }
    }

    pub fn go_back(&mut self) -> Option<String> {
        if self.can_go_back() {
            self.history_idx -= 1;
            let path = self.history[self.history_idx].clone();
            self.current_path = path.clone();
            Some(path)
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<String> {
        if self.can_go_forward() {
            self.history_idx += 1;
            let path = self.history[self.history_idx].clone();
            self.current_path = path.clone();
            Some(path)
        } else {
            None
        }
    }

    pub fn go_up(&mut self) -> Option<String> {
        let parent = std::path::Path::new(&self.current_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string());

        if let Some(ref p) = parent {
            self.push(p.clone());
        }
        parent
    }
}

// ─── DirectoryState ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct DirectoryState {
    pub entries: std::sync::Arc<Vec<FileEntry>>,
    pub is_loading: bool,
    pub load_error: Option<String>,
    pub show_hidden: bool,
    pub sort_by: SortBy,
    pub sort_asc: bool,
    pub search_cache: std::sync::Arc<Vec<FileEntry>>,
    pub last_query: String,
}

impl DirectoryState {
    fn new() -> Self {
        Self {
            entries: std::sync::Arc::new(Vec::new()),
            is_loading: false,
            load_error: None,
            show_hidden: false,
            sort_by: SortBy::Name,
            sort_asc: true,
            search_cache: std::sync::Arc::new(Vec::new()),
            last_query: String::new(),
        }
    }

    /// Zero-allocation cached visible entries
    pub fn visible_entries(&mut self, query: &str) -> std::sync::Arc<Vec<FileEntry>> {
        if query.is_empty() {
            self.entries.clone()
        } else {
            if self.last_query != query {
                let filtered: Vec<FileEntry> = self.entries.iter()
                    .filter(|e| (self.show_hidden || !e.hidden) && e.name.to_lowercase().contains(&query.to_lowercase()))
                    .cloned()
                    .collect();
                self.search_cache = std::sync::Arc::new(filtered);
                self.last_query = query.to_string();
            }
            self.search_cache.clone()
        }
    }

    /// Show_hidden filtreleyip query'ye göre ara
    pub fn filtered<'a>(&'a self, query: &'a str) -> impl Iterator<Item = &'a FileEntry> {
        self.entries.iter()
            .filter(move |e| self.show_hidden || !e.hidden)
            .filter(move |e| {
                query.is_empty()
                    || e.name.to_lowercase().contains(&query.to_lowercase())
            })
    }

    pub fn sort_slice(entries: &mut [FileEntry], sort_by: SortBy, sort_asc: bool) {
        match sort_by {
            SortBy::Name => {
                entries.sort_by(|a, b| {
                    match (a.is_dir, b.is_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    }
                });
            }
            SortBy::Size => {
                entries.sort_by(|a, b| {
                    match (a.is_dir, b.is_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.size.cmp(&b.size),
                    }
                });
            }
            SortBy::Modified => {
                entries.sort_by(|a, b| a.modified.cmp(&b.modified));
            }
            SortBy::Permissions => {
                entries.sort_by(|a, b| a.permissions.cmp(&b.permissions));
            }
            SortBy::Type => {
                entries.sort_by(|a, b| {
                    match (a.is_dir, b.is_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.extension.to_lowercase().cmp(&b.extension.to_lowercase()),
                    }
                });
            }
        }
        if !sort_asc {
            entries.reverse();
        }
    }

    /// Entries'i sort_by + sort_asc'e göre sırala
    pub fn sort(&mut self) {
        let mut entries = (*self.entries).clone();
        Self::sort_slice(&mut entries, self.sort_by, self.sort_asc);
        self.entries = std::sync::Arc::new(entries);
        self.last_query.clear();
    }

    pub fn load_listing(&mut self, listing: DirectoryListing) {
        let mut entries = listing.entries;
        Self::sort_slice(&mut entries, self.sort_by, self.sort_asc);
        self.entries = std::sync::Arc::new(entries);
        self.is_loading = false;
        self.load_error = None;
        self.last_query.clear();
    }
}

// ─── SidebarState ────────────────────────────────────────────────────────────

pub struct SidebarState {
    pub bookmarks: Vec<Bookmark>,
    pub drives: Vec<DriveInfo>,
    pub block_devices: Vec<BlockDevice>,
    pub adb_devices: Vec<crate::backend::adb::AdbDevice>,
    pub drag_over_bookmark: Option<String>,
    pub devices_expanded: bool,
}

impl SidebarState {
    fn new() -> Self {
        Self {
            bookmarks: crate::backend::fs_ops::get_bookmarks(),
            drives: Vec::new(),
            block_devices: Vec::new(),
            adb_devices: Vec::new(),
            drag_over_bookmark: None,
            devices_expanded: false,
        }
    }
}

// ─── PanelState ──────────────────────────────────────────────────────────────

pub struct PanelState {
    pub active: ActivePanel,
    pub context_menu: Option<ContextMenuState>,
}

impl PanelState {
    fn new() -> Self {
        Self {
            active: ActivePanel::None,
            context_menu: None,
        }
    }

    pub fn close(&mut self) {
        self.active = ActivePanel::None;
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    pub fn open_settings(&mut self) {
        self.active = ActivePanel::Settings;
    }

    pub fn open_preview(&mut self) {
        self.active = ActivePanel::Preview;
    }

    pub fn open_mount(&mut self) {
        self.active = ActivePanel::Mount;
    }

    pub fn open_open_with(&mut self, path: String) {
        self.active = ActivePanel::OpenWith(path);
    }

    pub fn open_bulk_rename(&mut self) {
        self.active = ActivePanel::BulkRename;
    }

    pub fn open_commands(&mut self) {
        self.active = ActivePanel::Commands;
    }

    pub fn open_network(&mut self) {
        self.active = ActivePanel::Network;
    }

    pub fn open_devices_view(&mut self) {
        self.active = ActivePanel::DevicesView;
    }

    pub fn open_compress(&mut self) {
        self.active = ActivePanel::Compress;
    }

    pub fn open_checksum(&mut self, path: String, md5: String, sha256: String) {
        self.active = ActivePanel::Checksum(path, md5, sha256);
    }

    pub fn open_properties(&mut self, path: String) {
        self.active = ActivePanel::Properties(path);
    }
}

// ─── SelectionState ──────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct SelectionState {
    pub selected: HashSet<String>,
    pub ranges: Vec<(usize, usize)>,
    pub last_clicked: Option<String>,
    pub anchor: Option<String>,
}

impl SelectionState {
    fn new() -> Self {
        Self {
            selected: HashSet::new(),
            ranges: Vec::new(),
            last_clicked: None,
            anchor: None,
        }
    }

    pub fn is_selected(&self, path: &str) -> bool {
        self.selected.contains(path)
    }

    pub fn is_selected_at(&self, path: &str, idx: usize) -> bool {
        if self.selected.contains(path) {
            return true;
        }
        for &(lo, hi) in &self.ranges {
            if idx >= lo && idx <= hi {
                return true;
            }
        }
        false
    }

    pub fn toggle(&mut self, path: String) {
        if self.selected.contains(&path) {
            self.selected.remove(&path);
        } else {
            self.selected.insert(path.clone());
        }
        self.last_clicked = Some(path.clone());
        self.anchor = Some(path);
    }

    pub fn select_only(&mut self, path: String) {
        self.selected.clear();
        self.ranges.clear();
        self.selected.insert(path.clone());
        self.last_clicked = Some(path.clone());
        self.anchor = Some(path);
    }

    pub fn select_many(&mut self, paths: &[String]) {
        self.selected.clear();
        self.ranges.clear();
        for p in paths {
            self.selected.insert(p.clone());
        }
        self.last_clicked = paths.last().cloned();
        self.anchor = paths.first().cloned();
    }

    pub fn select_all(&mut self, entries: &[FileEntry]) {
        self.selected.clear();
        if !entries.is_empty() {
            self.ranges = vec![(0, entries.len() - 1)];
        }
    }

    /// Shift+click aralık seçimi (keep_existing = true ise önceki seçimi koruyup aralığı ekler)
    pub fn select_range(&mut self, entries: &[FileEntry], from: &str, to: &str, keep_existing: bool) {
        let idx_from = entries.iter().position(|e| e.path == from);
        let idx_to   = entries.iter().position(|e| e.path == to);

        if let (Some(a), Some(b)) = (idx_from, idx_to) {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            if !keep_existing {
                self.selected.clear();
                self.ranges.clear();
            }
            if hi.saturating_sub(lo) > 200 {
                // High-performance Range storage: yüz binlerce dosyada O(1) anında seçim
                self.ranges.push((lo, hi));
            } else {
                for e in &entries[lo..=hi] {
                    self.selected.insert(e.path.clone());
                }
            }
        } else if let Some(b) = idx_to {
            // Anchor görünür listede bulunamazsa sadece hedefi seç/ekle
            if !keep_existing {
                self.selected.clear();
                self.ranges.clear();
            }
            self.selected.insert(entries[b].path.clone());
        }
        self.last_clicked = Some(to.to_string());
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.ranges.clear();
        self.last_clicked = None;
        self.anchor = None;
    }

    pub fn count(&self) -> usize {
        let mut total = self.selected.len();
        for &(lo, hi) in &self.ranges {
            total += hi.saturating_sub(lo) + 1;
        }
        total
    }

    pub fn paths(&self) -> Vec<String> {
        self.selected.iter().cloned().collect()
    }

    pub fn all_paths(&self, entries: &[FileEntry]) -> Vec<String> {
        let mut res = self.selected.clone();
        for &(lo, hi) in &self.ranges {
            let max_idx = hi.min(entries.len().saturating_sub(1));
            for i in lo..=max_idx {
                res.insert(entries[i].path.clone());
            }
        }
        res.into_iter().collect()
    }
}

// ─── SearchState ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
}

impl SearchState {
    fn new() -> Self {
        Self { active: false, query: String::new() }
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.query.clear();
    }
}

// ─── NotifyState ─────────────────────────────────────────────────────────────

pub struct NotifItem {
    pub id: Option<String>,
    pub kind: NotifyKind,
    pub message: String,
    pub progress: Option<f32>, // 0.0 ..= 1.0
    pub created: Instant,
    pub duration_ms: u64,
}

pub struct NotifyState {
    pub items: Vec<NotifItem>,
}

impl NotifyState {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn push(&mut self, kind: NotifyKind, message: String, duration_ms: u64) {
        self.items.push(NotifItem {
            id: None,
            kind,
            message,
            progress: None,
            created: Instant::now(),
            duration_ms,
        });
    }

    pub fn success(&mut self, message: String) { self.push(NotifyKind::Success, message, 3000); }
    pub fn error(&mut self, message: String)   { self.push(NotifyKind::Error,   message, 5000); }
    pub fn warning(&mut self, message: String) { self.push(NotifyKind::Warning, message, 4000); }
    pub fn info(&mut self, message: String)    { self.push(NotifyKind::Info,    message, 3000); }

    /// İlerleme çubuklu toast bildirimi oluştur veya var olanı güncelle
    pub fn update_progress(&mut self, id: &str, message: String, progress: f32) {
        if let Some(item) = self.items.iter_mut().find(|n| n.id.as_deref() == Some(id)) {
            item.message = message;
            item.progress = Some(progress.clamp(0.0, 1.0));
            item.created = Instant::now(); // Aktarım sürdükçe toast kapanmaz
            item.duration_ms = 60000;
        } else {
            self.items.push(NotifItem {
                id: Some(id.to_string()),
                kind: NotifyKind::Info,
                message,
                progress: Some(progress.clamp(0.0, 1.0)),
                created: Instant::now(),
                duration_ms: 60000,
            });
        }
    }

    /// İlerleme bildirimini tamamla (kapat veya başarıya çevir)
    pub fn finish_progress(&mut self, id: &str, message: String, success: bool) {
        if let Some(item) = self.items.iter_mut().find(|n| n.id.as_deref() == Some(id)) {
            item.kind = if success { NotifyKind::Success } else { NotifyKind::Error };
            item.message = message;
            item.progress = None;
            item.created = Instant::now();
            item.duration_ms = if success { 3500 } else { 5000 };
            item.id = None;
        } else {
            if success {
                self.success(message);
            } else {
                self.error(message);
            }
        }
    }

    /// Süresi dolan toast'ları temizle — her frame'de çağrılır
    pub fn tick(&mut self) {
        self.items.retain(|n| {
            n.created.elapsed().as_millis() < n.duration_ms as u128
        });
    }
}

// ─── DragState ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct DragState {
    pub dragging: bool,
    pub sources: Vec<String>,
    pub copy_mode: bool,
}

impl DragState {
    fn new() -> Self {
        Self { dragging: false, sources: Vec::new(), copy_mode: false }
    }

    pub fn start(&mut self, sources: Vec<String>, copy: bool) {
        self.dragging = true;
        self.sources = sources;
        self.copy_mode = copy;
    }

    pub fn cancel(&mut self) {
        self.dragging = false;
        self.sources.clear();
        self.copy_mode = false;
    }
}

// ─── TerminalState ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum TerminalMsgKind {
    Input,
    Output,
    Error,
    Info,
}

#[derive(Clone, Debug)]
pub struct TerminalLine {
    pub kind: TerminalMsgKind,
    pub text: String,
}

#[derive(Clone)]
pub struct TerminalState {
    pub open: bool,
    pub height: f32,
    pub cwd: String,
    pub history: Vec<TerminalLine>,
    pub input: String,
    pub cmd_history: Vec<String>,
    pub history_cursor: Option<usize>,
    pub is_running: bool,
    pub should_focus: bool,
    pub scroll_to_bottom: bool,
}

impl TerminalState {
    pub fn new() -> Self {
        let initial_cwd = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        Self {
            open: false,
            height: 220.0,
            cwd: initial_cwd,
            history: vec![
                TerminalLine {
                    kind: TerminalMsgKind::Info,
                    text: "Arch Explorer Integrated Terminal (bash ready)".into(),
                },
            ],
            input: String::new(),
            cmd_history: Vec::new(),
            history_cursor: None,
            is_running: false,
            should_focus: false,
            scroll_to_bottom: false,
        }
    }

    pub fn toggle(&mut self, path: Option<String>) {
        if self.open && path.is_none() {
            self.open = false;
        } else {
            self.open = true;
            self.should_focus = true;
            self.scroll_to_bottom = true;
            if let Some(p) = path {
                self.cwd = p;
            }
        }
    }

    pub fn open_with_path(&mut self, path: String) {
        self.open = true;
        self.cwd = path;
        self.should_focus = true;
        self.scroll_to_bottom = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

// ─── MediaPlayerState ────────────────────────────────────────────────────────

pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

pub struct MediaPlayerState {
    pub current_path: Option<String>,
    pub is_video: bool,
    pub child: Option<std::process::Child>,
    pub audio_child: Option<std::process::Child>,
    pub is_playing: bool,
    pub is_paused: bool,
    pub latest_frame: std::sync::Arc<std::sync::Mutex<Option<VideoFrame>>>,
    pub stop_signal: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub start_instant: Option<std::time::Instant>,
    pub seek_offset: f64,
    pub paused_elapsed: f64,
    pub total_duration: f64,
}

impl MediaPlayerState {
    pub fn new() -> Self {
        Self {
            current_path: None,
            is_video: false,
            child: None,
            audio_child: None,
            is_playing: false,
            is_paused: false,
            latest_frame: std::sync::Arc::new(std::sync::Mutex::new(None)),
            stop_signal: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            start_instant: None,
            seek_offset: 0.0,
            paused_elapsed: 0.0,
            total_duration: 0.0,
        }
    }

    pub fn is_playing(&mut self) -> bool {
        if self.is_paused {
            return true;
        }
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    self.is_playing = false;
                    self.child = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.is_playing = false;
                    self.child = None;
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn elapsed_secs(&self) -> f64 {
        if !self.is_playing && !self.is_paused {
            return 0.0;
        }
        if self.is_paused {
            return self.paused_elapsed;
        }
        if let Some(start) = self.start_instant {
            let el = self.seek_offset + start.elapsed().as_secs_f64();
            if self.total_duration > 0.0 {
                el.min(self.total_duration)
            } else {
                el
            }
        } else {
            self.seek_offset
        }
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(mut a_child) = self.audio_child.take() {
            let _ = a_child.kill();
            let _ = a_child.wait();
        }
        if let Ok(mut lock) = self.latest_frame.lock() {
            *lock = None;
        }
        self.is_playing = false;
        self.is_paused = false;
        self.current_path = None;
        self.start_instant = None;
        self.seek_offset = 0.0;
        self.paused_elapsed = 0.0;
    }

    pub fn play(&mut self, path: String, is_video: bool) {
        self.play_at(path, is_video, 0.0, 0.0);
    }

    pub fn play_at(&mut self, path: String, is_video: bool, start_secs: f64, duration: f64) {
        self.stop();

        self.current_path = Some(path.clone());
        self.is_video = is_video;
        self.seek_offset = start_secs.max(0.0);
        if duration > 0.0 {
            self.total_duration = duration;
        }

        let start_str = format!("{:.2}", self.seek_offset);

        if is_video {
            let w = 960usize;
            let h = 540usize;
            let frame_bytes = w * h * 4;

            self.stop_signal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stop_flag = self.stop_signal.clone();
            let frame_arc = self.latest_frame.clone();

            let mut ffmpeg_proc = std::process::Command::new("ffmpeg")
                .args([
                    "-ss", &start_str,
                    "-re",
                    "-i", &path,
                    "-f", "rawvideo",
                    "-pix_fmt", "rgba",
                    "-vf", "scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2",
                    "-r", "30",
                    "-an",
                    "-v", "quiet",
                    "-",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok();

            if let Some(ref mut proc) = ffmpeg_proc {
                if let Some(mut stdout) = proc.stdout.take() {
                    std::thread::spawn(move || {
                        use std::io::Read;
                        let mut buf = vec![0u8; frame_bytes];
                        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            if stdout.read_exact(&mut buf).is_ok() {
                                if let Ok(mut lock) = frame_arc.lock() {
                                    *lock = Some(VideoFrame {
                                        data: buf.clone(),
                                        width: w,
                                        height: h,
                                    });
                                }
                            } else {
                                break;
                            }
                        }
                    });
                }
            }

            // Sesi arka planda penceresiz çal
            let audio_proc = std::process::Command::new("ffplay")
                .args(["-nodisp", "-autoexit", "-v", "quiet", "-ss", &start_str, &path])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("mpv")
                        .args(["--no-video", &format!("--start={}", start_str), &path])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                })
                .ok();

            self.child = ffmpeg_proc;
            self.audio_child = audio_proc;
            self.is_playing = true;
            self.is_paused = false;
            self.start_instant = Some(std::time::Instant::now());
        } else {
            // Müzik: Arka planda penceresiz doğrudan çal
            let child = std::process::Command::new("ffplay")
                .args(["-nodisp", "-autoexit", "-v", "quiet", "-ss", &start_str, &path])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("mpv")
                        .args(["--no-video", &format!("--start={}", start_str), &path])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                })
                .or_else(|_| {
                    std::process::Command::new("pw-play")
                        .args([&path])
                        .spawn()
                })
                .ok();

            if let Some(c) = child {
                self.child = Some(c);
                self.is_playing = true;
                self.is_paused = false;
                self.start_instant = Some(std::time::Instant::now());
            }
        }
    }

    pub fn toggle_pause(&mut self) {
        if !self.is_playing && !self.is_paused {
            if let Some(p) = self.current_path.clone() {
                self.play_at(p, self.is_video, 0.0, self.total_duration);
            }
            return;
        }

        if self.is_paused {
            // Unpause: play from paused_elapsed
            let resume_pos = self.paused_elapsed;
            if let Some(p) = self.current_path.clone() {
                self.play_at(p, self.is_video, resume_pos, self.total_duration);
            }
        } else {
            // Pause: remember current elapsed and kill active subprocesses
            self.paused_elapsed = self.elapsed_secs();
            self.stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Some(mut a_child) = self.audio_child.take() {
                let _ = a_child.kill();
                let _ = a_child.wait();
            }
            self.is_playing = false;
            self.is_paused = true;
        }
    }

    pub fn seek_to(&mut self, target_secs: f64) {
        if let Some(path) = self.current_path.clone() {
            let clamped = if self.total_duration > 0.0 {
                target_secs.clamp(0.0, self.total_duration)
            } else {
                target_secs.max(0.0)
            };
            if self.is_paused {
                self.paused_elapsed = clamped;
            } else {
                self.play_at(path, self.is_video, clamped, self.total_duration);
            }
        }
    }

    pub fn seek_relative(&mut self, delta_secs: f64) {
        let cur = self.elapsed_secs();
        self.seek_to(cur + delta_secs);
    }

    pub fn toggle(&mut self, path: String, is_video: bool) {
        if (self.is_playing() || self.is_paused) && self.current_path.as_deref() == Some(&path) {
            self.stop();
        } else {
            self.play(path, is_video);
        }
    }
}

impl Drop for MediaPlayerState {
    fn drop(&mut self) {
        self.stop();
    }
}

// ─── AppState ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub nav:       NavigationState,
    pub dir:       DirectoryState,
    pub sidebar:   SidebarState,
    pub panels:    PanelState,
    pub selection: SelectionState,
    pub search:    SearchState,
    pub notify:    NotifyState,
    pub drag:      DragState,

    pub view_mode:  ViewMode,
    pub view_scale: f32,  // Ctrl+scroll zoom, 0.5..3.0
    pub needs_reload: bool,

    // Sekme sistemi
    pub tabs: TabState,

    // Dual pane
    pub dual_pane:   bool,
    pub active_pane: PaneId,
    pub right_pane:  Option<PaneSnapshot>,

    // Mount panel flow sıfırlama sinyali
    pub mount_flow_reset: bool,

    // Inline rename state
    pub rename_target: Option<String>,  // rename edilen dosyanın path'i
    pub rename_buf:    String,          // text input buffer
    pub rename_just_started: bool,      // focus & select-all for first frame

    // Async backend'den gelen sonuçlar — apply_pending'de işlenir
    pub pending_listing: Option<DirectoryListing>,
    pub pending_error:   Option<String>,
    pub pending_select_paths: Option<Vec<String>>,

    // Mount işlemi için şifre gerektiren durumlar
    // (device_path, label, error_msg)
    pub pending_mount_password: Option<(String, String, String)>,
    // (device_path, label) — yanlış şifre
    pub pending_mount_wrong: Option<(String, String)>,
    pub pending_checksum:    Option<(String, String, String)>, // (path, md5, sha256)
    pub pending_checksum_request: Option<String>,
    // Du cache — in-memory sonuçlar (path → bytes)
    pub du_sizes: std::collections::HashMap<String, u64>,
    pub pending_extract: Option<(String, String)>, // (archive, dest_dir)
    pub pending_du_request: Option<String>,
    pub pending_search_query: Option<String>,
    pub pending_search_changed: Option<std::time::Instant>,
    pub preview_width: Option<f32>,
    pub preview_mode: PreviewBehaviorMode,
    pub preview_custom_image: bool,
    pub preview_custom_video: bool,
    pub preview_custom_audio: bool,
    pub preview_custom_text: bool,
    pub preview_custom_doc: bool,
    pub preview_custom_dir: bool,
    pub preview_cache: std::collections::HashMap<String, Result<crate::backend::preview::FilePreview, String>>,
    pub preview_loading: Option<String>,
    pub pending_root_auth: Option<RootAuthRequest>,
    pub pending_root_wrong: bool,
    pub terminal: TerminalState,
    pub selection_prev: Vec<String>,
    pub media_player: MediaPlayerState,
    pub is_rubber_banding: bool,
    pub git_status: crate::backend::git::GitStatus,
    pub is_video_fullscreen: bool,
    pub conflict_dialog: Option<ConflictDialogState>,
    pub delete_error_dialog: Option<DeleteErrorDialogState>,
    pub always_root: bool,
    pub is_root_active: bool,
    pub list_columns: ListViewColumns,
    pub perf_settings: PerfSettings,
    pub pending_transfer: Option<TransferRequest>,
}

#[derive(Clone, Debug)]
pub struct TransferRequest {
    pub sources: Vec<String>,
    pub dest_dir: String,
    pub copy: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ConflictChoice {
    Replace,
    Skip,
    KeepBoth,
}

#[derive(Clone, Debug)]
pub struct ConflictItem {
    pub src_path: String,
    pub dest_path: String,
    pub is_copy: bool,
    pub is_dir: bool,
    pub src_size: u64,
    pub dest_size: u64,
    pub src_modified: String,
    pub dest_modified: String,
}

#[derive(Clone, Debug)]
pub struct ConflictDialogState {
    pub queue: Vec<ConflictItem>,
    pub current_idx: usize,
    pub apply_to_all: bool,
    pub policy_file: Option<ConflictChoice>,
    pub policy_dir: Option<ConflictChoice>,
    pub completed_undo_actions: Vec<crate::backend::clipboard::UndoAction>,
    pub dest_dir: String,
}

#[derive(Clone, Debug)]
pub struct DeleteErrorDialogState {
    pub failed_path: String,
    pub error_msg: String,
    pub remaining_paths: Vec<String>,
    pub permanent: bool,
}

impl AppState {
    pub fn new() -> Self {
        let prev_cfg = load_preview_settings();
        Self {
            nav:       NavigationState::new(),
            dir:       DirectoryState::new(),
            sidebar:   SidebarState::new(),
            panels:    PanelState::new(),
            selection: SelectionState::new(),
            search:    SearchState::new(),
            notify:    NotifyState::new(),
            drag:      DragState::new(),

            view_mode:   ViewMode::Grid,
            view_scale:  crate::theme::load_saved_view_scale().unwrap_or(1.0).clamp(0.4, 3.0),
            needs_reload: true,

            tabs: TabState::new(
                dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string())
            ),

            dual_pane:   false,
            active_pane: PaneId::Left,
            right_pane:  None,

            mount_flow_reset: false,
            rename_target: None,
            rename_buf: String::new(),
            rename_just_started: false,

            pending_listing:        None,
            pending_error:          None,
            pending_select_paths:   None,
            pending_mount_password: None,
            pending_mount_wrong:    None,
            pending_checksum:          None,
            du_sizes:                  std::collections::HashMap::new(),
            pending_checksum_request: None,
            pending_extract:          None,
            pending_du_request:       None,
            pending_search_query:     None,
            pending_search_changed:   None,
            preview_width:            None,
            preview_mode:             prev_cfg.mode,
            preview_custom_image:     prev_cfg.custom_image,
            preview_custom_video:     prev_cfg.custom_video,
            preview_custom_audio:     prev_cfg.custom_audio,
            preview_custom_text:      prev_cfg.custom_text,
            preview_custom_doc:       prev_cfg.custom_doc,
            preview_custom_dir:       prev_cfg.custom_dir,
            preview_cache:            std::collections::HashMap::new(),
            preview_loading:          None,
            pending_root_auth:         None,
            pending_root_wrong:        false,
            terminal:                 TerminalState::new(),
            selection_prev:           Vec::new(),
            media_player:             MediaPlayerState::new(),
            is_rubber_banding:        false,
            git_status:               Default::default(),
            is_video_fullscreen:      false,
            conflict_dialog:          None,
            delete_error_dialog:      None,
            always_root:              load_root_settings().always_root,
            is_root_active:           false,
            list_columns:             load_list_columns(),
            perf_settings:            load_perf_settings(),
            pending_transfer:         None,
        }
    }

    pub fn save_root_settings(&self) {
        save_root_settings_to_disk(&RootSettings {
            always_root: self.always_root,
        });
    }

    pub fn save_perf_settings(&self) {
        save_perf_settings_to_disk(&self.perf_settings);
    }

    pub fn save_list_columns(&self) {
        save_list_columns_to_disk(&self.list_columns);
    }

    pub fn save_preview_settings(&self) {
        let cfg = PreviewSettings {
            mode: self.preview_mode,
            custom_image: self.preview_custom_image,
            custom_video: self.preview_custom_video,
            custom_audio: self.preview_custom_audio,
            custom_text: self.preview_custom_text,
            custom_doc: self.preview_custom_doc,
            custom_dir: self.preview_custom_dir,
        };
        save_preview_settings_to_disk(&cfg);
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    pub fn active_pane_dir(&self) -> String {
        self.nav.current_path.clone()
    }

    pub fn active_context_path(&self) -> String {
        self.active_pane_dir()
    }

    pub fn sync_terminal_cwd(&mut self) {
        let dir = self.active_pane_dir();
        self.terminal.cwd = dir;
    }

    pub fn navigate(&mut self, path: String) {
        self.nav.push(path.clone());
        // Aktif tab'ın görünen path/label'ını güncelle
        self.tabs.active_mut().path = path;
        self.tabs.active_mut().update_label();
        self.selection.clear();
        self.search.clear();
        self.panels.close_context_menu();
        if self.panels.active == ActivePanel::DevicesView {
            self.panels.close();
        }
        self.needs_reload = true;
        self.sync_terminal_cwd();
    }

    pub fn open_terminal(&mut self, path: Option<String>) {
        let target = path.unwrap_or_else(|| self.active_pane_dir());
        self.terminal.open_with_path(target);
    }

    /// Tab geçişi — mevcut tab'ın TAMAMI kaydedilir, yeni tab restore edilir
    pub fn switch_tab(&mut self, idx: usize) {
        if idx == self.tabs.active_idx || idx >= self.tabs.len() { return; }

        // ── Mevcut tab'ı kaydet ───────────────────────────────────────────
        {
            let tab = self.tabs.active_mut();
            tab.path        = self.nav.current_path.clone();
            tab.history     = self.nav.history.clone();
            tab.history_idx = self.nav.history_idx;
            tab.selection   = self.selection.selected.clone();
            tab.search_query = self.search.query.clone();
            tab.update_label();
        }

        // ── Yeni tab'a geç ────────────────────────────────────────────────
        self.tabs.switch_to(idx);

        // ── Yeni tab'ın state'ini restore et ─────────────────────────────
        {
            let tab = self.tabs.active();
            self.nav.current_path = tab.path.clone();
            self.nav.history      = tab.history.clone();
            self.nav.history_idx  = tab.history_idx;
            self.selection.selected = tab.selection.clone();
            self.search.query     = tab.search_query.clone();
            self.search.active    = !tab.search_query.is_empty();
        }

        self.needs_reload = true;
        self.sync_terminal_cwd();
    }

    pub fn open_in_new_tab(&mut self, path: String) {
        // 1. Mevcut tab'ın state'ini kaydet
        {
            let tab = self.tabs.active_mut();
            tab.path        = self.nav.current_path.clone();
            tab.history     = self.nav.history.clone();
            tab.history_idx = self.nav.history_idx;
            tab.selection   = self.selection.selected.clone();
            tab.search_query = self.search.query.clone();
            tab.update_label();
        }

        // 2. Yeni tab'ı ekle ve ona geç
        self.tabs.new_tab(path.clone());
        let new_idx = self.tabs.active_idx;
        self.tabs.switch_to(new_idx);

        // 3. Yeni tab'ın state'ini aktif navigation'a yükle
        {
            let tab = self.tabs.active();
            self.nav.current_path = tab.path.clone();
            self.nav.history      = tab.history.clone();
            self.nav.history_idx  = tab.history_idx;
            self.selection.selected = tab.selection.clone();
            self.search.query     = tab.search_query.clone();
            self.search.active    = !tab.search_query.is_empty();
        }

        self.needs_reload = true;
        self.sync_terminal_cwd();
    }

    pub fn close_current_tab(&mut self) {
        let idx = self.tabs.active_idx;
        if self.tabs.len() <= 1 { return; }
        self.tabs.close_tab(idx);
        let path = self.tabs.active().path.clone();
        self.nav.current_path = path;
        self.needs_reload = true;
        self.sync_terminal_cwd();
    }

    pub fn go_back(&mut self) {
        if self.nav.go_back().is_some() {
            self.tabs.active_mut().path = self.nav.current_path.clone();
            self.tabs.active_mut().update_label();
            self.selection.clear();
            self.search.clear();
            if self.panels.active == ActivePanel::DevicesView {
                self.panels.close();
            }
            self.needs_reload = true;
            self.sync_terminal_cwd();
        }
    }

    pub fn go_forward(&mut self) {
        if self.nav.go_forward().is_some() {
            self.tabs.active_mut().path = self.nav.current_path.clone();
            self.tabs.active_mut().update_label();
            self.selection.clear();
            self.search.clear();
            if self.panels.active == ActivePanel::DevicesView {
                self.panels.close();
            }
            self.needs_reload = true;
            self.sync_terminal_cwd();
        }
    }

    pub fn go_up(&mut self) {
        if self.nav.go_up().is_some() {
            self.tabs.active_mut().path = self.nav.current_path.clone();
            self.tabs.active_mut().update_label();
            self.selection.clear();
            self.search.clear();
            if self.panels.active == ActivePanel::DevicesView {
                self.panels.close();
            }
            self.needs_reload = true;
            self.sync_terminal_cwd();
        }
    }

    pub fn refresh(&mut self) {
        self.needs_reload = true;
    }

    // ── Pending async results → state'e uygula ────────────────────────────────

    pub fn apply_pending(&mut self) {
        if let Some(listing) = self.pending_listing.take() {
            self.dir.load_listing(listing);
            if let Some(pending) = self.pending_select_paths.take() {
                self.selection.select_many(&pending);
            }
        }
        if let Some(err) = self.pending_error.take() {
            self.dir.is_loading = false;
            self.dir.load_error = Some(err);
        }
    }

    pub fn select_paths_and_queue(&mut self, paths: Vec<String>) {
        if paths.is_empty() { return; }
        self.selection.select_many(&paths);
        self.pending_select_paths = Some(paths);
    }

    pub fn duplicate_selected(&mut self) {
        let sources = self.selected_paths();
        if sources.is_empty() { return; }
        let mut undos = Vec::new();
        let mut new_targets = Vec::new();
        for src in &sources {
            let src_path = std::path::Path::new(src);
            if !src_path.exists() { continue; }
            let fname = match src_path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let parent = src_path.parent().unwrap_or_else(|| std::path::Path::new("/"));
            let resolved = crate::backend::clipboard::resolve_conflict(parent, &fname);
            let resolved_str = resolved.to_string_lossy().to_string();
            if let Ok(Some(action)) = execute_transfer_item(src, &resolved_str, true, ConflictChoice::Replace) {
                undos.push(action);
                new_targets.push(resolved_str);
            }
        }
        if !undos.is_empty() {
            let count = new_targets.len();
            let _ = crate::backend::clipboard::push_undo_batch(undos);
            self.refresh();
            self.select_paths_and_queue(new_targets);
            self.notify.success(format!("{} öğe çoğaltıldı", count));
        }
    }

    // ── Selection helpers ─────────────────────────────────────────────────────

    pub fn selected_entries(&self) -> Vec<FileEntry> {
        self.dir.entries.iter().enumerate()
            .filter(|(idx, e)| self.selection.is_selected_at(&e.path, *idx))
            .map(|(_, e)| e.clone())
            .collect()
    }

    pub fn selected_files_size(&self) -> u64 {
        if self.selection.count() == 0 {
            return 0;
        }
        let mut sum = 0u64;
        for &(lo, hi) in &self.selection.ranges {
            let max_idx = hi.min(self.dir.entries.len().saturating_sub(1));
            for i in lo..=max_idx {
                let e = &self.dir.entries[i];
                if !e.is_dir {
                    sum += e.size;
                }
            }
        }
        for p in &self.selection.selected {
            if let Some(e) = self.dir.entries.iter().find(|e| &e.path == p) {
                if !e.is_dir {
                    sum += e.size;
                }
            }
        }
        sum
    }

    pub fn selected_paths(&self) -> Vec<String> {
        let p = self.selection.all_paths(&self.dir.entries);
        if p.is_empty() {
            if let Some(ref last) = self.selection.last_clicked {
                return vec![last.clone()];
            }
        }
        p
    }

    pub fn navigate_media_sibling(&mut self, next: bool) -> Option<String> {
        let is_media_ext = |p: &str| {
            let ext = std::path::Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            matches!(ext.as_str(), "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac")
        };

        let media_files: Vec<String> = self.dir.entries.iter()
            .filter(|e| !e.is_dir && is_media_ext(&e.path))
            .map(|e| e.path.clone())
            .collect();

        if media_files.is_empty() {
            return None;
        }

        let cur_target = self.media_player.current_path.clone()
            .or_else(|| self.selected_paths().first().cloned());

        let cur_idx = cur_target.and_then(|cp| media_files.iter().position(|p| *p == cp));
        let next_idx = match cur_idx {
            Some(idx) => {
                if next {
                    (idx + 1) % media_files.len()
                } else {
                    if idx == 0 { media_files.len() - 1 } else { idx - 1 }
                }
            }
            None => 0,
        };

        let target_path = media_files[next_idx].clone();
        self.selection.select_only(target_path.clone());
        let is_video = !matches!(
            std::path::Path::new(&target_path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str(),
            "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac"
        );
        self.media_player.play(target_path.clone(), is_video);
        Some(target_path)
    }

    pub fn start_rename(&mut self, path: &str) {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.rename_target = Some(path.to_string());
        self.rename_buf    = name;
        self.rename_just_started = true;
    }

    pub fn cancel_rename(&mut self) {
        self.rename_target = None;
        self.rename_buf.clear();
        self.rename_just_started = false;
    }

    pub fn commit_rename(&mut self) {
        if let Some(ref old_path) = self.rename_target.clone() {
            let parent = std::path::Path::new(old_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let new_name = self.rename_buf.trim();
            let new_path = format!("{}/{}", parent, new_name);
            if new_path != *old_path && !new_name.is_empty() {
                if old_path.starts_with("adb://") {
                    if let (Some((serial, old_dev_p)), Some((_, new_dev_p))) = (
                        crate::backend::adb::parse_adb_path(old_path),
                        crate::backend::adb::parse_adb_path(&new_path),
                    ) {
                        if let Err(e) = crate::backend::adb::adb_rename(&serial, &old_dev_p, &new_dev_p) {
                            self.notify.error(format!("ADB yeniden adlandırma hatası: {}", e));
                        } else {
                            self.refresh();
                        }
                    }
                } else if let Err(e) = crate::backend::clipboard::tracked_rename(old_path.clone(), new_path.clone()) {
                    // Try fallback fs_ops
                    if let Err(e2) = crate::backend::fs_ops::rename_item(old_path.clone(), new_path) {
                        self.notify.error(format!("Yeniden adlandırma hatası: {}", e2));
                    } else {
                        self.refresh();
                    }
                } else {
                    self.refresh();
                }
            }
        }
        self.cancel_rename();
    }

    pub fn drop_files_into_dir(&mut self, dest_dir: String) {
        if !self.drag.dragging || self.drag.sources.is_empty() {
            return;
        }
        let sources = self.drag.sources.clone();
        let copy = self.drag.copy_mode;
        self.drag.cancel();
        self.start_transfer(sources, dest_dir, copy);
    }

    pub fn start_transfer(&mut self, sources: Vec<String>, dest_dir: String, copy: bool) {
        if sources.is_empty() { return; }

        let dest_is_adb = dest_dir.starts_with("adb://");
        let any_src_adb = sources.iter().any(|s| s.starts_with("adb://"));

        // ADB Transfer Handler — tamamen arka plan iş parçacığında çalışır, UI asla donmaz!
        if dest_is_adb || any_src_adb {
            self.notify.info(format!("{} öğe arka planda aktarılıyor…", sources.len()));
            self.pending_transfer = Some(TransferRequest {
                sources,
                dest_dir,
                copy,
            });
            return;
        }

        // Yerel transfer çakışma kontrolü
        let mut conflicts = Vec::new();
        let mut direct_sources = Vec::new();

        for src in &sources {
            let src_path = std::path::Path::new(src);
            if !src_path.exists() { continue; }
            let fname = match src_path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let dst = format!("{}/{}", dest_dir.trim_end_matches('/'), fname);
            let dst_path = std::path::Path::new(&dst);

            if dst_path.starts_with(src) && src != &dst {
                // Klasörü kendi alt klasörüne taşımayı/kopyalamayı engelle
                continue;
            }

            if src == &dst {
                if !copy {
                    continue;
                }
            }

            if dst_path.exists() {
                if src_path.is_dir() && dst_path.is_dir() {
                    let mut dummy_undos = Vec::new();
                    let mut dummy_count = 0;
                    collect_dir_conflicts_recursive(src_path, dst_path, copy, &mut conflicts, &mut dummy_undos, &mut dummy_count);
                } else {
                    let (src_sz, src_mod, is_dir) = get_file_summary(src);
                    let (dst_sz, dst_mod, _) = get_file_summary(&dst);
                    conflicts.push(ConflictItem {
                        src_path: src.clone(),
                        dest_path: dst,
                        is_copy: copy,
                        is_dir,
                        src_size: src_sz,
                        dest_size: dst_sz,
                        src_modified: src_mod,
                        dest_modified: dst_mod,
                    });
                }
            } else {
                direct_sources.push(src.clone());
            }
        }

        if !conflicts.is_empty() {
            self.conflict_dialog = Some(ConflictDialogState {
                queue: conflicts,
                current_idx: 0,
                apply_to_all: false,
                policy_file: None,
                policy_dir: None,
                completed_undo_actions: Vec::new(),
                dest_dir: dest_dir.clone(),
            });
            if !direct_sources.is_empty() {
                self.pending_transfer = Some(TransferRequest {
                    sources: direct_sources,
                    dest_dir,
                    copy,
                });
            }
        } else {
            // Çakışma yok: Dosyaları arka planda aktar ve sağ alttan yüzdeli bildirim göster
            self.notify.update_progress("file_transfer", format!("{} öğe aktarılıyor…", direct_sources.len()), 0.0);
            self.pending_transfer = Some(TransferRequest {
                sources: direct_sources,
                dest_dir,
                copy,
            });
        }
    }

    pub fn resolve_current_conflict(&mut self, choice: ConflictChoice) {
        let Some(mut diag) = self.conflict_dialog.take() else { return; };
        if diag.current_idx >= diag.queue.len() {
            if !diag.completed_undo_actions.is_empty() {
                let _ = crate::backend::clipboard::push_undo_batch(diag.completed_undo_actions);
            }
            self.refresh();
            return;
        }

        let is_dir = diag.queue[diag.current_idx].is_dir;

        if diag.apply_to_all {
            if is_dir {
                diag.policy_dir = Some(choice);
            } else {
                diag.policy_file = Some(choice);
            }

            let remaining_slice = diag.queue[diag.current_idx..].to_vec();
            for item in remaining_slice {
                let ch = if item.is_dir {
                    diag.policy_dir.unwrap_or(choice)
                } else {
                    diag.policy_file.unwrap_or(choice)
                };
                if let Ok(Some(action)) = execute_transfer_item(&item.src_path, &item.dest_path, item.is_copy, ch) {
                    diag.completed_undo_actions.push(action);
                }
            }

            let total_completed = diag.completed_undo_actions.len();
            let dest_dir = diag.dest_dir.clone();
            let mut target_paths = Vec::new();
            for a in &diag.completed_undo_actions {
                a.collect_targets(&mut target_paths);
            }
            if !diag.completed_undo_actions.is_empty() {
                let _ = crate::backend::clipboard::push_undo_batch(diag.completed_undo_actions);
            }
            self.refresh();
            if self.nav.current_path.trim_end_matches('/') == dest_dir.trim_end_matches('/') {
                if !target_paths.is_empty() {
                    self.select_paths_and_queue(target_paths);
                }
            }
            self.notify.success(format!("{} öğe başarıyla aktarıldı ({})", total_completed, dest_dir));
        } else {
            let item = diag.queue[diag.current_idx].clone();
            if let Ok(Some(action)) = execute_transfer_item(&item.src_path, &item.dest_path, item.is_copy, choice) {
                diag.completed_undo_actions.push(action);
            }
            diag.current_idx += 1;

            if diag.current_idx >= diag.queue.len() {
                let total_completed = diag.completed_undo_actions.len();
                let dest_dir = diag.dest_dir.clone();
                let mut target_paths = Vec::new();
                for a in &diag.completed_undo_actions {
                    a.collect_targets(&mut target_paths);
                }
                if !diag.completed_undo_actions.is_empty() {
                    let _ = crate::backend::clipboard::push_undo_batch(diag.completed_undo_actions);
                }
                self.refresh();
                if self.nav.current_path.trim_end_matches('/') == dest_dir.trim_end_matches('/') {
                    if !target_paths.is_empty() {
                        self.select_paths_and_queue(target_paths);
                    }
                }
                self.notify.success(format!("{} öğe başarıyla aktarıldı ({})", total_completed, dest_dir));
            } else {
                self.conflict_dialog = Some(diag);
            }
        }
    }

    pub fn cancel_conflict_dialog(&mut self) {
        if let Some(diag) = self.conflict_dialog.take() {
            if !diag.completed_undo_actions.is_empty() {
                let _ = crate::backend::clipboard::push_undo_batch(diag.completed_undo_actions);
            }
            self.refresh();
            self.notify.info("İşlem iptal edildi".into());
        }
    }
}

fn collect_dir_conflicts_recursive(
    src_dir: &std::path::Path,
    dst_dir: &std::path::Path,
    copy: bool,
    conflicts: &mut Vec<ConflictItem>,
    direct_undos: &mut Vec<crate::backend::clipboard::UndoAction>,
    direct_count: &mut usize,
) {
    let entries = match std::fs::read_dir(src_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let child_src = entry.path();
        let fname = entry.file_name();
        let child_dst = dst_dir.join(&fname);

        let child_src_str = child_src.to_string_lossy().to_string();
        let child_dst_str = child_dst.to_string_lossy().to_string();

        if child_dst.exists() {
            if child_src.is_dir() && child_dst.is_dir() {
                collect_dir_conflicts_recursive(&child_src, &child_dst, copy, conflicts, direct_undos, direct_count);
            } else {
                let (src_sz, src_mod, is_dir) = get_file_summary(&child_src_str);
                let (dst_sz, dst_mod, _) = get_file_summary(&child_dst_str);
                conflicts.push(ConflictItem {
                    src_path: child_src_str,
                    dest_path: child_dst_str,
                    is_copy: copy,
                    is_dir,
                    src_size: src_sz,
                    dest_size: dst_sz,
                    src_modified: src_mod,
                    dest_modified: dst_mod,
                });
            }
        } else {
            if let Ok(Some(action)) = execute_transfer_item(&child_src_str, &child_dst_str, copy, ConflictChoice::Replace) {
                direct_undos.push(action);
                *direct_count += 1;
            }
        }
    }
}

fn get_file_summary(path: &str) -> (u64, String, bool) {
    let p = std::path::Path::new(path);
    let is_dir = p.is_dir();
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = meta.and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_else(|| "-".to_string());
    (size, modified, is_dir)
}

pub fn execute_transfer_item(
    src: &str,
    dst: &str,
    is_copy: bool,
    choice: ConflictChoice,
) -> Result<Option<crate::backend::clipboard::UndoAction>, String> {
    let src_path = std::path::Path::new(src);
    let dst_path = std::path::Path::new(dst);
    let is_dir = src_path.is_dir();

    let mut backed_up_path: Option<String> = None;
    let mut was_dst_dir = false;

    let target_path = match choice {
        ConflictChoice::Skip => return Ok(None),
        ConflictChoice::Replace => {
            // Check if src and target point to the exact same file on disk
            let is_same_file = src == dst || (dst_path.exists() && match (src_path.canonicalize(), dst_path.canonicalize()) {
                (Ok(s), Ok(d)) => s == d,
                _ => false,
            });

            if is_same_file {
                if is_copy {
                    // Copying a file onto itself with "Replace" is a no-op!
                    // Do NOT run std::fs::copy(src, dst) because it truncates the file to 0 bytes!
                    // Return Ok(None) so no truncation happens and undo doesn't delete it!
                    return Ok(None);
                } else {
                    // Moving a file onto itself is also a no-op
                    return Ok(None);
                }
            }

            if dst_path.exists() {
                was_dst_dir = dst_path.is_dir();
                match crate::backend::clipboard::backup_overwritten_item(dst_path) {
                    Ok(backup_target) => {
                        backed_up_path = Some(backup_target.to_string_lossy().to_string());
                    }
                    Err(e) => {
                        // Eğer yedekleme bir şekilde başarısız olduysa eskisini kaldır ki işlem tıkanmasın
                        eprintln!("[arch-explorer] Backup warning: {}", e);
                        if dst_path.is_dir() {
                            let _ = std::fs::remove_dir_all(dst_path);
                        } else {
                            let _ = std::fs::remove_file(dst_path);
                        }
                    }
                }
            }
            dst_path.to_path_buf()
        }
        ConflictChoice::KeepBoth => {
            let parent = dst_path.parent().unwrap_or_else(|| std::path::Path::new("/"));
            let name = dst_path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            crate::backend::clipboard::resolve_conflict(parent, name)
        }
    };

    let target_str = target_path.to_string_lossy().to_string();

    if is_copy {
        if is_dir {
            if let Err(e) = crate::backend::clipboard::copy_recursive(src_path, &target_path) {
                // If permission error and root password is cached, try elevated cp
                if crate::backend::fs_ops::get_root_password().is_some() {
                    crate::backend::fs_ops::sudo_run_elevated("cp", &["-r", src, &target_str]).map_err(|e2| format!("{}: {}", e, e2))?;
                } else {
                    return Err(e.to_string());
                }
            }
        } else {
            if let Err(e) = std::fs::copy(src, &target_str) {
                if crate::backend::fs_ops::get_root_password().is_some() {
                    crate::backend::fs_ops::sudo_run_elevated("cp", &[src, &target_str]).map_err(|e2| format!("{}: {}", e, e2))?;
                } else {
                    return Err(e.to_string());
                }
            }
        }

        if let Some(backup_path) = backed_up_path {
            Ok(Some(crate::backend::clipboard::UndoAction::Replace {
                dest: target_str,
                backup_path,
                was_dir: was_dst_dir,
                is_copy: true,
                orig_src: None,
            }))
        } else {
            Ok(Some(crate::backend::clipboard::UndoAction::Copy {
                dest: target_str,
            }))
        }
    } else {
        if let Err(e) = crate::backend::clipboard::move_item_inner(src_path, &target_path) {
            if crate::backend::fs_ops::get_root_password().is_some() {
                crate::backend::fs_ops::sudo_run_elevated("mv", &[src, &target_str]).map_err(|e2| format!("{}: {}", e, e2))?;
            } else {
                return Err(e.to_string());
            }
        }

        if let Some(backup_path) = backed_up_path {
            Ok(Some(crate::backend::clipboard::UndoAction::Replace {
                dest: target_str,
                backup_path,
                was_dir: was_dst_dir,
                is_copy: false,
                orig_src: Some(src.to_string()),
            }))
        } else {
            Ok(Some(crate::backend::clipboard::UndoAction::Move {
                from: target_str,
                to: src.to_string(),
            }))
        }
    }
}

// ─── Tab System ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TabEntry {
    pub path:        String,
    pub history:     VecDeque<String>,
    pub history_idx: usize,
    pub selection:   HashSet<String>,
    pub search_query: String,
    pub label:       String,  // klasör adı
}

impl TabEntry {
    pub fn new(path: String) -> Self {
        let label = std::path::Path::new(&path)
            .file_name().map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let mut history = VecDeque::new();
        history.push_back(path.clone());
        Self { path: path.clone(), history, history_idx: 0, selection: HashSet::new(), search_query: String::new(), label }
    }

    pub fn update_label(&mut self) {
        self.label = std::path::Path::new(&self.path)
            .file_name().map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.clone());
    }
}

#[derive(Clone)]
pub struct TabState {
    pub tabs:       Vec<TabEntry>,
    pub active_idx: usize,
}

impl TabState {
    pub fn new(start_path: String) -> Self {
        Self { tabs: vec![TabEntry::new(start_path)], active_idx: 0 }
    }

    pub fn active(&self) -> &TabEntry { &self.tabs[self.active_idx] }
    pub fn active_mut(&mut self) -> &mut TabEntry { &mut self.tabs[self.active_idx] }
    pub fn len(&self) -> usize { self.tabs.len() }

    pub fn new_tab(&mut self, path: String) {
        self.tabs.insert(self.active_idx + 1, TabEntry::new(path));
        self.active_idx += 1;
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 { return; }
        self.tabs.remove(idx);
        if self.active_idx >= self.tabs.len() {
            self.active_idx = self.tabs.len() - 1;
        } else if self.active_idx > 0 && idx <= self.active_idx {
            self.active_idx -= 1;
        }
    }

    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() { self.active_idx = idx; }
    }

    pub fn next_tab(&mut self) {
        self.active_idx = (self.active_idx + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.active_idx == 0 { self.active_idx = self.tabs.len() - 1; }
        else { self.active_idx -= 1; }
    }

    /// Aktif tab'ı navigate et, history güncelle
    pub fn navigate_active(&mut self, path: String) {
        let tab = self.active_mut();
        while tab.history.len() > tab.history_idx + 1 { tab.history.pop_back(); }
        tab.history.push_back(path.clone());
        tab.history_idx = tab.history.len() - 1;
        tab.path = path;
        tab.selection.clear();
        tab.search_query.clear();
        tab.update_label();
    }

    pub fn save_active_selection(&mut self, selected: HashSet<String>) {
        self.active_mut().selection = selected;
    }
}

// ─── Dual Pane ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaneId { Left, Right }

/// Inactive pane'in snapshot'ı — swap edilince aktif olur
#[derive(Clone)]
pub struct PaneSnapshot {
    pub pane_id:       PaneId,
    pub nav:           NavigationState,
    pub dir:           DirectoryState,
    pub selection:     SelectionState,
    pub search:        SearchState,
    pub drag:          DragState,
    pub view_mode:     ViewMode,
    pub view_scale:    f32,
    pub tabs:          TabState,
    pub needs_reload:  bool,
    pub context_menu:  Option<ContextMenuState>,
    pub rename_target: Option<String>,
    pub rename_buf:    String,
    pub rename_just_started: bool,
    pub pending_listing: Option<DirectoryListing>,
    pub pending_error:   Option<String>,
    pub du_sizes: std::collections::HashMap<String, u64>,
    pub pending_search_query:   Option<String>,
    pub pending_search_changed: Option<std::time::Instant>,
    pub pending_du_request:     Option<String>,
    pub pending_extract:        Option<(String, String)>,
    pub pending_checksum_request: Option<String>,
    pub is_devices_view:        bool,
}

impl PaneSnapshot {
    pub fn from_state(state: &AppState, id: PaneId) -> Self {
        Self {
            pane_id:       id,
            nav:           state.nav.clone(),
            dir:           state.dir.clone(),
            selection:     state.selection.clone(),
            search:        state.search.clone(),
            drag:          state.drag.clone(),
            view_mode:     state.view_mode.clone(),
            view_scale:    state.view_scale,
            tabs:          state.tabs.clone(),
            needs_reload:  state.needs_reload,
            context_menu:  state.panels.context_menu.clone(),
            rename_target: state.rename_target.clone(),
            rename_buf:    state.rename_buf.clone(),
            rename_just_started: state.rename_just_started,
            pending_listing: state.pending_listing.clone(),
            pending_error:   state.pending_error.clone(),
            du_sizes:        state.du_sizes.clone(),
            pending_search_query:     state.pending_search_query.clone(),
            pending_search_changed:   state.pending_search_changed,
            pending_du_request:       state.pending_du_request.clone(),
            pending_extract:          state.pending_extract.clone(),
            pending_checksum_request: state.pending_checksum_request.clone(),
            is_devices_view:          state.panels.active == ActivePanel::DevicesView,
        }
    }

    pub fn selected_files_size(&self) -> u64 {
        if self.selection.count() == 0 {
            return 0;
        }
        let mut sum = 0u64;
        for &(lo, hi) in &self.selection.ranges {
            let max_idx = hi.min(self.dir.entries.len().saturating_sub(1));
            for i in lo..=max_idx {
                let e = &self.dir.entries[i];
                if !e.is_dir {
                    sum += e.size;
                }
            }
        }
        for p in &self.selection.selected {
            if let Some(e) = self.dir.entries.iter().find(|e| &e.path == p) {
                if !e.is_dir {
                    sum += e.size;
                }
            }
        }
        sum
    }
}

impl AppState {
    // ── Dual Pane ─────────────────────────────────────────────────────────────

    /// Dual pane'i aç — mevcut state'i klonlayarak right pane oluştur
    pub fn open_dual_pane(&mut self) {
        if self.dual_pane { return; }
        let snapshot = PaneSnapshot::from_state(self, PaneId::Right);
        self.right_pane    = Some(snapshot);
        self.active_pane   = PaneId::Left;
        self.dual_pane     = true;
    }

    pub fn close_dual_pane(&mut self) {
        // Sağ pane aktifse önce sol'a geç
        if self.active_pane == PaneId::Right {
            self.switch_active_pane();
        }
        self.right_pane  = None;
        self.dual_pane   = false;
        self.active_pane = PaneId::Left;
    }

    pub fn switch_active_pane(&mut self) {
        // take() releases the mutable borrow so we can borrow self again below
        let Some(snap) = self.right_pane.take() else { return; };

        // Snapshot current active state BEFORE overwriting
        let old_snap = PaneSnapshot::from_state(self, self.active_pane);

        // Load snapshot into active state
        self.nav           = snap.nav.clone();
        self.dir           = snap.dir.clone();
        self.selection     = snap.selection.clone();
        self.search        = snap.search.clone();
        self.drag          = snap.drag.clone();
        self.view_mode     = snap.view_mode.clone();
        self.view_scale    = snap.view_scale;
        self.tabs          = snap.tabs.clone();
        self.needs_reload  = snap.needs_reload;
        self.panels.context_menu = snap.context_menu.clone();
        self.rename_target = snap.rename_target.clone();
        self.rename_buf    = snap.rename_buf.clone();
        self.rename_just_started = snap.rename_just_started;
        self.pending_listing         = snap.pending_listing.clone();
        self.pending_error           = snap.pending_error.clone();
        self.du_sizes                = snap.du_sizes.clone();
        self.pending_search_query    = snap.pending_search_query.clone();
        self.pending_search_changed  = snap.pending_search_changed;
        self.pending_du_request      = snap.pending_du_request.clone();
        self.pending_extract         = snap.pending_extract.clone();
        self.pending_checksum_request = snap.pending_checksum_request.clone();

        if snap.is_devices_view {
            self.panels.active = ActivePanel::DevicesView;
        } else if self.panels.active == ActivePanel::DevicesView {
            self.panels.close();
        }

        // Put old state back as the snapshot
        self.right_pane = Some(old_snap);

        self.active_pane = match self.active_pane {
            PaneId::Left  => PaneId::Right,
            PaneId::Right => PaneId::Left,
        };

        if self.terminal.open {
            self.terminal.cwd = self.active_context_path();
        }
    }

    /// Karşı panelin mevcut dizinine kopyala/taşı
    pub fn other_pane_path(&self) -> Option<String> {
        self.right_pane.as_ref().map(|s| s.nav.current_path.clone())
    }

    /// Snapshot'ı güncelle (need_reload gibi değişiklikler için)
    pub fn sync_snapshot_if_needed(&mut self) {
        // nothing — swap approach handles this
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigate_closes_devices_view() {
        let mut state = AppState::new();
        state.panels.open_devices_view();
        assert_eq!(state.panels.active, ActivePanel::DevicesView);

        state.navigate("/home".to_string());
        assert_eq!(state.panels.active, ActivePanel::None);
        assert_eq!(state.nav.current_path, "/home");
    }

    #[test]
    fn test_terminal_open_and_toggle() {
        let mut state = AppState::new();
        assert!(!state.terminal.open);

        state.open_terminal(Some("/home/aiuser".to_string()));
        assert!(state.terminal.open);
        assert_eq!(state.terminal.cwd, "/home/aiuser");
        assert!(state.terminal.should_focus);

        state.terminal.toggle(None);
        assert!(!state.terminal.open);
    }

    #[test]
    fn test_active_context_path_and_terminal_sync() {
        let mut state = AppState::new();
        state.nav.current_path = "/home/aiuser/Downloads".to_string();
        assert_eq!(state.active_pane_dir(), "/home/aiuser/Downloads");
        assert_eq!(state.active_context_path(), "/home/aiuser/Downloads");

        // Selecting a directory does not hijack active_pane_dir
        state.selection.selected.insert("/tmp".to_string());
        assert_eq!(state.active_pane_dir(), "/home/aiuser/Downloads");
        assert_eq!(state.active_context_path(), "/home/aiuser/Downloads");

        state.selection.clear();
        state.open_terminal(None);
        assert_eq!(state.terminal.cwd, "/home/aiuser/Downloads");

        // Dual pane switch syncs terminal cwd to that pane's folder
        state.open_dual_pane();
        state.nav.current_path = "/var/log".to_string();
        state.switch_active_pane();
        assert_eq!(state.terminal.cwd, state.active_pane_dir());
    }

    #[test]
    fn test_media_player_playback() {
        let vid_path = "/tmp/test_crash.mp4";
        if !std::path::Path::new(vid_path).exists() {
            let _ = std::process::Command::new("ffmpeg")
                .args(["-y", "-f", "lavfi", "-i", "testsrc=duration=5:size=320x240:rate=10", vid_path])
                .output();
        }
        let mut mp = MediaPlayerState::new();
        mp.play(vid_path.to_string(), true);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(mp.is_playing());
        mp.stop();
        assert!(!mp.is_playing());
    }

    #[test]
    fn test_self_copy_replace_safety() {
        let test_dir = format!("/tmp/ae_test_self_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        std::fs::create_dir_all(&test_dir).unwrap();
        let file_path = format!("{}/test.txt", test_dir);
        let content = "PRECIOUS USER CONTENT - MUST NEVER BE TRUNCATED";
        std::fs::write(&file_path, content).unwrap();

        // Transferring a file to itself with Replace should be a safe no-op!
        let action = execute_transfer_item(&file_path, &file_path, true, ConflictChoice::Replace).unwrap();
        assert!(action.is_none());

        // Verify content is still intact and not truncated
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_selection_range_and_ctrl() {
        let mut sel = SelectionState::new();
        let make_entry = |name: &str, path: &str| FileEntry {
            name: name.to_string(),
            path: path.to_string(),
            is_dir: false,
            is_symlink: false,
            size: 10,
            modified: String::new(),
            created: String::new(),
            permissions: "644".into(),
            mime_type: "text/plain".into(),
            extension: "txt".into(),
            hidden: false,
            is_executable: false,
        };
        let entries = vec![
            make_entry("a.txt", "/tmp/a.txt"),
            make_entry("b.txt", "/tmp/b.txt"),
            make_entry("c.txt", "/tmp/c.txt"),
            make_entry("d.txt", "/tmp/d.txt"),
        ];

        // Shift selection from a.txt to c.txt
        sel.select_range(&entries, "/tmp/a.txt", "/tmp/c.txt", false);
        assert_eq!(sel.selected.len(), 3);
        assert!(sel.is_selected("/tmp/a.txt"));
        assert!(sel.is_selected("/tmp/b.txt"));
        assert!(sel.is_selected("/tmp/c.txt"));
        assert!(!sel.is_selected("/tmp/d.txt"));

        // Ctrl+Shift additive selection: keep existing, select d.txt
        sel.select_range(&entries, "/tmp/c.txt", "/tmp/d.txt", true);
        assert_eq!(sel.selected.len(), 4);
        assert!(sel.is_selected("/tmp/d.txt"));

        // Regular range replaces without keep_existing
        sel.select_range(&entries, "/tmp/b.txt", "/tmp/c.txt", false);
        assert_eq!(sel.selected.len(), 2);
        assert!(!sel.is_selected("/tmp/a.txt"));
        assert!(sel.is_selected("/tmp/b.txt"));
        assert!(sel.is_selected("/tmp/c.txt"));
        assert!(!sel.is_selected("/tmp/d.txt"));
    }

    #[test]
    fn test_perf_settings_and_ram_optimization() {
        let default_cfg = PerfSettings::default();
        assert_eq!(default_cfg.max_cached_textures, 300);
        assert!(default_cfg.fast_directory_load);
        assert!(!default_cfg.low_memory_mode);

        let mut modified = default_cfg.clone();
        modified.low_memory_mode = true;
        save_perf_settings_to_disk(&modified);

        let loaded = load_perf_settings();
        assert!(loaded.low_memory_mode);
        assert_eq!(loaded.max_cached_textures, 300);

        // Reset to default
        save_perf_settings_to_disk(&default_cfg);

        // Test preview cache clearing & memory safety
        let mut state = AppState::new();
        state.preview_cache.insert("/tmp/dummy.png".into(), Ok(crate::backend::preview::FilePreview {
            kind: "image".into(),
            mime_type: "image/png".into(),
            size: 1024,
            image_b64: None,
            image_rgba: Some((vec![255; 400], 10, 10)),
            text_content: None,
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
            dir_entries: None,
        }));
        assert_eq!(state.preview_cache.len(), 1);
        state.preview_cache.clear();
        assert_eq!(state.preview_cache.len(), 0);
    }
}


