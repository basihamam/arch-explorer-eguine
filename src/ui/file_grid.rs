// src/ui/file_grid.rs
// Ana dosya görünümü — Grid ve List modları, çoklu seçim, drag-drop, thumbnail
use egui::*;
use image; // GIF decoding in draw_grid_thumbnail
use crate::theme::types::AexTheme;
use crate::state::{AppState, ViewMode, MediaGridStyle, ContextMenuState, ActivePanel};
use crate::backend::fs_ops::FileEntry;
use super::{file_icon, human_size, icon};

pub fn render(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let pane_tag = if state.dual_pane {
        if state.active_pane == crate::state::PaneId::Left { "left" } else { "right" }
    } else {
        "single"
    };
    render_tagged(ui, t, state, pane_tag);
}

pub fn render_tagged(ui: &mut Ui, t: &AexTheme, state: &mut AppState, pane_tag: &str) {
    let c  = &t.colors;
    let sp = &t.spacing;

    // Loading göstergesi
    if state.dir.is_loading {
        loading_view(ui, t);
        return;
    }

    // Hata göstergesi
    if let Some(ref err) = state.dir.load_error.clone() {
        error_view(ui, t, err);
        return;
    }

    // Boş klasör
    let entries = state.dir.visible_entries(&state.search.query);
    if entries.is_empty() {
        empty_view(ui, t, state.search.active);
        return;
    }

    if state.drag.dragging {
        state.drag.target_dir = None;
    }

    // Render — mode'a göre
    match state.view_mode {
        ViewMode::Grid    => grid_view(ui, t, state, &entries, pane_tag),
        ViewMode::List    => list_view(ui, t, state, &entries, pane_tag),
        ViewMode::Compact => compact_view(ui, t, state, &entries, pane_tag),
        ViewMode::Columns => list_view(ui, t, state, &entries, pane_tag), // TODO
    }

    // Cancel drag if mouse was released outside any target
    if state.drag.dragging && ui.input(|i| i.pointer.any_released()) {
        state.drag.cancel();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Grid View
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridHitArea {
    pub thumb_hit: Rect,
    pub label_hit: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListHitArea {
    pub row_rect: Rect,
    pub item_hit: Rect,
}

impl GridHitArea {
    #[inline]
    pub fn contains(&self, p: Pos2) -> bool {
        self.thumb_hit.contains(p) || self.label_hit.contains(p)
    }

    #[inline]
    pub fn intersects(&self, r: &Rect) -> bool {
        self.thumb_hit.intersects(*r) || self.label_hit.intersects(*r)
    }
}

pub fn format_nemo_size(bytes: u64) -> String {
    const K: u64 = 1024;
    if bytes < K {
        format!("{} B", bytes)
    } else if bytes < K * K {
        format!("{:.1} kB", bytes as f64 / K as f64)
    } else if bytes < K * K * K {
        format!("{:.1} MB", bytes as f64 / (K * K) as f64)
    } else if bytes < K * K * K * K {
        format!("{:.1} GB", bytes as f64 / (K * K * K) as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / (K * K * K * K) as f64)
    }
}

pub fn entry_type_label(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        return "Folder";
    }
    let ext = entry.extension.to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "ico" | "svg" | "tiff" | "avif" | "heic" | "heif" => "Image",
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" | "ts" => "Video",
        "mp3" | "flac" | "ogg" | "wav" | "aac" | "m4a" | "opus" | "wma" => "Audio",
        "pdf" => "PDF Document",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" => "Archive",
        "txt" | "md" | "rst" | "log" => "Plain Text",
        "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" | "html" | "css" | "json" | "toml" | "yaml" | "yml" | "sh" | "bash" | "zsh" => "Document",
        "doc" | "docx" | "odt" => "Document",
        "xls" | "xlsx" | "ods" | "csv" => "Spreadsheet",
        "ppt" | "pptx" | "odp" => "Presentation",
        "iso" | "img" => "Disk Image",
        _ if entry.is_executable => "Executable",
        _ => "File",
    }
}

pub fn truncate_to_width(ui: &Ui, text: &str, font_id: &FontId, max_width: f32) -> String {
    if max_width <= 8.0 {
        return String::new();
    }
    let galley = ui.painter().layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE);
    if galley.size().x <= max_width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut low = 0;
    let mut high = chars.len();
    let mut best = String::from("…");
    while low <= high {
        let mid = (low + high) / 2;
        let candidate = format!("{}…", chars[..mid].iter().collect::<String>());
        let g = ui.painter().layout_no_wrap(candidate.clone(), font_id.clone(), Color32::WHITE);
        if g.size().x <= max_width {
            best = candidate;
            low = mid + 1;
        } else {
            if mid == 0 { break; }
            high = mid - 1;
        }
    }
    best
}

fn is_media_entry(entry: &FileEntry) -> bool {
    let ext = entry.extension.to_lowercase();
    matches!(ext.as_str(), "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"|"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts")
}

fn grid_view(ui: &mut Ui, t: &AexTheme, state: &mut AppState, entries: &[FileEntry], pane_tag: &str) {
    let c         = &t.colors;
    let sp        = &t.spacing;
    let s         = &t.shape;

    let item_size = (t.layout.grid_item_size * state.view_scale).clamp(40.0, 240.0);
    let gap       = t.layout.grid_gap.max(12.0);

    // Zoom level → thumbnail quality
    let thumb_q = if item_size < 100.0 {
        crate::backend::thumbnails::ThumbQuality::Low
    } else if item_size < 170.0 {
        crate::backend::thumbnails::ThumbQuality::Mid
    } else {
        crate::backend::thumbnails::ThumbQuality::High
    };

    let available_w = ui.available_width();
    let cols = ((available_w + gap) / (item_size + gap)).max(1.0) as usize;

    // Preserve scroll position when column count changes (e.g. when preview panel opens or closes)
    let scroll_id = Id::new(format!("file_grid_{}", pane_tag)).with(&state.nav.current_path);
    let cols_cache_id = Id::new(format!("prev_cols_{}", pane_tag));
    let prev_cols: Option<usize> = ui.ctx().data_mut(|d| d.get_temp(cols_cache_id));
    if let Some(old_c) = prev_cols {
        if old_c != cols && old_c > 0 && cols > 0 {
            if let Some(mut scroll_state) = egui::scroll_area::State::load(ui.ctx(), scroll_id) {
                scroll_state.offset.y = scroll_state.offset.y * (old_c as f32 / cols as f32);
                scroll_state.store(ui.ctx(), scroll_id);
            }
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(cols_cache_id, cols));

    // Pane-tagged ID: Sol/Sağ panel geçişinde scroll asla sıfırlanmaz
    egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .auto_shrink([false; 2])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            // Background rect — use visible clip rect in screen coords
            let bg_rect = ui.clip_rect();

            // Click deselect / context menu via ctx.input
            let ptr_clicked  = ui.ctx().input(|i| i.pointer.primary_clicked());
            let ptr_sec      = ui.ctx().input(|i| i.pointer.secondary_clicked());
            let ptr_pos      = ui.ctx().input(|i| i.pointer.interact_pos());
            let ptr_in_bg    = ptr_pos.map(|p| bg_rect.contains(p)).unwrap_or(false);

            // Rubber-band drag state & selection
            let rb_id = Id::new(format!("rubber_band_start_grid_{}", pane_tag));
            let is_primary_down = ui.ctx().input(|i| i.pointer.primary_down());
            let press_origin = ui.ctx().input(|i| i.pointer.press_origin());

            let rects_cache_id = Id::new(format!("rendered_rects_grid_{}", pane_tag));
            let prev_rects: Vec<GridHitArea> = ui.ctx().data_mut(|d| d.get_temp(rects_cache_id)).unwrap_or_default();
            let pressed_on_item = press_origin.map(|origin| prev_rects.iter().any(|hit| hit.contains(origin))).unwrap_or(false);

            let is_resizing_preview: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_preview")).unwrap_or(false));
            let is_resizing_sidebar: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_sidebar")).unwrap_or(false));
            let is_resizing_column: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_column")).unwrap_or(false));
            let is_preview_open = state.panels.active == ActivePanel::Preview;
            let scroll_bar_w = (ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_inner_margin + ui.spacing().scroll.bar_outer_margin).max(18.0) + 10.0;
            let safe_right_margin = if is_preview_open { 36.0 } else { scroll_bar_w };
            let safe_left_margin = 0.0;
            let safe_bottom_margin = (ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_inner_margin).max(18.0) + 6.0;
            let safe_bg_rect = Rect::from_min_max(
                Pos2::new(bg_rect.min.x + safe_left_margin, bg_rect.min.y),
                Pos2::new(bg_rect.max.x - safe_right_margin, bg_rect.max.y - safe_bottom_margin),
            );

            let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            let shift = ui.input(|i| i.modifiers.shift);

            if ptr_clicked && ptr_in_bg && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column && !ctrl && !shift {
                state.selection.clear();
            }
            if ptr_sec && ptr_in_bg && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column {
                if let Some(pos) = ptr_pos {
                    state.panels.context_menu = Some(ContextMenuState { x: pos.x, y: pos.y, target: None });
                }
            }

            let content_top_y = ui.cursor().min.y;
            let rb_rel_y_id = Id::new(format!("rubber_band_rel_y_grid_{}", pane_tag));
            let rb_init_sel_id = Id::new(format!("rubber_band_init_sel_grid_{}", pane_tag));

            // Only start rubber-band if press was NOT on any file item, NOT on preview/sidebar resize handle, NOT on scrollbars/column resizer, and NOT dragging files
            if is_primary_down && !state.drag.dragging && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column {
                if let (Some(origin), Some(cur_pos)) = (press_origin, ptr_pos) {
                    if safe_bg_rect.contains(origin) {
                        let drag_delta = cur_pos - origin;
                        if drag_delta.length() > 4.0 {
                            let already_active = ui.ctx().data_mut(|d| d.get_temp::<Pos2>(rb_id).is_some());
                            if !already_active {
                                let start_rel_y = origin.y - content_top_y;
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(rb_id, origin);
                                    d.insert_temp(rb_rel_y_id, start_rel_y);
                                    d.insert_temp(rb_init_sel_id, state.selection.selected.clone());
                                });
                            }
                            state.is_rubber_banding = true;
                        }
                    }
                }
            } else if !is_primary_down || state.drag.dragging || is_resizing_preview {
                ui.ctx().data_mut(|d| {
                    d.remove::<Pos2>(rb_id);
                    d.remove::<f32>(rb_rel_y_id);
                    d.remove::<std::collections::HashSet<String>>(rb_init_sel_id);
                });
                state.is_rubber_banding = false;
            }

            // Draw rubber-band on foreground layer and update selection with auto-scroll
            let mut active_rb_rect: Option<Rect> = None;
            if let (Some(start), Some(start_rel_y)) = (
                ui.ctx().data_mut(|d| d.get_temp::<Pos2>(rb_id)),
                ui.ctx().data_mut(|d| d.get_temp::<f32>(rb_rel_y_id)),
            ) {
                if let Some(cur) = ptr_pos {
                    state.is_rubber_banding = true;

                    // Auto-scroll when rubber-band dragged near or past the top / bottom edge
                    if cur.y < bg_rect.min.y + 45.0 {
                        let overflow = ((bg_rect.min.y + 45.0) - cur.y).max(0.0);
                        let scroll_speed = (overflow * 0.45 + 4.0).clamp(4.0, 36.0);
                        ui.scroll_with_delta(Vec2::new(0.0, scroll_speed));
                        ui.ctx().request_repaint();
                    } else if cur.y > bg_rect.max.y - 45.0 {
                        let overflow = (cur.y - (bg_rect.max.y - 45.0)).max(0.0);
                        let scroll_speed = (overflow * 0.45 + 4.0).clamp(4.0, 36.0);
                        ui.scroll_with_delta(Vec2::new(0.0, -scroll_speed));
                        ui.ctx().request_repaint();
                    }

                    // Calculate selection rectangle anchored directly to content position where dragging started
                    let current_start_y = content_top_y + start_rel_y;
                    let effective_start = Pos2::new(start.x, current_start_y);
                    let rb_rect = Rect::from_two_pos(effective_start, cur);
                    active_rb_rect = Some(rb_rect);

                    // Visible box clamped to viewport background so it never paints over toolbars
                    let visible_box = rb_rect.intersect(bg_rect);
                    if visible_box.is_positive() {
                        let theme_accent = c.accent();
                        let rb_fill = Color32::from_rgba_unmultiplied(
                            theme_accent.r(), theme_accent.g(), theme_accent.b(), 35
                        );
                        let rb_stroke = theme_accent;

                        let fg_painter = ui.ctx().layer_painter(LayerId::new(Order::Foreground, Id::new("rubber_band_layer")));
                        fg_painter.rect_filled(visible_box, egui::Rounding::ZERO, rb_fill);
                        fg_painter.rect_stroke(visible_box, egui::Rounding::ZERO, egui::Stroke::new(1.0_f32, rb_stroke));
                    }
                }
            }

            let total_count = entries.len();
            let is_media_dir = entries.iter().any(is_media_entry);

            let mut current_frame_rects = Vec::with_capacity(total_count + 1);
            let mut rendered_entries: Vec<FileEntry> = Vec::with_capacity(total_count + 1);

            if is_media_dir && state.media_grid_style == MediaGridStyle::Masonry {
                let col_w = ((available_w - (cols.saturating_sub(1)) as f32 * gap) / cols as f32).max(60.0);
                let mut col_assignments: Vec<Vec<usize>> = vec![Vec::new(); cols];
                let mut col_heights: Vec<f32> = vec![0.0; cols];

                for (idx, entry) in entries.iter().enumerate() {
                    let shortest_col = col_heights.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i).unwrap_or(0);
                    col_assignments[shortest_col].push(idx);

                    let est_h = if is_media_entry(entry) {
                        let cache_key = format!("thumb_{}_{}", entry.path, thumb_q.suffix());
                        let thumb_id = egui::Id::new(&cache_key);
                        let handle: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(thumb_id));
                        let aspect = if let Some(ref h) = handle {
                            (h.size_vec2().x / h.size_vec2().y).clamp(0.4, 2.4)
                        } else {
                            1.25
                        };
                        let th = ((col_w - 6.0) / aspect).clamp(col_w * 0.45, col_w * 1.5).round();
                        th + 42.0
                    } else {
                        col_w * 1.05
                    };
                    col_heights[shortest_col] += est_h + gap;
                }

                ui.columns(cols, |columns| {
                    for (c, col_ui) in columns.iter_mut().enumerate() {
                        col_ui.spacing_mut().item_spacing = Vec2::new(0.0, gap);
                        for &idx in &col_assignments[c] {
                            let entry = &entries[idx];
                            let can_expand = col_assignments[c].last() == Some(&idx);
                            let item_hit = masonry_grid_item(col_ui, t, state, entry, entries, col_w, thumb_q, idx, can_expand);
                            current_frame_rects.push(item_hit);
                            rendered_entries.push(entry.clone());
                        }
                    }
                });
                ui.add_space(60.0);
            } else {
                let total_rows = (total_count + cols - 1) / cols;
                let item_w = item_size;
                let item_h = (item_size * 1.25).max(item_size + 38.0).round();
                let row_h = item_h + gap;

                let view_top = (bg_rect.min.y - content_top_y).max(0.0);
                let view_bottom = (bg_rect.max.y - content_top_y).max(0.0);

                let first_visible_row = if row_h > 0.0 { ((view_top / row_h).floor() as usize).min(total_rows) } else { 0 };
                let last_visible_row = if row_h > 0.0 { (((view_bottom / row_h).ceil() as usize) + 1).min(total_rows) } else { total_rows };

                let start_row = first_visible_row.saturating_sub(4);
                let end_row = (last_visible_row + 4).min(total_rows);

                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                let top_spacer = start_row as f32 * row_h;
                if top_spacer > 0.0 {
                    ui.add_space(top_spacer);
                }

                for row_idx in start_row..end_row {
                    let row_start = row_idx * cols;
                    let row_end = (row_start + cols).min(total_count);
                    let is_last_row = (row_idx + 1) >= total_rows;

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(gap, 0.0);
                        for idx in row_start..row_end {
                            let entry = &entries[idx];
                            let has_item_below = (idx + cols) < total_count;
                            // İsim uzaması SADECE son satırdaki öğelerde yapılabilir!
                            let can_expand = is_last_row && !has_item_below;
                            let item_hit = grid_item(ui, t, state, entry, entries, item_w, item_h, thumb_q, idx, can_expand);
                            current_frame_rects.push(item_hit);
                            rendered_entries.push(entry.clone());
                        }
                    });
                    ui.add_space(gap);
                }

                let bottom_spacer = (total_rows.saturating_sub(end_row)) as f32 * row_h;
                if bottom_spacer > 0.0 {
                    ui.add_space(bottom_spacer);
                }
                ui.add_space(60.0);
            }

            // Update rubber band selection using stored item rects
            if let Some(rb_rect) = active_rb_rect {
                let ctrl_active = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let shift_active = ui.input(|i| i.modifiers.shift);
                let initial_sel: std::collections::HashSet<String> = ui.ctx().data_mut(|d| d.get_temp(rb_init_sel_id)).unwrap_or_default();

                let mut next_sel = if ctrl_active || shift_active {
                    initial_sel
                } else {
                    std::collections::HashSet::new()
                };

                for entry in entries {
                    let item_rect_id = Id::new(format!("item_rect_{}", entry.path));
                    if let Some(r) = ui.ctx().data_mut(|d| d.get_temp::<Rect>(item_rect_id)) {
                        if rb_rect.intersects(r) {
                            next_sel.insert(entry.path.clone());
                        }
                    }
                }
                state.selection.selected = next_sel;
            }

            ui.ctx().data_mut(|d| d.insert_temp(rects_cache_id, current_frame_rects));

            // Allocate empty background area at the bottom so user can always click in empty space to deselect / context menu
            let avail_h = ui.available_height().max(100.0);
            let (_empty_rect, empty_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), avail_h), Sense::click());
            if empty_resp.clicked() && !ui.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift) {
                state.selection.clear();
            }
            if empty_resp.secondary_clicked() {
                if let Some(pos) = ui.ctx().pointer_interact_pos() {
                    state.panels.context_menu = Some(ContextMenuState { x: pos.x, y: pos.y, target: None });
                }
            }
        });
}

fn get_grid_thumb_rect(
    ctx: &egui::Context,
    entry: &FileEntry,
    thumb_center: Pos2,
    item_w: f32,
    max_thumb_w: f32,
    max_thumb_h: f32,
    thumb_q: crate::backend::thumbnails::ThumbQuality,
) -> Rect {
    let icon_box_sz = (item_w * 0.50).clamp(26.0, 64.0);
    let tight_icon_rect = Rect::from_center_size(thumb_center, Vec2::splat(icon_box_sz));

    let ext = entry.extension.to_lowercase();
    let is_media = !entry.is_dir && matches!(ext.as_str(),
        "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"
        |"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts"
        |"mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma");

    if !is_media {
        return tight_icon_rect;
    }

    if ext == "gif" {
        let gif_id = egui::Id::new(format!("gif_cache_{}", entry.path));
        let frames: Option<Vec<egui::TextureHandle>> = ctx.data_mut(|d| d.get_temp(gif_id));
        if let Some(ref flist) = frames {
            if let Some(h) = flist.first() {
                let tex_size = h.size_vec2();
                let aspect = if tex_size.y > 0.0 { (tex_size.x / tex_size.y).clamp(0.2, 5.0) } else { 1.0 };
                let (tw, th) = if aspect >= (max_thumb_w / max_thumb_h) {
                    (max_thumb_w, (max_thumb_w / aspect).round())
                } else {
                    (((max_thumb_h * aspect).round()).min(max_thumb_w), max_thumb_h)
                };
                return Rect::from_center_size(thumb_center, Vec2::new(tw, th));
            }
        }
    } else {
        let cache_key = format!("thumb_{}_{}", entry.path, thumb_q.suffix());
        let thumb_id = egui::Id::new(&cache_key);
        let handle: Option<egui::TextureHandle> = ctx.data_mut(|d| d.get_temp(thumb_id));
        if let Some(ref h) = handle {
            let tex_size = h.size_vec2();
            let aspect = if tex_size.y > 0.0 { (tex_size.x / tex_size.y).clamp(0.2, 5.0) } else { 1.0 };
            let (tw, th) = if aspect >= (max_thumb_w / max_thumb_h) {
                (max_thumb_w, (max_thumb_w / aspect).round())
            } else {
                (((max_thumb_h * aspect).round()).min(max_thumb_w), max_thumb_h)
            };
            return Rect::from_center_size(thumb_center, Vec2::new(tw, th));
        }
    }

    tight_icon_rect
}

fn masonry_grid_item(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    entry: &FileEntry,
    visible_entries: &[FileEntry],
    item_w: f32,
    thumb_q: crate::backend::thumbnails::ThumbQuality,
    idx: usize,
    can_expand: bool,
) -> GridHitArea {
    let c = &t.colors;
    let s = &t.shape;
    let is_sel = state.selection.is_selected_at(&entry.path, idx);
    let is_cut = false;
    let is_renaming = state.rename_target.as_deref() == Some(&entry.path);

    let is_media = is_media_entry(entry);

    let (thumb_h, _) = if is_media {
        let cache_key = format!("thumb_{}_{}", entry.path, thumb_q.suffix());
        let thumb_id = egui::Id::new(&cache_key);
        let handle: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(thumb_id));
        let aspect = if let Some(ref h) = handle {
            (h.size_vec2().x / h.size_vec2().y).clamp(0.4, 2.4)
        } else {
            1.25
        };
        let th = ((item_w - 6.0) / aspect).clamp(item_w * 0.45, item_w * 1.5).round();
        (th, aspect)
    } else {
        ((item_w * 0.65).round(), 1.0)
    };

    let name_font  = (item_w * 0.102).clamp(9.5, 12.5);
    let badge_font = (item_w * 0.080).clamp(8.0, 10.5);
    let font_id = FontId::new(name_font, FontFamily::Proportional);
    let max_text_w = (item_w + 4.0).max(24.0);

    // Standart öğelerde maksimum 3 satır; yalnızca sonda ve altında öğe yoksa (can_expand) 20 satır
    let max_lines = if can_expand { 20 } else { 3 };
    let name_lines = wrap_name_multiline_measured(ui, &entry.name, &font_id, max_text_w, max_lines);

    let line_spacing = name_font * 1.20;
    let label_h = if can_expand {
        (name_lines.len() as f32 * line_spacing + 4.0).max(16.0)
    } else {
        (name_lines.len() as f32 * line_spacing + 4.0).clamp(16.0, 52.0)
    };
    let has_badge = entry.is_dir || entry.size > 0;
    let badge_h = if has_badge { badge_font + 6.0 } else { 0.0 };

    let total_card_h = thumb_h + label_h + badge_h + 12.0;

    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(item_w, total_card_h),
        Sense::click_and_drag(),
    );

    let item_rect_id = Id::new(format!("item_rect_{}", entry.path));
    ui.ctx().data_mut(|d| d.insert_temp(item_rect_id, rect));

    let thumb_rect = Rect::from_min_size(
        rect.min + Vec2::new(3.0, 3.0),
        Vec2::new(item_w - 6.0, thumb_h),
    );

    let text_w = name_lines.iter().map(|line| {
        let galley = ui.painter().layout_no_wrap(line.clone(), font_id.clone(), Color32::WHITE);
        galley.size().x
    }).fold(0.0_f32, f32::max);
    let name_text_w = (text_w + 8.0).clamp(24.0, max_text_w);
    let label_top_y = rect.min.y + thumb_h + 6.0;
    let name_center_y = label_top_y + label_h / 2.0;
    let label_pill_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, name_center_y),
        Vec2::new(name_text_w, label_h),
    );
    let label_bottom_y = if has_badge {
        rect.max.y - 4.0
    } else {
        label_pill_rect.max.y
    };

    // Nemo connected hitbox layout:
    // Tightened icon box for masonry when not media or thumbnail not yet ready
    let icon_box_sz = (item_w * 0.50).clamp(26.0, 64.0);
    let visual_thumb = if is_media {
        let has_thumb = if entry.extension.eq_ignore_ascii_case("gif") {
            let gif_id = egui::Id::new(format!("gif_cache_{}", entry.path));
            let frames: Option<Vec<egui::TextureHandle>> = ui.ctx().data_mut(|d| d.get_temp(gif_id));
            frames.map_or(false, |f| !f.is_empty())
        } else {
            let cache_key = format!("thumb_{}_{}", entry.path, thumb_q.suffix());
            let thumb_id = egui::Id::new(&cache_key);
            let handle: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(thumb_id));
            handle.is_some()
        };
        if has_thumb {
            thumb_rect
        } else {
            Rect::from_center_size(Pos2::new(rect.center().x, thumb_rect.center().y), Vec2::splat(icon_box_sz))
        }
    } else {
        Rect::from_center_size(Pos2::new(rect.center().x, thumb_rect.center().y), Vec2::splat(icon_box_sz))
    };

    // 1) Thumbnail hitbox: bounds of thumbnail box with 2px margin on left, top, right;
    //    extending its bottom margin all the way down to connect seamlessly with the label hitbox!
    // 2) Label hitbox: text/pill bounds with ~2px margin, extending its top up to meet thumbnail.
    // 3) Together, covers the entire active interactive area of the card with zero dead zones.
    let thumb_margin = 2.0_f32;
    let label_margin = 2.0_f32;
    let thumb_hit = Rect::from_min_max(
        Pos2::new(visual_thumb.min.x - thumb_margin, visual_thumb.min.y - thumb_margin),
        Pos2::new(visual_thumb.max.x + thumb_margin, (label_top_y + 1.0).max(visual_thumb.max.y + thumb_margin)),
    );
    let label_hit = Rect::from_min_max(
        Pos2::new(label_pill_rect.min.x - label_margin, visual_thumb.max.y.min(label_top_y - label_margin)),
        Pos2::new(label_pill_rect.max.x + label_margin, label_bottom_y + label_margin),
    );

    let hit_area = GridHitArea {
        thumb_hit,
        label_hit,
    };

    let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()));
    let is_hovered_on_item = resp.hovered() && pointer_pos.map_or(false, |p| hit_area.contains(p));

    let hover_anim_id = Id::new(format!("masonry_hover_{}", entry.path));
    let hover_factor = ui.ctx().animate_bool(hover_anim_id, is_hovered_on_item);
    let sel_anim_id = Id::new(format!("masonry_sel_{}", entry.path));
    let sel_factor = ui.ctx().animate_bool(sel_anim_id, is_sel);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let opacity = if is_cut { 0.4 } else { 1.0 };

        if sel_factor > 0.001 {
            let sel_alpha = (38.0 * sel_factor) as u8;
            let sel_bg = Color32::from_rgba_unmultiplied(
                c.accent().r(), c.accent().g(), c.accent().b(), sel_alpha
            );
            painter.rect_filled(rect, Rounding::same(s.radius_md), sel_bg);
            let border_alpha = (255.0 * sel_factor) as u8;
            let border_col = Color32::from_rgba_unmultiplied(
                c.accent().r(), c.accent().g(), c.accent().b(), border_alpha
            );
            painter.rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(1.5_f32, border_col));
        } else if hover_factor > 0.001 {
            let hover_alpha = (95.0 * hover_factor) as u8;
            let hover_bg = Color32::from_rgba_unmultiplied(
                c.bg_surface2().r(), c.bg_surface2().g(), c.bg_surface2().b(), hover_alpha
            );
            painter.rect_filled(rect, Rounding::same(s.radius_md), hover_bg);
            let hover_stroke_alpha = (45.0 * hover_factor) as u8;
            let hover_stroke_col = Color32::from_rgba_unmultiplied(
                c.text_hint().r(), c.text_hint().g(), c.text_hint().b(), hover_stroke_alpha
            );
            painter.rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(1.0_f32, hover_stroke_col));
        }

        let drew_thumb = if is_media {
            let cache_key = format!("thumb_{}_{}", entry.path, thumb_q.suffix());
            let thumb_id = egui::Id::new(&cache_key);
            let handle: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(thumb_id));
            let tint = Color32::from_rgba_unmultiplied(255, 255, 255, if is_cut { 102 } else { (255.0 * opacity) as u8 });
            if let Some(ref h) = handle {
                painter.rect_filled(thumb_rect, Rounding::same(s.radius_sm), Color32::from_black_alpha(30));
                painter.image(
                    h.id(),
                    thumb_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    tint,
                );
                true
            } else {
                draw_grid_thumbnail(ui, t, entry, rect, item_w, thumb_h, thumb_q, is_cut, opacity)
            }
        } else {
            draw_grid_thumbnail(ui, t, entry, rect, item_w, thumb_h, thumb_q, is_cut, opacity)
        };

        if !drew_thumb {
            let icon_str = file_icon(t, entry);
            let icon_color = icon_color_for(t, entry);
            let icon_font = (item_w * 0.46).clamp(24.0, 60.0);
            let icon_font_id = FontId::new(icon_font, FontFamily::Monospace);
            let icon_color_tinted = Color32::from_rgba_unmultiplied(
                icon_color.r(), icon_color.g(), icon_color.b(),
                (255.0 * opacity) as u8,
            );
            let icon_center = Pos2::new(rect.center().x, (rect.min.y + 3.0 + label_top_y) / 2.0);
            painter.text(
                icon_center,
                Align2::CENTER_CENTER,
                icon_str,
                icon_font_id,
                icon_color_tinted,
            );
        }

        let name_center_y = rect.min.y + thumb_h + 6.0 + label_h / 2.0;
        let text_color = if is_sel {
            c.accent()
        } else {
            Color32::from_rgba_unmultiplied(c.text_primary().r(), c.text_primary().g(), c.text_primary().b(), (255.0 * opacity) as u8)
        };

        let start_y = name_center_y - ((name_lines.len() - 1) as f32 * line_spacing) / 2.0;
        for (li, line) in name_lines.iter().enumerate() {
            let ly = start_y + li as f32 * line_spacing;
            let is_last = li + 1 == name_lines.len();
            // Ara satırlarda ASLA ellipsis olmamalıdır! Yalnızca son satır kesilebilir.
            let display_line = if is_last {
                truncate_to_width(ui, line, &font_id, max_text_w)
            } else {
                line.clone()
            };
            painter.text(
                Pos2::new(rect.center().x, ly),
                Align2::CENTER_CENTER,
                display_line,
                font_id.clone(),
                text_color,
            );
        }

        if has_badge {
            let badge_text = if entry.is_dir {
                if let Some(&sz) = state.du_sizes.get(&entry.path) {
                    format_nemo_size(sz)
                } else if let Some(sz) = crate::backend::du_cache::get_cached_size(&entry.path) {
                    format_nemo_size(sz)
                } else {
                    "klasör".into()
                }
            } else {
                format_nemo_size(entry.size)
            };
            let badge_font_id = FontId::new(badge_font, FontFamily::Proportional);
            let truncated_badge = truncate_to_width(ui, &badge_text, &badge_font_id, max_text_w);
            let badge_y = rect.max.y - 6.0;
            painter.text(
                Pos2::new(rect.center().x, badge_y),
                Align2::CENTER_BOTTOM,
                truncated_badge,
                badge_font_id,
                c.text_hint(),
            );
        }
    }

    if !state.drag.dragging && !is_renaming && resp.hovered() && is_hovered_on_item {
        attach_delayed_hover_tooltip(&resp, ui, t, entry, is_sel);
    }

    let interact_pos = resp.interact_pointer_pos()
        .or_else(|| ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.press_origin()).or(i.pointer.latest_pos())));
    let clicked_on_item = interact_pos.map_or(false, |p| hit_area.contains(p));

    if resp.clicked() {
        if clicked_on_item {
            let is_ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            let is_shift = ui.input(|i| i.modifiers.shift);
            if is_ctrl {
                state.selection.toggle(entry.path.clone());
            } else if is_shift {
                let anchor = state.selection.last_clicked.clone().unwrap_or_else(|| entry.path.clone());
                state.selection.select_range(visible_entries, &anchor, &entry.path, is_ctrl);
            } else {
                state.selection.select_only(entry.path.clone());
            }
        } else {
            let modifiers = ui.input(|i| i.modifiers);
            if !modifiers.ctrl && !modifiers.command && !modifiers.shift {
                state.selection.clear();
            }
        }
    }
    if resp.double_clicked() && !is_renaming && clicked_on_item {
        open_entry(state, entry);
    }
    if resp.secondary_clicked() {
        let pos = interact_pos.unwrap_or_else(|| rect.center());
        if clicked_on_item {
            if !state.selection.is_selected(&entry.path) {
                state.selection.select_only(entry.path.clone());
            }
            state.panels.context_menu = Some(ContextMenuState {
                x: pos.x,
                y: pos.y,
                target: Some(entry.clone()),
            });
        } else {
            state.panels.context_menu = Some(ContextMenuState {
                x: pos.x,
                y: pos.y,
                target: None,
            });
        }
    }

    let press_origin = ui.ctx().input(|i| i.pointer.press_origin());
    let drag_originated_on_item = press_origin.map_or(false, |p| hit_area.contains(p));

    if resp.drag_started() && drag_originated_on_item {
        if !state.selection.is_selected(&entry.path) {
            state.selection.select_only(entry.path.clone());
        }
        let sources = state.selection.paths();
        let copy = ui.input(|i| i.modifiers.ctrl);
        spawn_native_drag(&sources);
        state.drag.start(sources, copy);
    }

    // Drop target handling for folders
    let is_pointer_over = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .map(|p| hit_area.contains(p) || rect.contains(p))
        .unwrap_or(false);

    if state.drag.dragging && entry.is_dir && is_pointer_over {
        state.drag.target_dir = Some(entry.path.clone());
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(2.5_f32, c.accent()));
        ui.painter().rect_filled(rect, Rounding::same(s.radius_md),
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 55));

        if ui.input(|i| i.pointer.any_released()) {
            let dest = entry.path.clone();
            let sources = state.drag.sources.clone();
            let copy = state.drag.copy_mode;
            state.drag.cancel();
            if !sources.contains(&dest) {
                state.start_transfer(sources, dest, copy);
            }
        }
    }

    hit_area
}

fn grid_item(
    ui: &mut Ui, t: &AexTheme, state: &mut AppState,
    entry: &FileEntry,
    visible_entries: &[FileEntry],
    item_w: f32,
    item_h: f32,
    thumb_q: crate::backend::thumbnails::ThumbQuality,
    idx: usize,
    can_expand: bool,
) -> GridHitArea {
    let c      = &t.colors;
    let s      = &t.shape;
    let is_sel = state.selection.is_selected_at(&entry.path, idx);
    let is_cut = false;
    let is_renaming = state.rename_target.as_deref() == Some(&entry.path);

    let max_text_w = (item_w + 4.0).max(24.0);
    let name_font  = (item_w * 0.096).clamp(9.5, 12.5);
    let sub_font   = (name_font * 0.86).clamp(8.5, 10.5);
    let name_font_id = FontId::new(name_font, FontFamily::Proportional);
    let sub_font_id  = FontId::new(sub_font, FontFamily::Proportional);
    let line_spacing = name_font * 1.20;
    let sub_line_step = sub_font * 1.25;

    // Multi-line filename wrapping:
    // Standart öğelerde maksimum 3 satır (10-15 karakter daha fazla gösterim).
    // Yalnızca son satırda ve altında öğe yoksa (can_expand) 20 satıra kadar tam isim göster!
    let max_lines = if can_expand { 20 } else { 3 };
    let name_lines = wrap_name_multiline_measured(ui, &entry.name, &name_font_id, max_text_w, max_lines);

    let icon_area_h = (item_h * 0.52).round();
    let label_top_y_offset = icon_area_h + 3.0;

    let total_card_h = if !can_expand {
        item_h
    } else {
        let name_pill_h = name_lines.len() as f32 * line_spacing + 4.0;
        let needed_h = label_top_y_offset + name_pill_h + sub_font * 0.7 + 2.0 + sub_line_step + sub_font * 0.7 + 8.0;
        needed_h.max(item_h)
    };

    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(item_w, total_card_h),
        Sense::click_and_drag(),
    );

    let label_top_y = rect.min.y + label_top_y_offset;

    // Visual calculations: Proportional layout coordinates (Nemo style)
    let max_thumb_w = (item_w - 12.0).max(24.0);
    let max_thumb_h = (icon_area_h - 6.0).max(24.0);
    let visual_center = Pos2::new(rect.center().x, (rect.min.y + 3.0 + label_top_y) / 2.0);
    let visual_thumb = get_grid_thumb_rect(ui.ctx(), entry, visual_center, item_w, max_thumb_w, max_thumb_h, thumb_q);

    let text_w = name_lines.iter().map(|line| {
        let galley = ui.painter().layout_no_wrap(line.clone(), name_font_id.clone(), Color32::WHITE);
        galley.size().x
    }).fold(0.0_f32, f32::max);
    let name_text_w = (text_w + 8.0).clamp(24.0, max_text_w);

    let (name_pill_h, y_name, y_size, y_type, label_bottom_y) = if !can_expand {
        let label_avail_h = (rect.min.y + item_h - 3.0 - label_top_y).max(18.0);
        let pill_h = if is_renaming {
            20.0_f32
        } else {
            (name_lines.len() as f32 * line_spacing + 4.0).min(label_avail_h - 2.0)
        };
        let y_n = label_top_y + pill_h / 2.0;
        let y_s = (y_n + pill_h / 2.0 + sub_font * 0.7 + 2.0).min(rect.min.y + item_h - sub_line_step - 2.0);
        let y_t = (y_s + sub_line_step).min(rect.min.y + item_h - 4.0);
        let l_bottom = if y_t < rect.min.y + item_h - 1.0 && y_t > y_s + sub_font * 0.8 {
            y_t + sub_font * 0.7
        } else if y_s < rect.min.y + item_h - 2.0 {
            y_s + sub_font * 0.7
        } else {
            y_n + pill_h / 2.0
        };
        (pill_h, y_n, y_s, y_t, l_bottom)
    } else {
        let pill_h = if is_renaming {
            20.0_f32
        } else {
            name_lines.len() as f32 * line_spacing + 4.0
        };
        let y_n = label_top_y + pill_h / 2.0;
        let y_s = y_n + pill_h / 2.0 + sub_font * 0.7 + 2.0;
        let y_t = y_s + sub_line_step;
        let l_bottom = y_t + sub_font * 0.7;
        (pill_h, y_n, y_s, y_t, l_bottom)
    };

    let label_pill_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, y_name),
        Vec2::new(name_text_w, name_pill_h),
    );

    // Nemo connected hitbox layout:
    // 1) Thumbnail hitbox: bounds of thumbnail/icon with 2px margin on left, top, right;
    //    extending its bottom margin all the way down to connect seamlessly with the label hitbox!
    // 2) Label hitbox: text/pill bounds with ~2px margin, extending its top up to meet thumbnail.
    // 3) Together, covers the entire active interactive area of the card with zero dead zones.
    let thumb_margin = 2.0_f32;
    let label_margin = 2.0_f32;

    let thumb_hit = Rect::from_min_max(
        Pos2::new(visual_thumb.min.x - thumb_margin, visual_thumb.min.y - thumb_margin),
        Pos2::new(visual_thumb.max.x + thumb_margin, (label_top_y + 1.0).max(visual_thumb.max.y + thumb_margin)),
    );
    let label_hit = Rect::from_min_max(
        Pos2::new(label_pill_rect.min.x - label_margin, visual_thumb.max.y.min(label_top_y - label_margin)),
        Pos2::new(label_pill_rect.max.x + label_margin, label_bottom_y + label_margin),
    );

    let hit_area = GridHitArea {
        thumb_hit,
        label_hit,
    };

    // Store item rect for rubber-band and click deselect
    let item_rect_id = Id::new(format!("item_rect_{}", entry.path));
    ui.ctx().data_mut(|d| d.insert_temp(item_rect_id, rect));

    let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()));
    let is_hovered_on_item = resp.hovered() && pointer_pos.map_or(false, |p| hit_area.contains(p));

    // ── Smooth UI Animations (Tauri / Modern Desktop Style) ──
    let hover_anim_id = Id::new(format!("hover_anim_{}", entry.path));
    let hover_factor = ui.ctx().animate_bool(hover_anim_id, is_hovered_on_item);
    let sel_anim_id = Id::new(format!("sel_anim_{}", entry.path));
    let sel_factor = ui.ctx().animate_bool(sel_anim_id, is_sel);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let opacity = if is_cut { 0.4 } else { 1.0 };

        // ── Card Selection & Hover Backgrounds with Smooth Interpolation ──
        if sel_factor > 0.001 {
            let sel_alpha = (38.0 * sel_factor) as u8;
            let sel_bg = Color32::from_rgba_unmultiplied(
                c.accent().r(), c.accent().g(), c.accent().b(), sel_alpha
            );
            painter.rect_filled(rect, Rounding::same(s.radius_md), sel_bg);
            let border_alpha = (255.0 * sel_factor) as u8;
            let border_col = Color32::from_rgba_unmultiplied(
                c.accent().r(), c.accent().g(), c.accent().b(), border_alpha
            );
            painter.rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(1.5_f32, border_col));
            // Name pill highlight
            painter.rect_filled(label_pill_rect.intersect(rect.shrink(2.0)), Rounding::same(3.0), border_col);
        } else if hover_factor > 0.001 {
            let hover_alpha = (95.0 * hover_factor) as u8;
            let hover_bg = Color32::from_rgba_unmultiplied(
                c.bg_surface2().r(), c.bg_surface2().g(), c.bg_surface2().b(), hover_alpha
            );
            painter.rect_filled(rect, Rounding::same(s.radius_md), hover_bg);
            let hover_stroke_alpha = (45.0 * hover_factor) as u8;
            let hover_stroke_col = Color32::from_rgba_unmultiplied(
                c.text_hint().r(), c.text_hint().g(), c.text_hint().b(), hover_stroke_alpha
            );
            painter.rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(1.0_f32, hover_stroke_col));
        }

        // Geri alma / aktarım sonrası parıldama efekti (Glow pulse)
        if let Some(pulse) = state.get_highlight_pulse(&entry.path) {
            let glow_col = Color32::from_rgba_unmultiplied(
                c.accent().r(), c.accent().g(), c.accent().b(), (200.0 * pulse) as u8
            );
            painter.rect_stroke(
                rect.expand(2.5),
                Rounding::same(s.radius_md + 2.0),
                Stroke::new(2.5_f32, glow_col),
            );
            ui.ctx().request_repaint();
        }

        // Thumbnail çizildiyse icon çizme (native aspect ratio korunur)
        let drew_thumb = draw_grid_thumbnail(ui, t, entry, rect, item_w, item_h, thumb_q, is_cut, opacity);

        // Icon — sadece thumbnail yoksa çiz
        if !drew_thumb {
            let icon_str = file_icon(t, entry);
            let icon_color = icon_color_for(t, entry);
            let icon_font = (item_w * 0.46).clamp(24.0, 60.0);
            let icon_font_id = FontId::new(icon_font, FontFamily::Monospace);
            let icon_color_tinted = Color32::from_rgba_unmultiplied(
                icon_color.r(), icon_color.g(), icon_color.b(),
                (255.0 * opacity) as u8,
            );

            let icon_center = Pos2::new(rect.center().x, (rect.min.y + 3.0 + label_top_y) / 2.0);
            painter.text(
                icon_center,
                Align2::CENTER_CENTER,
                icon_str,
                icon_font_id,
                icon_color_tinted,
            );
        }

        // Symlink indicator
        if entry.is_symlink {
            painter.text(
                rect.min + Vec2::new(item_w * 0.76, item_h * 0.16),
                Align2::CENTER_CENTER,
                icon(t, "symlink"),
                FontId::new(sub_font, FontFamily::Monospace),
                c.info(),
            );
        }

        // File name — rename mode veya normal
        if is_renaming {
            let te_rect = Rect::from_center_size(
                Pos2::new(rect.center().x, y_name),
                Vec2::new(max_text_w, 20.0),
            );
            let te_id = Id::new(format!("grid_rename_{}", entry.path));
            let te_resp = ui.put(
                te_rect,
                egui::TextEdit::singleline(&mut state.rename_buf)
                    .id(te_id)
                    .font(FontId::new(t.typography.size_xs, FontFamily::Proportional))
                    .desired_width(max_text_w)
                    .frame(true),
            );
            if state.rename_just_started {
                te_resp.request_focus();
                let char_len = state.rename_buf.chars().count();
                let select_len = if entry.is_dir {
                    char_len
                } else if let Some(dot_idx) = state.rename_buf.rfind('.') {
                    dot_idx
                } else {
                    char_len
                };
                let mut te_state = egui::text_edit::TextEditState::load(ui.ctx(), te_id).unwrap_or_default();
                te_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(select_len),
                )));
                te_state.store(ui.ctx(), te_id);
                state.rename_just_started = false;
            }

            if ui.input(|i| i.key_pressed(Key::Enter)) {
                state.commit_rename();
            } else if te_resp.lost_focus() {
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.cancel_rename();
                } else {
                    state.commit_rename();
                }
            }
        } else {
            // Text color: when selected, text stands out clearly in the accent pill
            let text_color = if is_sel {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(
                    c.text_primary().r(), c.text_primary().g(), c.text_primary().b(),
                    (255.0 * opacity) as u8
                )
            };

            let start_y = y_name - ((name_lines.len() - 1) as f32 * line_spacing) / 2.0;
            for (li, line) in name_lines.iter().enumerate() {
                let ly = start_y + li as f32 * line_spacing;
                let is_last = li + 1 == name_lines.len();
                // Ara satırlarda ASLA ellipsis olmamalıdır! Yalnızca son satır kesilebilir.
                let display_line = if is_last {
                    truncate_to_width(ui, line, &name_font_id, max_text_w)
                } else {
                    line.clone()
                };
                painter.text(
                    Pos2::new(rect.center().x, ly),
                    Align2::CENTER_CENTER,
                    display_line,
                    name_font_id.clone(),
                    text_color,
                );
            }
        }

        // Lines 2 & 3: Nemo-style size and type labels (reference screenshot matching)
        let size_str = if entry.is_dir {
            if let Some(&sz) = state.du_sizes.get(&entry.path) {
                format_nemo_size(sz)
            } else if let Some(sz) = crate::backend::du_cache::get_cached_size(&entry.path) {
                format_nemo_size(sz)
            } else if let Some(cnt) = crate::backend::du_cache::get_dir_item_count(&entry.path) {
                format!("{} items", cnt)
            } else {
                crate::backend::du_cache::request_dir_item_count(&entry.path, ui.ctx().clone());
                "Folder".to_string()
            }
        } else {
            format_nemo_size(entry.size)
        };
        let display_size = truncate_to_width(ui, &size_str, &sub_font_id, max_text_w);

        let type_str = entry_type_label(entry);
        let display_type = truncate_to_width(ui, type_str, &sub_font_id, max_text_w);

        if y_size < rect.max.y - 2.0 {
            painter.text(
                Pos2::new(rect.center().x, y_size),
                Align2::CENTER_CENTER,
                display_size,
                sub_font_id.clone(),
                c.text_hint(),
            );
        }

        if y_type < rect.max.y - 1.0 && y_type > y_size + sub_font * 0.8 {
            painter.text(
                Pos2::new(rect.center().x, y_type),
                Align2::CENTER_CENTER,
                display_type,
                sub_font_id,
                c.text_hint(),
            );
        }

        // Git status badge (eğer repo içindeyse ve değiştirildiyse)
        if let Some((b_str, b_col)) = git_badge_for(state, entry) {
            let b_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - 18.0, rect.min.y + 4.0),
                Vec2::new(14.0, 14.0),
            );
            painter.rect_filled(b_rect, Rounding::same(3.0), b_col);
            painter.text(
                b_rect.center(),
                Align2::CENTER_CENTER,
                b_str,
                FontId::new(9.0, FontFamily::Proportional),
                Color32::from_rgb(15, 15, 20),
            );
        }
    }

    if !state.drag.dragging && !is_renaming && resp.hovered() && is_hovered_on_item {
        attach_delayed_hover_tooltip(&resp, ui, t, entry, is_sel);
    }

    // ── Interactions ──────────────────────────────────────────────────────
    let interact_pos = resp.interact_pointer_pos()
        .or_else(|| ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.press_origin()).or(i.pointer.latest_pos())));
    let clicked_on_item = interact_pos.map_or(false, |p| hit_area.contains(p));

    // Single click
    if resp.clicked() && !is_renaming {
        if clicked_on_item {
            let modifiers = ui.input(|i| i.modifiers);
            let is_ctrl = modifiers.ctrl || modifiers.command;
            let is_shift = modifiers.shift;

            if is_ctrl && !is_shift {
                state.selection.toggle(entry.path.clone());
            } else if is_shift {
                let anchor = state.selection.anchor.clone()
                    .or_else(|| state.selection.last_clicked.clone())
                    .unwrap_or_else(|| entry.path.clone());
                state.selection.select_range(visible_entries, &anchor, &entry.path, is_ctrl);
            } else {
                state.selection.select_only(entry.path.clone());
            }
        } else {
            // Clicked in empty cell margin -> clear selection (empty canvas click)
            let modifiers = ui.input(|i| i.modifiers);
            if !modifiers.ctrl && !modifiers.command && !modifiers.shift {
                state.selection.clear();
            }
        }
    }

    // Double click — only if clicked ON the item
    if resp.double_clicked() && !is_renaming && clicked_on_item {
        open_entry(state, entry);
    }

    // Right click
    if resp.secondary_clicked() {
        let pos = interact_pos.unwrap_or_else(|| rect.center());
        if clicked_on_item {
            if !state.selection.is_selected(&entry.path) {
                state.selection.select_only(entry.path.clone());
            }
            state.panels.context_menu = Some(ContextMenuState {
                x: pos.x, y: pos.y, target: Some(entry.clone()),
            });
        } else {
            // Tıpkı Nemo ve Arch Explorer Tauri gibi: Boş alana sağ tıklanırsa tuval bağlam menüsü açılır
            state.panels.context_menu = Some(ContextMenuState {
                x: pos.x, y: pos.y, target: None,
            });
        }
    }

    // Drag start — only if drag started ON the item!
    let press_origin = ui.ctx().input(|i| i.pointer.press_origin());
    let drag_originated_on_item = press_origin.map_or(false, |p| hit_area.contains(p));

    if resp.drag_started() && drag_originated_on_item {
        if !state.selection.is_selected(&entry.path) {
            state.selection.select_only(entry.path.clone());
        }
        let sources = state.selection.paths();
        let copy = ui.input(|i| i.modifiers.ctrl);
        // Sürükle-bırak panoyu (clipboard) kirletmez; doğrudan yerel Wayland DnD motorunu başlatır
        spawn_native_drag(&sources);
        state.drag.start(sources, copy);
    }

    // Drop target handling for folders
    let is_pointer_over = ui.ctx().pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_hover_pos())
        .map(|p| hit_area.contains(p) || rect.contains(p))
        .unwrap_or(false);

    if state.drag.dragging && entry.is_dir && is_pointer_over {
        state.drag.target_dir = Some(entry.path.clone());
        // Prominent glowing border & tint for drop target
        ui.painter().rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(2.5_f32, c.accent()));
        ui.painter().rect_filled(rect, Rounding::same(s.radius_md),
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 55));

        if ui.input(|i| i.pointer.any_released()) {
            let dest = entry.path.clone();
            let sources = state.drag.sources.clone();
            let copy = state.drag.copy_mode;
            state.drag.cancel();
            if !sources.contains(&dest) {
                state.start_transfer(sources, dest, copy);
            }
        }
    }

    hit_area
}

// ─────────────────────────────────────────────────────────────────────────────
// List View
// ─────────────────────────────────────────────────────────────────────────────

fn list_view(ui: &mut Ui, t: &AexTheme, state: &mut AppState, entries: &[FileEntry], pane_tag: &str) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    let row_h = (t.layout.item_height * state.view_scale).clamp(18.0, 80.0);
    let show_thumb = row_h >= 28.0;

    // Minimum içerik genişliği: Başlıklar ve satırların dar pencerelerde ezilip iç içe girmesini önler
    let min_list_w = {
        let size_w = if state.list_columns.show_size { state.list_columns.size_w.clamp(60.0, 300.0) + 16.0 } else { 0.0 };
        let perm_w = if state.list_columns.show_perm { state.list_columns.perm_w.clamp(60.0, 300.0) + 16.0 } else { 0.0 };
        let mod_w = if state.list_columns.show_modified { state.list_columns.modified_w.clamp(80.0, 400.0) + 16.0 } else { 0.0 };
        let type_w = if state.list_columns.show_type { state.list_columns.type_w.clamp(50.0, 250.0) + 16.0 } else { 0.0 };
        let acc_w = if state.list_columns.show_accessed { state.list_columns.accessed_w.clamp(80.0, 400.0) + 16.0 } else { 0.0 };
        let prefix_w = sp.md + 26.0;
        let fixed_right = size_w + perm_w + mod_w + type_w + acc_w + 32.0; // modified'ı sola çekecek rahat pay
        prefix_w + 140.0 + fixed_right // En az 140px isim alanı garanti
    };

    // Her iki yönde scroll (ScrollArea::both) ile uygulama daraldığında yatay kaydırma çubuğu devreye girer
    egui::ScrollArea::both()
        .id_salt(Id::new(format!("file_list_scroll_{}", pane_tag)).with(&state.nav.current_path))
        .auto_shrink([false; 2])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            let avail_w = ui.available_width().max(min_list_w);
            ui.set_min_width(avail_w);

            // Column headers sticky inside scroll area
            list_headers_with_width(ui, t, state, avail_w);

            let bg_rect = ui.clip_rect();
            let ptr_clicked  = ui.ctx().input(|i| i.pointer.primary_clicked());
            let ptr_sec      = ui.ctx().input(|i| i.pointer.secondary_clicked());
            let ptr_pos      = ui.ctx().input(|i| i.pointer.interact_pos());
            let ptr_in_bg    = ptr_pos.map(|p| bg_rect.contains(p)).unwrap_or(false);

            let rb_id = Id::new(format!("rubber_band_start_list_{}", pane_tag));
            let is_primary_down = ui.ctx().input(|i| i.pointer.primary_down());
            let press_origin = ui.ctx().input(|i| i.pointer.press_origin());

            let rects_cache_id = Id::new(format!("rendered_rects_list_{}", pane_tag));
            let prev_hits: Vec<ListHitArea> = ui.ctx().data_mut(|d| d.get_temp(rects_cache_id)).unwrap_or_default();
            // Sol kenardaki boşluk payı (16px) veya satırın solunda tıklandığında öğe üzerine basılmamış sayılır -> şeffaf seçim açılır!
            let pressed_on_item = press_origin.map(|origin| {
                origin.x >= bg_rect.min.x + 22.0 && prev_hits.iter().any(|h| h.item_hit.contains(origin))
            }).unwrap_or(false);

            let is_resizing_preview: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_preview")).unwrap_or(false));
            let is_resizing_sidebar: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_sidebar")).unwrap_or(false));
            let is_resizing_column: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_column")).unwrap_or(false));
            let is_preview_open = state.panels.active == ActivePanel::Preview;
            let scroll_bar_w = (ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_inner_margin + ui.spacing().scroll.bar_outer_margin).max(18.0) + 10.0;
            let safe_right_margin = if is_preview_open { 36.0 } else { scroll_bar_w };
            let safe_left_margin = 0.0;
            let safe_bottom_margin = (ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_inner_margin).max(18.0) + 6.0;
            let safe_bg_rect = Rect::from_min_max(
                Pos2::new(bg_rect.min.x + safe_left_margin, bg_rect.min.y),
                Pos2::new(bg_rect.max.x - safe_right_margin, bg_rect.max.y - safe_bottom_margin),
            );

            let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            let shift = ui.input(|i| i.modifiers.shift);

            if ptr_clicked && ptr_in_bg && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column && !ctrl && !shift && !state.is_rubber_banding {
                state.selection.clear();
            }
            if ptr_sec && ptr_in_bg && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column {
                if let Some(pos) = ptr_pos {
                    state.panels.context_menu = Some(ContextMenuState { x: pos.x, y: pos.y, target: None });
                }
            }

            let content_top_y = ui.cursor().min.y;
            let rb_rel_y_id = Id::new(format!("rubber_band_rel_y_list_{}", pane_tag));
            let rb_init_sel_id = Id::new(format!("rubber_band_init_sel_list_{}", pane_tag));

            // Only start rubber-band if press was NOT on preview/sidebar resize handle, NOT on scrollbars/column resizers, and NOT dragging files
            if is_primary_down && !state.drag.dragging && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column {
                if let (Some(origin), Some(cur_pos)) = (press_origin, ptr_pos) {
                    if safe_bg_rect.contains(origin) {
                        let drag_delta = cur_pos - origin;
                        if drag_delta.length() > 4.0 {
                            let already_active = ui.ctx().data_mut(|d| d.get_temp::<Pos2>(rb_id).is_some());
                            if !already_active {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(rb_id, origin);
                                    d.insert_temp(rb_init_sel_id, state.selection.selected.clone());
                                });
                            }
                            state.is_rubber_banding = true;
                        }
                    }
                }
            } else if !is_primary_down || state.drag.dragging || is_resizing_preview {
                ui.ctx().data_mut(|d| {
                    d.remove::<Pos2>(rb_id);
                    d.remove::<f32>(rb_rel_y_id);
                    d.remove::<std::collections::HashSet<String>>(rb_init_sel_id);
                });
                state.is_rubber_banding = false;
            }

            let mut active_rb_screen_rect: Option<Rect> = None;
            if let Some(start) = ui.ctx().data_mut(|d| d.get_temp::<Pos2>(rb_id)) {
                if let Some(cur) = ptr_pos {
                    state.is_rubber_banding = true;

                    // Auto-scroll when rubber-band dragged near or past the top / bottom edge
                    if cur.y < bg_rect.min.y + 45.0 {
                        let overflow = ((bg_rect.min.y + 45.0) - cur.y).max(0.0);
                        let scroll_speed = (overflow * 0.45 + 4.0).clamp(4.0, 36.0);
                        ui.scroll_with_delta(Vec2::new(0.0, scroll_speed));
                        ui.ctx().request_repaint();
                    } else if cur.y > bg_rect.max.y - 45.0 {
                        let overflow = (cur.y - (bg_rect.max.y - 45.0)).max(0.0);
                        let scroll_speed = (overflow * 0.45 + 4.0).clamp(4.0, 36.0);
                        ui.scroll_with_delta(Vec2::new(0.0, -scroll_speed));
                        ui.ctx().request_repaint();
                    }

                    // Pure Screen-space rubber-band box matching exactly what the user sees & drags
                    let rb_screen = Rect::from_two_pos(start, cur);
                    active_rb_screen_rect = Some(rb_screen);

                    let visible_box = rb_screen.intersect(bg_rect);
                    if visible_box.is_positive() {
                        let theme_accent = c.accent();
                        let rb_fill = Color32::from_rgba_unmultiplied(
                            theme_accent.r(), theme_accent.g(), theme_accent.b(), 35
                        );
                        let rb_stroke = theme_accent;

                        let fg_painter = ui.ctx().layer_painter(LayerId::new(Order::Foreground, Id::new("rubber_band_layer_list")));
                        fg_painter.rect_filled(visible_box, egui::Rounding::ZERO, rb_fill);
                        fg_painter.rect_stroke(visible_box, egui::Rounding::ZERO, egui::Stroke::new(1.0_f32, rb_stroke));
                    }
                }
            }

            let total_count = entries.len();
            let view_top = (bg_rect.min.y - content_top_y).max(0.0);
            let view_bottom = (bg_rect.max.y - content_top_y).max(0.0);

            let first_visible_idx = if row_h > 0.0 { ((view_top / row_h).floor() as usize).min(total_count) } else { 0 };
            let last_visible_idx = if row_h > 0.0 { (((view_bottom / row_h).ceil() as usize) + 1).min(total_count) } else { total_count };

            let start_idx = first_visible_idx.saturating_sub(4);
            let end_idx = (last_visible_idx + 4).min(total_count);

            let mut current_frame_rects: Vec<ListHitArea> = Vec::with_capacity(end_idx - start_idx + 1);
            let mut rendered_entries: Vec<FileEntry> = Vec::with_capacity(end_idx - start_idx + 1);

            let top_spacer = start_idx as f32 * row_h;
            if top_spacer > 0.0 {
                ui.add_space(top_spacer);
            }

            for idx in start_idx..end_idx {
                let entry = &entries[idx];
                let is_renaming = state.rename_target.as_deref() == Some(&entry.path);
                let is_sel = state.selection.is_selected_at(&entry.path, idx);
                let w = avail_w.max(ui.available_width());
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::click_and_drag());
                rendered_entries.push(entry.clone());

                let size_w = state.list_columns.size_w.clamp(60.0, 300.0);
                let perm_w = state.list_columns.perm_w.clamp(60.0, 300.0);
                let mod_w = state.list_columns.modified_w.clamp(80.0, 400.0);
                let right_space = size_w + perm_w + mod_w + 24.0;

                // Dinamik boyutlandırma: Row yüksekliğine göre ikon/thumbnail boyutu ve yazı başlangıç konumu
                let th_sz = (row_h - 4.0).clamp(16.0, 72.0);
                let icon_display_sz = (row_h * 0.60).clamp(15.0, 48.0);
                let icon_cx = rect.min.x + sp.md + 4.0 + (th_sz.max(icon_display_sz) / 2.0);
                let actual_name_x = (icon_cx + (th_sz.max(icon_display_sz) / 2.0) + 8.0).max(rect.min.x + sp.md + 26.0);

                // Rubber-band hit testing
                let item_rect_id = Id::new(format!("item_rect_{}", entry.path));
                ui.ctx().data_mut(|d| d.insert_temp(item_rect_id, rect));

                // Smooth animations for list row hover & selection
                let list_hover_id = Id::new(format!("lst_hov_{}", entry.path));
                let list_hov_factor = ui.ctx().animate_bool(list_hover_id, resp.hovered());
                let list_sel_id = Id::new(format!("lst_sel_{}", entry.path));
                let list_sel_factor = ui.ctx().animate_bool(list_sel_id, is_sel);

                if ui.is_rect_visible(rect) {
                    // Row background with smooth transition
                    if list_sel_factor > 0.001 {
                        let sel_alpha = (36.0 * list_sel_factor) as u8;
                        let bg = Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), sel_alpha);
                        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);
                    } else if list_hov_factor > 0.001 {
                        let hov_alpha = (80.0 * list_hov_factor) as u8;
                        let bg = Color32::from_rgba_unmultiplied(c.bg_surface0().r(), c.bg_surface0().g(), c.bg_surface0().b(), hov_alpha);
                        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);
                    }

                    // Left accent strip (selected) with smooth scale/fade
                    if list_sel_factor > 0.001 {
                        let strip_w = 2.5 * list_sel_factor;
                        let strip_alpha = (255.0 * list_sel_factor) as u8;
                        let strip_col = Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), strip_alpha);
                        ui.painter().rect_filled(
                            Rect::from_min_size(rect.min, Vec2::new(strip_w, rect.height())),
                            Rounding::same(1.0),
                            strip_col,
                        );
                    }

                    // Geri alma / aktarım sonrası parıldama efekti (Glow pulse)
                    if let Some(pulse) = state.get_highlight_pulse(&entry.path) {
                        let glow_col = Color32::from_rgba_unmultiplied(
                            c.accent().r(), c.accent().g(), c.accent().b(), (180.0 * pulse) as u8
                        );
                        ui.painter().rect_stroke(
                            rect.expand(1.0),
                            Rounding::same(s.radius_sm + 1.0),
                            Stroke::new(2.0_f32, glow_col),
                        );
                        ui.ctx().request_repaint();
                    }

                    let row_y = rect.min.y + row_h / 2.0;

                    // Dinamik boyutlandırma: Row yüksekliğine göre ikon/thumbnail boyutu ve yazı başlangıç konumu
                    let th_sz = (row_h - 4.0).clamp(16.0, 72.0);
                    let icon_display_sz = (row_h * 0.60).clamp(15.0, 48.0);
                    let icon_cx = rect.min.x + sp.md + 4.0 + (th_sz.max(icon_display_sz) / 2.0);

                    let ext = entry.extension.to_lowercase();
                    let is_media = !entry.is_dir && matches!(ext.as_str(),
                        "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"
                        |"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts"
                        |"mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma");
                    let mut drew_thumb = false;

                    if is_media && show_thumb {
                        let th_id  = egui::Id::new(format!("thumb_{}_lo", entry.path));
                        let handle: Option<egui::TextureHandle> =
                            ui.ctx().data_mut(|d| d.get_temp(th_id));
                        if let Some(ref h) = handle {
                            let tex_sz = h.size();
                            let aspect = if tex_sz[1] > 0 { tex_sz[0] as f32 / tex_sz[1] as f32 } else { 1.0 };
                            let (fit_w, fit_h) = if aspect >= 1.0 {
                                (th_sz, (th_sz / aspect).max(1.0))
                            } else {
                                ((th_sz * aspect).max(1.0), th_sz)
                            };
                            let tr = Rect::from_center_size(
                                Pos2::new(icon_cx, rect.center().y), Vec2::new(fit_w, fit_h));
                            ui.painter().image(h.id(), tr,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0,1.0)), Color32::WHITE);
                            drew_thumb = true;
                        } else {
                            // Load low-quality thumb
                            use crate::backend::thumbnails::{get_thumbnail, jpeg_to_color_image, ThumbQuality};
                            let req = egui::Id::new(format!("lstreq_{}", entry.path));
                            let loading: bool = ui.ctx().data_mut(|d| d.get_temp(req).unwrap_or(false));
                            if !loading {
                                ui.ctx().data_mut(|d| d.insert_temp(req, true));
                                let p = entry.path.clone(); let ctx2 = ui.ctx().clone();
                                let tid = th_id;
                                std::thread::spawn(move || {
                                    if let Some(j) = get_thumbnail(&p, ThumbQuality::Low) {
                                        if let Some(ci) = jpeg_to_color_image(&j) {
                                            ctx2.data_mut(|d| d.insert_temp(egui::Id::new(format!("lstraw_{}", p)), (ci, p.clone())));
                                            ctx2.request_repaint();
                                        }
                                    }
                                });
                            }
                            let raw: Option<(egui::ColorImage, String)> =
                                ui.ctx().data_mut(|d| d.get_temp(egui::Id::new(format!("lstraw_{}", entry.path))));
                            if let Some((ci, p)) = raw {
                                let h = ui.ctx().load_texture(format!("lthumb_{}", p), ci, egui::TextureOptions::LINEAR);
                                ui.ctx().data_mut(|d| {
                                    let th_key = format!("thumb_{}_lo", p);
                                    d.insert_temp(egui::Id::new(&th_key), h);
                                    d.remove::<(egui::ColorImage, String)>(egui::Id::new(format!("lstraw_{}", p)));

                                    let lru_id = egui::Id::new("list_thumb_lru_keys");
                                    let mut lru: std::collections::VecDeque<String> = d.get_temp(lru_id).unwrap_or_default();
                                    lru.push_back(th_key);
                                    while lru.len() > 300 {
                                        if let Some(old_key) = lru.pop_front() {
                                            d.remove::<egui::TextureHandle>(egui::Id::new(&old_key));
                                        }
                                    }
                                    d.insert_temp(lru_id, lru);
                                });
                                ui.ctx().request_repaint();
                            }
                        }
                    }

                    if !drew_thumb {
                        ui.painter().text(
                            Pos2::new(icon_cx, rect.center().y),
                            Align2::CENTER_CENTER,
                            file_icon(t, entry),
                            FontId::new(icon_display_sz, FontFamily::Monospace),
                            icon_color_for(t, entry),
                        );
                    }

                    // Name or Rename Edit
                    // Yazı başlangıç x konumu thumbnail/ikon genişliğine göre sağa kayar!
                    let name_x = actual_name_x;

                    if is_renaming {
                        let te_w = (rect.max.x - actual_name_x - right_space).max(100.0);
                        let te_rect = Rect::from_min_size(
                            Pos2::new(actual_name_x, rect.min.y + 4.0),
                            Vec2::new(te_w, row_h - 8.0),
                        );
                        let te_id = Id::new(format!("list_rename_{}", entry.path));
                        let te_resp = ui.put(
                            te_rect,
                            egui::TextEdit::singleline(&mut state.rename_buf)
                                .id(te_id)
                                .font(FontId::new(t.typography.size_base, FontFamily::Proportional))
                                .desired_width(te_w)
                                .frame(true),
                        );
                        if state.rename_just_started {
                            te_resp.request_focus();
                            let char_len = state.rename_buf.chars().count();
                            let select_len = if entry.is_dir {
                                char_len
                            } else if let Some(dot_idx) = state.rename_buf.rfind('.') {
                                dot_idx
                            } else {
                                char_len
                            };
                            let mut te_state = egui::text_edit::TextEditState::load(ui.ctx(), te_id).unwrap_or_default();
                            te_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(select_len),
                            )));
                            te_state.store(ui.ctx(), te_id);
                            state.rename_just_started = false;
                        }

                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            state.commit_rename();
                        } else if te_resp.lost_focus() {
                            if ui.input(|i| i.key_pressed(Key::Escape)) {
                                state.cancel_rename();
                            } else {
                                state.commit_rename();
                            }
                        }
                    } else {
                        // Max visible characters for Name
                        let max_name_w = (rect.max.x - actual_name_x - right_space).max(40.0);
                        let mut name_text = entry.name.clone();
                        // Truncate if overly long for available name width
                        let approx_char_w = 8.0_f32;
                        let max_chars = (max_name_w / approx_char_w).floor() as usize;
                        if name_text.chars().count() > max_chars && max_chars > 3 {
                            let truncated: String = name_text.chars().take(max_chars.saturating_sub(1)).collect();
                            name_text = format!("{}…", truncated);
                        }

                        let name_font_sz = (t.typography.size_base * (row_h / 26.0).clamp(0.9, 1.4)).clamp(11.0, 16.0);
                        ui.painter().text(
                            Pos2::new(actual_name_x, rect.min.y + row_h / 2.0),
                            Align2::LEFT_CENTER,
                            &name_text,
                            FontId::new(name_font_sz, FontFamily::Proportional),
                            if is_sel { c.text_primary() } else { c.text_secondary() },
                        );

                        // Symlink rozeti: Arch-Explorer Tauri gibi "⤷" işareti
                        if entry.is_symlink {
                            let text_len = name_text.chars().count() as f32;
                            let sym_x = actual_name_x + (text_len * name_font_sz * 0.58) + 6.0;
                            ui.painter().text(
                                Pos2::new(sym_x, rect.min.y + row_h / 2.0),
                                Align2::LEFT_CENTER,
                                "⤷",
                                FontId::new(name_font_sz * 1.1, FontFamily::Monospace),
                                c.accent(),
                            );
                        }
                    }

                    // Git status badge
                    if let Some((b_str, b_col)) = git_badge_for(state, entry) {
                        let badge_x = rect.max.x - right_space - 10.0;
                        let b_rect = Rect::from_center_size(
                            Pos2::new(badge_x, rect.min.y + row_h / 2.0),
                            Vec2::new(14.0, 14.0),
                        );
                        ui.painter().rect_filled(b_rect, Rounding::same(3.0), b_col);
                        ui.painter().text(
                            b_rect.center(),
                            Align2::CENTER_CENTER,
                            b_str,
                            FontId::new(9.0, FontFamily::Proportional),
                            Color32::from_rgb(15, 15, 20),
                        );
                    }

                    // Dynamically position columns based on state.list_columns.order:
                    // Sağ kenardan 24px ofset ile Modified'ı daha sola alarak ferah alan sağlıyoruz
                    let mut cur_right = rect.max.x - 24.0;
                    for col_name in state.list_columns.order.iter().rev() {
                        match col_name.as_str() {
                            "modified" if state.list_columns.show_modified => {
                                ui.painter().text(
                                    Pos2::new(cur_right, rect.min.y + row_h / 2.0),
                                    Align2::RIGHT_CENTER,
                                    &entry.modified,
                                    FontId::new(t.typography.size_sm, FontFamily::Monospace),
                                    c.text_hint(),
                                );
                                cur_right -= mod_w + 12.0;
                            }
                            "accessed" if state.list_columns.show_accessed => {
                                let acc_str = if !entry.created.is_empty() { &entry.created } else { &entry.modified };
                                ui.painter().text(
                                    Pos2::new(cur_right, rect.min.y + row_h / 2.0),
                                    Align2::RIGHT_CENTER,
                                    acc_str,
                                    FontId::new(t.typography.size_sm, FontFamily::Monospace),
                                    c.text_hint(),
                                );
                                cur_right -= state.list_columns.accessed_w.clamp(80.0, 400.0) + 12.0;
                            }
                            "perm" if state.list_columns.show_perm => {
                                ui.painter().text(
                                    Pos2::new(cur_right, rect.min.y + row_h / 2.0),
                                    Align2::RIGHT_CENTER,
                                    &entry.permissions,
                                    FontId::new(t.typography.size_sm, FontFamily::Monospace),
                                    c.text_hint(),
                                );
                                cur_right -= perm_w + 12.0;
                            }
                            "size" if state.list_columns.show_size => {
                                let size_display = if entry.is_dir {
                                    if let Some(&sz) = state.du_sizes.get(&entry.path) {
                                        human_size(sz)
                                    } else if let Some(sz) = crate::backend::du_cache::get_cached_size(&entry.path) {
                                        human_size(sz)
                                    } else if let Some(cnt) = crate::backend::du_cache::get_dir_item_count(&entry.path) {
                                        format!("{} öğe", cnt)
                                    } else {
                                        crate::backend::du_cache::request_dir_item_count(&entry.path, ui.ctx().clone());
                                        "…".into()
                                    }
                                } else {
                                    human_size(entry.size)
                                };
                                ui.painter().text(
                                    Pos2::new(cur_right, rect.min.y + row_h / 2.0),
                                    Align2::RIGHT_CENTER,
                                    size_display,
                                    FontId::new(t.typography.size_sm, FontFamily::Proportional),
                                    c.text_hint(),
                                );
                                cur_right -= size_w + 12.0;
                            }
                            "type" if state.list_columns.show_type => {
                                let type_str = if entry.is_dir {
                                    "Klasör".to_string()
                                } else if !entry.extension.is_empty() {
                                    format!("{} Dosyası", entry.extension.to_uppercase())
                                } else {
                                    "Dosya".to_string()
                                };
                                ui.painter().text(
                                    Pos2::new(cur_right, rect.min.y + row_h / 2.0),
                                    Align2::RIGHT_CENTER,
                                    type_str,
                                    FontId::new(t.typography.size_sm, FontFamily::Proportional),
                                    c.text_hint(),
                                );
                                cur_right -= state.list_columns.type_w.clamp(50.0, 250.0) + 12.0;
                            }
                            _ => {}
                        }
                    }

                    // Row separator
                    ui.painter().hline(
                        rect.left()..=rect.right(),
                        rect.bottom(),
                        Stroke::new(0.5_f32, c.border()),
                    );
                }

                // Calculate item hit area (Icon + Name label + extended right area so clicking isn't missed)
                let text_len = entry.name.chars().count().min(40) as f32;
                let text_w = (text_len * 9.0).clamp(32.0, (rect.max.x - actual_name_x - right_space).max(32.0));
                let hit_max_x = (actual_name_x + text_w + 180.0).min(rect.max.x - 12.0).max(actual_name_x + text_w + 24.0);
                let hit_area = Rect::from_min_max(
                    Pos2::new(rect.min.x + sp.md, rect.min.y),
                    Pos2::new(hit_max_x, rect.max.y),
                );
                current_frame_rects.push(ListHitArea {
                    row_rect: rect,
                    item_hit: hit_area,
                });

                let click_pos = resp.interact_pointer_pos()
                    .or_else(|| ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.press_origin()).or(i.pointer.latest_pos())));
                let press_origin = ui.ctx().input(|i| i.pointer.press_origin());
                let clicked_on_item = click_pos.map_or(false, |p| hit_area.contains(p));
                let drag_originated_on_item = press_origin.map_or(false, |p| hit_area.contains(p));

                if !state.drag.dragging && !is_renaming {
                    attach_delayed_hover_tooltip(&resp, ui, t, entry, is_sel);
                }

                // Interactions
                if resp.clicked() && !is_renaming && !state.is_rubber_banding {
                    let mods = ui.input(|i| i.modifiers);
                    let is_ctrl = mods.ctrl || mods.command;
                    let is_shift = mods.shift;
                    if clicked_on_item {
                        if is_ctrl && !is_shift {
                            state.selection.toggle(entry.path.clone());
                        } else if is_shift {
                            let anchor = state.selection.anchor.clone()
                                .or_else(|| state.selection.last_clicked.clone())
                                .unwrap_or_else(|| entry.path.clone());
                            state.selection.select_range(entries, &anchor, &entry.path, is_ctrl);
                        } else {
                            state.selection.select_only(entry.path.clone());
                        }
                    } else if !is_ctrl && !is_shift {
                        state.selection.clear();
                    }
                }

                if resp.double_clicked() && !is_renaming && clicked_on_item { open_entry(state, entry); }

                if resp.secondary_clicked() {
                    let pos = ui.ctx().pointer_interact_pos()
                        .or_else(|| ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()).or(i.pointer.press_origin())))
                        .unwrap_or_else(|| rect.center());
                    if clicked_on_item {
                        if !state.selection.is_selected_at(&entry.path, idx) {
                            state.selection.select_only(entry.path.clone());
                        }
                        state.panels.context_menu = Some(ContextMenuState {
                            x: pos.x, y: pos.y, target: Some(entry.clone()),
                        });
                    } else {
                        // Nemo-style: Boş alana sağ tıklanırsa tuval bağlam menüsü
                        state.panels.context_menu = Some(ContextMenuState {
                            x: pos.x, y: pos.y, target: None,
                        });
                    }
                }

                // Drag start — if not selected, select it immediately and start dragging
                if resp.drag_started() && drag_originated_on_item && !state.is_rubber_banding {
                    if !state.selection.is_selected_at(&entry.path, idx) {
                        state.selection.select_only(entry.path.clone());
                    }
                    let sources = state.selection.paths();
                    let copy = ui.input(|i| i.modifiers.ctrl);
                    // Sürükle-bırak panoyu (clipboard) kirletmez; doğrudan yerel Wayland DnD motorunu başlatır
                    spawn_native_drag(&sources);
                    state.drag.start(sources, copy);
                }

                // Drop target handling for folders
                let is_pointer_over = ui.ctx().pointer_interact_pos()
                    .or_else(|| ui.ctx().pointer_hover_pos())
                    .map(|p| rect.contains(p))
                    .unwrap_or(false);

                if state.drag.dragging && entry.is_dir && is_pointer_over {
                    state.drag.target_dir = Some(entry.path.clone());
                    // Highlighting for drop target
                    ui.painter().rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(2.0_f32, c.accent()));
                    ui.painter().rect_filled(rect, Rounding::same(s.radius_sm),
                        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 50));

                    if ui.input(|i| i.pointer.any_released()) {
                        let dest = entry.path.clone();
                        let sources = state.drag.sources.clone();
                        let copy = state.drag.copy_mode;
                        state.drag.cancel();
                        if !sources.contains(&dest) {
                            state.start_transfer(sources, dest, copy);
                        }
                    }
                }
            }

            let bottom_spacer = (total_count.saturating_sub(end_idx)) as f32 * row_h;
            if bottom_spacer > 0.0 {
                ui.add_space(bottom_spacer);
            }

            // Update rubber band selection using direct screen-space coordinates matching the visual box exactly
            if let Some(rb_rect) = active_rb_screen_rect {
                let ctrl_active = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let shift_active = ui.input(|i| i.modifiers.shift);
                let initial_sel: std::collections::HashSet<String> = ui.ctx().data_mut(|d| d.get_temp(rb_init_sel_id)).unwrap_or_default();

                let mut next_sel = if ctrl_active || shift_active {
                    initial_sel
                } else {
                    std::collections::HashSet::new()
                };

                for (item, entry) in current_frame_rects.iter().zip(rendered_entries.iter()) {
                    if rb_rect.intersects(item.row_rect) {
                        next_sel.insert(entry.path.clone());
                    }
                }
                state.selection.selected = next_sel;
            }

            ui.ctx().data_mut(|d| d.insert_temp(rects_cache_id, current_frame_rects));

            // Allocate empty background area at the bottom so user can always click in empty space to deselect / context menu
            let avail_h = ui.available_height().max(100.0);
            let (_empty_rect, empty_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), avail_h), Sense::click());
            if empty_resp.clicked() && !ui.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift) {
                state.selection.clear();
            }
            if empty_resp.secondary_clicked() {
                if let Some(pos) = ui.ctx().pointer_interact_pos() {
                    state.panels.context_menu = Some(ContextMenuState { x: pos.x, y: pos.y, target: None });
                }
            }
        });
}

fn list_headers(ui: &mut Ui, t: &AexTheme, state: &mut AppState) {
    let w = ui.available_width().max(200.0);
    list_headers_with_width(ui, t, state, w);
}

fn list_headers_with_width(ui: &mut Ui, t: &AexTheme, state: &mut AppState, w: f32) {
    let c = &t.colors;

    let prefix_w = t.spacing.md + 26.0;
    let size_w = if state.list_columns.show_size { state.list_columns.size_w.clamp(60.0, 300.0) } else { 0.0 };
    let perm_w = if state.list_columns.show_perm { state.list_columns.perm_w.clamp(60.0, 300.0) } else { 0.0 };
    let mod_w = if state.list_columns.show_modified { state.list_columns.modified_w.clamp(80.0, 400.0) } else { 0.0 };
    let type_w = if state.list_columns.show_type { state.list_columns.type_w.clamp(50.0, 250.0) } else { 0.0 };
    let acc_w = if state.list_columns.show_accessed { state.list_columns.accessed_w.clamp(80.0, 400.0) } else { 0.0 };

    let fixed_right = size_w + perm_w + mod_w + type_w + acc_w + 32.0;
    let name_w = (w - prefix_w - fixed_right).max(100.0);

    let mut changed = false;

    let (head_rect, _) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
    let mut header_ui = ui.new_child(egui::UiBuilder::new().max_rect(head_rect));

    let order = state.list_columns.order.clone();

    header_ui.spacing_mut().item_spacing.x = 0.0;
    header_ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(prefix_w);

        // 1. Name column
        sort_header(ui, t, state, "Name", crate::state::SortBy::Name, name_w, Align2::LEFT_CENTER, None);

        // Columns according to dynamic order:
        for col in &order {
            match col.as_str() {
                "type" if state.list_columns.show_type => {
                    let sep = column_resizer(ui, t, "resizer_type");
                    if let Some(delta) = sep {
                        state.list_columns.type_w = (state.list_columns.type_w - delta).clamp(50.0, 250.0);
                        changed = true;
                    }
                    if sort_header(ui, t, state, "Type", crate::state::SortBy::Name, type_w, Align2::RIGHT_CENTER, Some("type")) {
                        changed = true;
                    }
                }
                "size" if state.list_columns.show_size => {
                    let sep = column_resizer(ui, t, "resizer_size");
                    if let Some(delta) = sep {
                        state.list_columns.size_w = (state.list_columns.size_w - delta).clamp(60.0, 300.0);
                        changed = true;
                    }
                    if sort_header(ui, t, state, "Size", crate::state::SortBy::Size, size_w, Align2::RIGHT_CENTER, Some("size")) {
                        changed = true;
                    }
                }
                "perm" if state.list_columns.show_perm => {
                    let sep = column_resizer(ui, t, "resizer_perm");
                    if let Some(delta) = sep {
                        state.list_columns.perm_w = (state.list_columns.perm_w - delta).clamp(60.0, 300.0);
                        changed = true;
                    }
                    if sort_header(ui, t, state, "Permissions", crate::state::SortBy::Permissions, perm_w, Align2::RIGHT_CENTER, Some("perm")) {
                        changed = true;
                    }
                }
                "accessed" if state.list_columns.show_accessed => {
                    let sep = column_resizer(ui, t, "resizer_acc");
                    if let Some(delta) = sep {
                        state.list_columns.accessed_w = (state.list_columns.accessed_w - delta).clamp(80.0, 400.0);
                        changed = true;
                    }
                    if sort_header(ui, t, state, "Accessed", crate::state::SortBy::Modified, acc_w, Align2::RIGHT_CENTER, Some("accessed")) {
                        changed = true;
                    }
                }
                "modified" if state.list_columns.show_modified => {
                    let sep = column_resizer(ui, t, "resizer_mod");
                    if let Some(delta) = sep {
                        state.list_columns.modified_w = (state.list_columns.modified_w - delta).clamp(80.0, 400.0);
                        changed = true;
                    }
                    if sort_header(ui, t, state, "Modified", crate::state::SortBy::Modified, mod_w, Align2::RIGHT_CENTER, Some("modified")) {
                        changed = true;
                    }
                }
                _ => {}
            }
        }

        // Right-edge resizer for the last column (e.g. Modified) so user can drag left/right freely
        let sep_end = column_resizer(ui, t, "resizer_end");
        if let Some(delta) = sep_end {
            if let Some(last_col) = order.last() {
                match last_col.as_str() {
                    "modified" => state.list_columns.modified_w = (state.list_columns.modified_w + delta).clamp(80.0, 400.0),
                    "size" => state.list_columns.size_w = (state.list_columns.size_w + delta).clamp(60.0, 300.0),
                    "perm" => state.list_columns.perm_w = (state.list_columns.perm_w + delta).clamp(60.0, 300.0),
                    "type" => state.list_columns.type_w = (state.list_columns.type_w + delta).clamp(50.0, 250.0),
                    "accessed" => state.list_columns.accessed_w = (state.list_columns.accessed_w + delta).clamp(80.0, 400.0),
                    _ => {}
                }
                changed = true;
            }
        }
    });

    if changed {
        state.save_list_columns();
    }

    ui.painter().hline(
        head_rect.left()..=head_rect.right(),
        head_rect.bottom(),
        Stroke::new(1.0_f32, c.border()),
    );
}

fn column_resizer(ui: &mut Ui, t: &AexTheme, id_str: &str) -> Option<f32> {
    let c = &t.colors;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(8.0, 24.0), Sense::click_and_drag());
    let is_active = resp.hovered() || resp.dragged();

    // Görünür, belirgin ayırıcı çizgi: normalde hafif sınır rengi, hover/drag durumunda parlak accent rengi
    let line_col = if is_active {
        c.accent()
    } else {
        Color32::from_rgba_unmultiplied(c.border().r(), c.border().g(), c.border().b(), 160)
    };
    let line_w = if is_active { 2.5_f32 } else { 1.5_f32 };

    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(line_w, line_col));

    if is_active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
        ui.ctx().data_mut(|d| d.insert_temp(Id::new("is_resizing_column"), true));
    } else if ui.input(|i| !i.pointer.any_down()) {
        ui.ctx().data_mut(|d| d.remove::<bool>(Id::new("is_resizing_column")));
    }
    if resp.dragged() {
        Some(resp.drag_delta().x)
    } else {
        None
    }
}

fn sort_header(
    ui: &mut Ui,
    t: &AexTheme,
    state: &mut AppState,
    label: &str,
    sort: crate::state::SortBy,
    width: f32,
    align: Align2,
    col_key: Option<&str>,
) -> bool {
    let c = &t.colors;
    let is_active = state.dir.sort_by == sort;
    let mut order_changed = false;

    let text = if is_active {
        format!("{} {}", label, if state.dir.sort_asc { "↑" } else { "↓" })
    } else {
        label.to_string()
    };

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(20.0), 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(3.0), Color32::from_rgba_unmultiplied(255, 255, 255, 10));
    }

    let pos = match align {
        Align2::LEFT_CENTER => Pos2::new(rect.min.x + 2.0, rect.center().y),
        Align2::RIGHT_CENTER => Pos2::new(rect.max.x - 2.0, rect.center().y),
        _ => rect.center(),
    };

    ui.painter().text(
        pos,
        align,
        text,
        FontId::new(t.typography.size_xs, FontFamily::Proportional),
        if is_active { c.accent() } else { c.text_hint() },
    );

    // Left/Right reorder buttons on hover for custom columns
    if let Some(col) = col_key {
        if resp.hovered() && rect.width() > 50.0 {
            let btn_size = Vec2::new(14.0, 14.0);
            let btn_left_rect = Rect::from_center_size(Pos2::new(rect.min.x + 10.0, rect.center().y), btn_size);
            let btn_right_rect = Rect::from_center_size(Pos2::new(rect.min.x + 26.0, rect.center().y), btn_size);

            if let Some(ptr) = ui.input(|i| i.pointer.interact_pos()) {
                if btn_left_rect.contains(ptr) && ui.input(|i| i.pointer.primary_clicked()) {
                    state.list_columns.move_col_left(col);
                    order_changed = true;
                } else if btn_right_rect.contains(ptr) && ui.input(|i| i.pointer.primary_clicked()) {
                    state.list_columns.move_col_right(col);
                    order_changed = true;
                }
            }

            ui.painter().text(btn_left_rect.center(), Align2::CENTER_CENTER, "◀", FontId::new(9.0, FontFamily::Proportional), c.text_secondary());
            ui.painter().text(btn_right_rect.center(), Align2::CENTER_CENTER, "▶", FontId::new(9.0, FontFamily::Proportional), c.text_secondary());
        }
    }

    if resp.clicked() && !order_changed {
        if is_active {
            state.dir.sort_asc = !state.dir.sort_asc;
        } else {
            state.dir.sort_by = sort;
            state.dir.sort_asc = true;
        }
        state.dir.sort();
    }

    order_changed
}

// ─────────────────────────────────────────────────────────────────────────────
// Compact View (Nemo-style multi-column layout)
// ─────────────────────────────────────────────────────────────────────────────

fn compact_view(ui: &mut Ui, t: &AexTheme, state: &mut AppState, entries: &[FileEntry], pane_tag: &str) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let available_w = ui.available_width();
    let col_w_base = (220.0 * state.view_scale).clamp(160.0, 360.0);
    let row_h = (28.0 * (state.view_scale * 0.4 + 0.6)).clamp(22.0, 42.0);
    let col_gap = 10.0;
    let row_gap = 2.0;

    let cols = ((available_w + col_gap) / (col_w_base + col_gap)).max(1.0) as usize;
    let actual_col_w = ((available_w - (cols.saturating_sub(1) as f32 * col_gap)) / cols as f32).max(120.0);

    let scroll_id = Id::new(format!("file_compact_{}", pane_tag)).with(&state.nav.current_path);

    egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .auto_shrink([false; 2])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            let bg_rect = ui.clip_rect();

            // Deselect / context menu via ctx.input
            let ptr_clicked = ui.ctx().input(|i| i.pointer.primary_clicked());
            let ptr_sec = ui.ctx().input(|i| i.pointer.secondary_clicked());
            let ptr_pos = ui.ctx().input(|i| i.pointer.interact_pos());
            let ptr_in_bg = ptr_pos.map(|p| bg_rect.contains(p)).unwrap_or(false);

            // Rubber-band drag state & selection
            let rb_id = Id::new(format!("rubber_band_start_compact_{}", pane_tag));
            let is_primary_down = ui.ctx().input(|i| i.pointer.primary_down());
            let press_origin = ui.ctx().input(|i| i.pointer.press_origin());

            let rects_cache_id = Id::new(format!("rendered_rects_compact_{}", pane_tag));
            let prev_rects: Vec<GridHitArea> = ui.ctx().data_mut(|d| d.get_temp(rects_cache_id)).unwrap_or_default();
            let pressed_on_item = press_origin.map(|origin| prev_rects.iter().any(|hit| hit.contains(origin))).unwrap_or(false);

            let is_resizing_preview: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_preview")).unwrap_or(false));
            let is_resizing_sidebar: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_sidebar")).unwrap_or(false));
            let is_resizing_column: bool = ui.ctx().data_mut(|d| d.get_temp(Id::new("is_resizing_column")).unwrap_or(false));

            let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            let shift = ui.input(|i| i.modifiers.shift);

            if ptr_clicked && ptr_in_bg && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column && !ctrl && !shift {
                state.selection.clear();
            }
            if ptr_sec && ptr_in_bg && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column {
                if let Some(pos) = ptr_pos {
                    state.panels.context_menu = Some(ContextMenuState { x: pos.x, y: pos.y, target: None });
                }
            }

            let content_top_y = ui.cursor().min.y;
            let rb_rel_y_id = Id::new(format!("rubber_band_rel_y_compact_{}", pane_tag));
            let rb_init_sel_id = Id::new(format!("rubber_band_init_sel_compact_{}", pane_tag));

            if is_primary_down && !state.drag.dragging && !pressed_on_item && !is_resizing_preview && !is_resizing_sidebar && !is_resizing_column {
                if let (Some(origin), Some(cur_pos)) = (press_origin, ptr_pos) {
                    if bg_rect.contains(origin) {
                        let drag_delta = cur_pos - origin;
                        if drag_delta.length() > 4.0 {
                            let already_active = ui.ctx().data_mut(|d| d.get_temp::<Pos2>(rb_id).is_some());
                            if !already_active {
                                let start_rel_y = origin.y - content_top_y;
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(rb_id, origin);
                                    d.insert_temp(rb_rel_y_id, start_rel_y);
                                    d.insert_temp(rb_init_sel_id, state.selection.selected.clone());
                                });
                            }
                            state.is_rubber_banding = true;
                        }
                    }
                }
            } else if !is_primary_down || state.drag.dragging || is_resizing_preview {
                ui.ctx().data_mut(|d| {
                    d.remove::<Pos2>(rb_id);
                    d.remove::<f32>(rb_rel_y_id);
                    d.remove::<std::collections::HashSet<String>>(rb_init_sel_id);
                });
                state.is_rubber_banding = false;
            }

            let mut active_rb_rect: Option<Rect> = None;
            if let (Some(start), Some(start_rel_y)) = (
                ui.ctx().data_mut(|d| d.get_temp::<Pos2>(rb_id)),
                ui.ctx().data_mut(|d| d.get_temp::<f32>(rb_rel_y_id)),
            ) {
                if let Some(cur) = ptr_pos {
                    state.is_rubber_banding = true;

                    // Auto-scroll near edge
                    if cur.y < bg_rect.min.y + 45.0 {
                        let overflow = ((bg_rect.min.y + 45.0) - cur.y).max(0.0);
                        let scroll_speed = (overflow * 0.45 + 4.0).clamp(4.0, 36.0);
                        ui.scroll_with_delta(Vec2::new(0.0, scroll_speed));
                        ui.ctx().request_repaint();
                    } else if cur.y > bg_rect.max.y - 45.0 {
                        let overflow = (cur.y - (bg_rect.max.y - 45.0)).max(0.0);
                        let scroll_speed = (overflow * 0.45 + 4.0).clamp(4.0, 36.0);
                        ui.scroll_with_delta(Vec2::new(0.0, -scroll_speed));
                        ui.ctx().request_repaint();
                    }

                    let current_start_y = content_top_y + start_rel_y;
                    let effective_start = Pos2::new(start.x, current_start_y);
                    let rb_rect = Rect::from_two_pos(effective_start, cur);
                    active_rb_rect = Some(rb_rect);

                    let visible_box = rb_rect.intersect(bg_rect);
                    if visible_box.is_positive() {
                        let theme_accent = c.accent();
                        let rb_fill = Color32::from_rgba_unmultiplied(
                            theme_accent.r(), theme_accent.g(), theme_accent.b(), 35
                        );
                        let rb_stroke = theme_accent;

                        let fg_painter = ui.ctx().layer_painter(LayerId::new(Order::Foreground, Id::new("rubber_band_layer_compact")));
                        fg_painter.rect_filled(visible_box, Rounding::ZERO, rb_fill);
                        fg_painter.rect_stroke(visible_box, Rounding::ZERO, Stroke::new(1.0_f32, rb_stroke));
                    }
                }
            }

            let total_count = entries.len();
            let total_rows = (total_count + cols - 1) / cols;
            let mut current_frame_rects = Vec::with_capacity(total_count + 1);

            ui.spacing_mut().item_spacing = Vec2::ZERO;

            for row_idx in 0..total_rows {
                let row_start = row_idx * cols;
                let row_end = (row_start + cols).min(total_count);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(col_gap, 0.0);

                    for idx in row_start..row_end {
                        let entry = &entries[idx];
                        let is_sel = state.selection.is_selected_at(&entry.path, idx);
                        let is_renaming = state.rename_target.as_deref() == Some(&entry.path);

                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(actual_col_w, row_h),
                            Sense::click_and_drag(),
                        );

                        let item_rect_id = Id::new(format!("item_rect_{}", entry.path));
                        ui.ctx().data_mut(|d| d.insert_temp(item_rect_id, rect));

                        current_frame_rects.push(GridHitArea {
                            thumb_hit: rect,
                            label_hit: rect,
                        });

                        if ui.is_rect_visible(rect) {
                            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));

                            // Background highlight
                            if is_sel {
                                let sel_bg = Color32::from_rgba_unmultiplied(
                                    c.accent().r(), c.accent().g(), c.accent().b(), 40
                                );
                                painter.rect_filled(rect, Rounding::same(s.radius_sm), sel_bg);
                                painter.rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(1.0_f32, c.accent()));
                            } else if resp.hovered() {
                                painter.rect_filled(rect, Rounding::same(s.radius_sm), c.bg_surface0());
                            }

                            // Left: Icon (20-24px)
                            let icon_sz = (row_h - 6.0).clamp(18.0, 24.0);
                            let icon_rect = Rect::from_center_size(
                                Pos2::new(rect.min.x + 4.0 + icon_sz / 2.0, rect.center().y),
                                Vec2::splat(icon_sz),
                            );

                            // Media thumbnail in compact view if available
                            let ext = entry.extension.to_lowercase();
                            let is_media = !entry.is_dir && matches!(ext.as_str(),
                                "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"|"tiff"|"avif"
                                |"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts");
                            let mut drew_thumb = false;
                            if is_media {
                                let th_id = egui::Id::new(format!("thumb_{}_lo", entry.path));
                                let handle: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(th_id));
                                if let Some(ref h) = handle {
                                    let tex_sz = h.size();
                                    let aspect = if tex_sz[1] > 0 { tex_sz[0] as f32 / tex_sz[1] as f32 } else { 1.0 };
                                    let (fit_w, fit_h) = if aspect >= 1.0 {
                                        (icon_sz, (icon_sz / aspect).max(1.0))
                                    } else {
                                        ((icon_sz * aspect).max(1.0), icon_sz)
                                    };
                                    let tr = Rect::from_center_size(icon_rect.center(), Vec2::new(fit_w, fit_h));
                                    painter.rect_filled(tr, Rounding::same(s.radius_sm), Color32::from_black_alpha(25));
                                    painter.image(h.id(), tr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                    painter.rect_stroke(tr, Rounding::same(s.radius_sm), Stroke::new(1.0_f32, Color32::from_white_alpha(30)));
                                    drew_thumb = true;
                                } else {
                                    // Asynchronously request low-quality thumbnail
                                    use crate::backend::thumbnails::{get_thumbnail, jpeg_to_color_image, ThumbQuality};
                                    let req = egui::Id::new(format!("lstreq_{}", entry.path));
                                    let loading: bool = ui.ctx().data_mut(|d| d.get_temp(req).unwrap_or(false));
                                    if !loading {
                                        ui.ctx().data_mut(|d| d.insert_temp(req, true));
                                        let p = entry.path.clone();
                                        let ctx2 = ui.ctx().clone();
                                        std::thread::spawn(move || {
                                            if let Some(j) = get_thumbnail(&p, ThumbQuality::Low) {
                                                if let Some(ci) = jpeg_to_color_image(&j) {
                                                    ctx2.data_mut(|d| d.insert_temp(egui::Id::new(format!("lstraw_{}", p)), (ci, p.clone())));
                                                    ctx2.request_repaint();
                                                }
                                            }
                                        });
                                    }
                                    let raw: Option<(egui::ColorImage, String)> =
                                        ui.ctx().data_mut(|d| d.get_temp(egui::Id::new(format!("lstraw_{}", entry.path))));
                                    if let Some((ci, p)) = raw {
                                        let h = ui.ctx().load_texture(format!("lthumb_{}", p), ci, egui::TextureOptions::LINEAR);
                                        let tex_sz = h.size();
                                        let aspect = if tex_sz[1] > 0 { tex_sz[0] as f32 / tex_sz[1] as f32 } else { 1.0 };
                                        let (fit_w, fit_h) = if aspect >= 1.0 {
                                            (icon_sz, (icon_sz / aspect).max(1.0))
                                        } else {
                                            ((icon_sz * aspect).max(1.0), icon_sz)
                                        };
                                        let tr = Rect::from_center_size(icon_rect.center(), Vec2::new(fit_w, fit_h));
                                        painter.rect_filled(tr, Rounding::same(s.radius_sm), Color32::from_black_alpha(25));
                                        painter.image(h.id(), tr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                        painter.rect_stroke(tr, Rounding::same(s.radius_sm), Stroke::new(1.0_f32, Color32::from_white_alpha(30)));
                                        drew_thumb = true;

                                        ui.ctx().data_mut(|d| {
                                            let th_key = format!("thumb_{}_lo", p);
                                            d.insert_temp(egui::Id::new(&th_key), h);
                                            d.remove::<(egui::ColorImage, String)>(egui::Id::new(format!("lstraw_{}", p)));

                                            let lru_id = egui::Id::new("list_thumb_lru_keys");
                                            let mut lru: std::collections::VecDeque<String> = d.get_temp(lru_id).unwrap_or_default();
                                            lru.push_back(th_key);
                                            while lru.len() > 300 {
                                                if let Some(old_key) = lru.pop_front() {
                                                    d.remove::<egui::TextureHandle>(egui::Id::new(&old_key));
                                                }
                                            }
                                            d.insert_temp(lru_id, lru);
                                        });
                                        ui.ctx().request_repaint();
                                    }
                                }
                            }
                            if !drew_thumb {
                                painter.text(
                                    icon_rect.center(),
                                    Align2::CENTER_CENTER,
                                    file_icon(t, entry),
                                    FontId::monospace(icon_sz * 0.85),
                                    icon_color_for(t, entry),
                                );
                            }

                            // Right: subtle human-readable size
                            let size_font = FontId::proportional((row_h * 0.38).clamp(9.5, 11.5));
                            let size_text = if entry.is_dir {
                                if let Some(&sz) = state.du_sizes.get(&entry.path) {
                                    format_nemo_size(sz)
                                } else if let Some(sz) = crate::backend::du_cache::get_cached_size(&entry.path) {
                                    format_nemo_size(sz)
                                } else {
                                    String::new()
                                }
                            } else {
                                format_nemo_size(entry.size)
                            };

                            let size_w = if !size_text.is_empty() {
                                let galley = ui.painter().layout_no_wrap(size_text.clone(), size_font.clone(), c.text_hint());
                                let sw = galley.size().x;
                                painter.text(
                                    Pos2::new(rect.max.x - 6.0, rect.center().y),
                                    Align2::RIGHT_CENTER,
                                    size_text,
                                    size_font,
                                    c.text_hint(),
                                );
                                sw + 8.0
                            } else {
                                0.0
                            };

                            // Middle: Filename
                            let name_font = FontId::proportional((row_h * 0.42).clamp(10.5, 13.0));
                            let name_left = icon_rect.max.x + 6.0;
                            let name_right = rect.max.x - size_w - 6.0;
                            let avail_name_w = (name_right - name_left).max(20.0);

                            if is_renaming {
                                let te_rect = Rect::from_min_max(
                                    Pos2::new(name_left, rect.min.y + 2.0),
                                    Pos2::new(name_right, rect.max.y - 2.0),
                                );
                                let te_id = Id::new(format!("compact_rename_{}", entry.path));
                                let te_resp = ui.put(
                                    te_rect,
                                    egui::TextEdit::singleline(&mut state.rename_buf)
                                        .id(te_id)
                                        .font(name_font)
                                        .frame(true),
                                );
                                if state.rename_just_started {
                                    te_resp.request_focus();
                                    state.rename_just_started = false;
                                }
                                if ui.input(|i| i.key_pressed(Key::Enter)) {
                                    state.commit_rename();
                                } else if te_resp.lost_focus() {
                                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                                        state.cancel_rename();
                                    } else {
                                        state.commit_rename();
                                    }
                                }
                            } else {
                                let display_name = truncate_to_width(ui, &entry.name, &name_font, avail_name_w);
                                let name_color = if is_sel { c.accent() } else { c.text_primary() };
                                painter.text(
                                    Pos2::new(name_left, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    display_name,
                                    name_font,
                                    name_color,
                                );
                            }
                        }

                        if !state.drag.dragging && !is_renaming && resp.hovered() {
                            attach_delayed_hover_tooltip(&resp, ui, t, entry, is_sel);
                        }

                        // Interactions
                        if resp.clicked() && !is_renaming {
                            let modifiers = ui.input(|i| i.modifiers);
                            let is_ctrl = modifiers.ctrl || modifiers.command;
                            let is_shift = modifiers.shift;
                            if is_ctrl && !is_shift {
                                state.selection.toggle(entry.path.clone());
                            } else if is_shift {
                                let anchor = state.selection.anchor.clone()
                                    .or_else(|| state.selection.last_clicked.clone())
                                    .unwrap_or_else(|| entry.path.clone());
                                state.selection.select_range(entries, &anchor, &entry.path, is_ctrl);
                            } else {
                                state.selection.select_only(entry.path.clone());
                            }
                        }

                        if resp.double_clicked() && !is_renaming {
                            open_entry(state, entry);
                        }

                        if resp.secondary_clicked() {
                            let pos = ui.ctx().pointer_interact_pos().unwrap_or_else(|| rect.center());
                            if !state.selection.is_selected(&entry.path) {
                                state.selection.select_only(entry.path.clone());
                            }
                            state.panels.context_menu = Some(ContextMenuState {
                                x: pos.x, y: pos.y, target: Some(entry.clone()),
                            });
                        }

                        if resp.drag_started() {
                            if !state.selection.is_selected(&entry.path) {
                                state.selection.select_only(entry.path.clone());
                            }
                            let sources = state.selection.paths();
                            let copy = ui.input(|i| i.modifiers.ctrl);
                            spawn_native_drag(&sources);
                            state.drag.start(sources, copy);
                        }

                        // Drop target for folders
                        let is_pointer_over = ui.ctx().pointer_interact_pos()
                            .or_else(|| ui.ctx().pointer_hover_pos())
                            .map(|p| rect.contains(p))
                            .unwrap_or(false);

                        if state.drag.dragging && entry.is_dir && is_pointer_over {
                            state.drag.target_dir = Some(entry.path.clone());
                            ui.painter().rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(2.0_f32, c.accent()));
                            if ui.input(|i| i.pointer.any_released()) {
                                let dest = entry.path.clone();
                                let sources = state.drag.sources.clone();
                                let copy = state.drag.copy_mode;
                                state.drag.cancel();
                                if !sources.contains(&dest) {
                                    state.start_transfer(sources, dest, copy);
                                }
                            }
                        }
                    }
                });
                ui.add_space(row_gap);
            }

            // Update rubber band selection
            if let Some(rb_rect) = active_rb_rect {
                let ctrl_active = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let shift_active = ui.input(|i| i.modifiers.shift);
                let initial_sel: std::collections::HashSet<String> = ui.ctx().data_mut(|d| d.get_temp(rb_init_sel_id)).unwrap_or_default();

                let mut next_sel = if ctrl_active || shift_active {
                    initial_sel
                } else {
                    std::collections::HashSet::new()
                };

                for entry in entries {
                    let item_rect_id = Id::new(format!("item_rect_{}", entry.path));
                    if let Some(r) = ui.ctx().data_mut(|d| d.get_temp::<Rect>(item_rect_id)) {
                        if rb_rect.intersects(r) {
                            next_sel.insert(entry.path.clone());
                        }
                    }
                }
                state.selection.selected = next_sel;
            }

            ui.ctx().data_mut(|d| d.insert_temp(rects_cache_id, current_frame_rects));

            // Empty background click area
            let avail_h = ui.available_height().max(80.0);
            let (_empty_rect, empty_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), avail_h), Sense::click());
            if empty_resp.clicked() && !ui.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift) {
                state.selection.clear();
            }
            if empty_resp.secondary_clicked() {
                if let Some(pos) = ui.ctx().pointer_interact_pos() {
                    state.panels.context_menu = Some(ContextMenuState { x: pos.x, y: pos.y, target: None });
                }
            }
        });
}

pub fn spawn_native_drag(sources: &[String]) {
    if sources.is_empty() { return; }
    let preview = crate::backend::thumbnails::generate_drag_preview_bgra(sources);
    // Doğrudan yamalanmış winit Wayland veri aygıtı (SCTK) protokolünü dosyanın gerçek thumbnail'iyle tetikle!
    winit::start_drag_files(sources.to_vec(), preview);
}

/// Tek ikili (Single-binary) bünyesinde çalışan yerel Wayland & GTK drag-drop motoru
pub fn run_internal_dnd_worker(paths: &[String]) {
    if paths.is_empty() { return; }
    let paths_json = match serde_json::to_string(paths) {
        Ok(j) => j,
        Err(_) => return,
    };

    let script = r#"
import sys, os, json, urllib.parse, math
if "WAYLAND_DISPLAY" in os.environ and "GDK_BACKEND" not in os.environ:
    os.environ["GDK_BACKEND"] = "wayland,x11"
try:
    import gi
    gi.require_version('Gtk', '3.0')
    from gi.repository import Gtk, Gdk, GLib
    import cairo

    if not Gtk.init_check()[0]:
        sys.exit(0)

    paths = json.loads(sys.argv[1])
    if not paths:
        sys.exit(0)

    uris = [f"file://{urllib.parse.quote(os.path.abspath(p))}" for p in paths]
    uri_text = ("\r\n".join(uris) + "\r\n").encode('utf-8')
    gnome_text = ("copy\n" + "\n".join(uris) + "\n").encode('utf-8')
    plain_text = ("\n".join(paths) + "\n").encode('utf-8')

    win = Gtk.Window(type=Gtk.WindowType.POPUP)
    win.set_default_size(24, 24)

    targets = [
        Gtk.TargetEntry.new("text/uri-list", 0, 0),
        Gtk.TargetEntry.new("x-special/gnome-copied-files", 0, 1),
        Gtk.TargetEntry.new("text/plain", 0, 2),
        Gtk.TargetEntry.new("UTF8_STRING", 0, 3),
        Gtk.TargetEntry.new("DownloadURL", 0, 4),
        Gtk.TargetEntry.new("STRING", 0, 5)
    ]
    tlist = Gtk.TargetList.new(targets)

    def on_drag_data_get(widget, context, data, info, time):
        if info == 0:
            data.set_uris(uris)
        elif info == 1:
            atom = Gdk.Atom.intern("x-special/gnome-copied-files", False)
            data.set(atom, 8, gnome_text)
        elif info == 4:
            if paths:
                fname = os.path.basename(paths[0])
                dl = f"application/octet-stream:{fname}:{uris[0]}".encode('utf-8')
                atom = Gdk.Atom.intern("DownloadURL", False)
                data.set(atom, 8, dl)
        else:
            data.set_text(plain_text.decode('utf-8'), len(plain_text))

    def on_drag_end(widget, context):
        Gtk.main_quit()

    def on_drag_failed(widget, context, result):
        Gtk.main_quit()

    def on_drag_begin(widget, context):
        try:
            count = len(paths)
            first_name = os.path.basename(paths[0]) if paths else "Öğe"
            display_text = first_name[:24]
            if count > 1:
                display_text += f" (+{count - 1})"

            card_w = max(160, min(280, len(display_text) * 9 + 40))
            card_h = 36

            surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, card_w, card_h)
            cr = cairo.Context(surface)

            r = 8
            cr.arc(r, r, r, math.pi, 1.5 * math.pi)
            cr.arc(card_w - r, r, r, 1.5 * math.pi, 2 * math.pi)
            cr.arc(card_w - r, card_h - r, r, 0, 0.5 * math.pi)
            cr.arc(r, card_h - r, r, 0.5 * math.pi, math.pi)
            cr.close_path()

            # Arka plan: rgba(24, 24, 37, 0.95)
            cr.set_source_rgba(0.094, 0.094, 0.145, 0.95)
            cr.fill_preserve()

            # Kenarlık: rgba(130, 100, 255, 0.85)
            cr.set_source_rgba(0.51, 0.39, 1.0, 0.85)
            cr.set_line_width(1.5)
            cr.stroke()

            # İkon sembolü
            cr.set_source_rgba(0.51, 0.39, 1.0, 1.0)
            cr.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
            cr.set_font_size(13)
            cr.move_to(10, 23)
            cr.show_text("📄")

            # Dosya adı metni: #ffffff
            cr.set_source_rgba(1.0, 1.0, 1.0, 1.0)
            cr.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
            cr.set_font_size(11.5)
            cr.move_to(30, 23)
            cr.show_text(display_text)

            # Doğrudan Wayland donanım imleç katmanına bağla!
            Gtk.drag_set_icon_surface(context, surface)
        except Exception:
            pass

    win.connect("drag-begin", on_drag_begin)
    win.connect("drag-data-get", on_drag_data_get)
    win.connect("drag-end", on_drag_end)
    win.connect("drag-failed", on_drag_failed)
    win.show_all()

    # Max 15 saniye yaşama sınırı (zombi süreç oluşmasını kesin olarak engeller)
    GLib.timeout_add_seconds(15, Gtk.main_quit)

    disp = Gdk.Display.get_default()
    x, y = 0, 0
    if disp:
        seat = disp.get_default_seat()
        pointer = seat.get_pointer() if seat else None
        if pointer:
            _, x, y = pointer.get_position()
            win.move(x - 12, y - 12)

    ctx = Gtk.drag_begin_with_coordinates(win, tlist, Gdk.DragAction.COPY | Gdk.DragAction.MOVE, 1, None, x, y)
    if not ctx:
        Gtk.main_quit()
        sys.exit(0)

    # Context geldikten hemen sonra da bir kez garanti olarak ikonu ata
    try:
        on_drag_begin(win, ctx)
    except Exception:
        pass

    Gtk.main()
except Exception:
    sys.exit(0)
"#;

    if let Ok(mut child) = std::process::Command::new("python3")
        .args(["-c", script, &paths_json])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        let _ = child.wait();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn open_entry(state: &mut AppState, entry: &FileEntry) {
    let is_adb_folder = entry.path.starts_with("adb://") && (entry.is_symlink || entry.extension.is_empty());
    if entry.is_dir || is_adb_folder {
        state.navigate(entry.path.clone());
    } else {
        let ext = entry.extension.to_lowercase();
        let is_video = matches!(ext.as_str(), "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts");
        let is_audio = matches!(ext.as_str(), "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma");
        let is_windows = matches!(ext.as_str(), "exe" | "msi" | "bat");
        let is_script = !is_windows && matches!(ext.as_str(), "sh"|"bash"|"zsh");

        if entry.path.starts_with("adb://") {
            // ADB üzerindeki dosyayı /tmp altına geçici çekip aç
            if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(&entry.path) {
                let temp_dir = std::env::temp_dir().join("arch-explorer-adb");
                let _ = std::fs::create_dir_all(&temp_dir);
                let fname = std::path::Path::new(&dev_path)
                    .file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "temp_file".to_string());
                let local_target = temp_dir.join(&fname);
                let local_str = local_target.to_string_lossy().to_string();

                let mut cmd = std::process::Command::new("adb");
                cmd.args(["-s", &serial, "pull", &dev_path, &local_str]);
                if let Ok(out) = cmd.output() {
                    if out.status.success() {
                        if is_video || is_audio {
                            state.selection.select_only(entry.path.clone());
                            if state.panels.active != crate::state::ActivePanel::Preview {
                                state.panels.open_preview();
                            }
                            state.media_player.play(local_str, is_video);
                        } else {
                            std::process::Command::new("xdg-open")
                                .arg(&local_str)
                                .spawn()
                                .ok();
                        }
                    } else {
                        state.notify.error("Telefondan dosya açılamadı (ADB pull başarısız)".into());
                    }
                }
            }
        } else if is_video || is_audio {
            // Panel içinde doğrudan oynat!
            state.selection.select_only(entry.path.clone());
            if state.panels.active != crate::state::ActivePanel::Preview {
                state.panels.open_preview();
            }
            state.media_player.play(entry.path.clone(), is_video);
        } else if is_windows {
            // Windows çalıştırılabilir dosyalarını Wine ile doğrudan arka planda çalıştır (terminal açmadan!)
            let path_clone = entry.path.clone();
            std::thread::spawn(move || {
                let _ = crate::backend::open_with::open_with_wine(&path_clone);
            });
            state.notify.info(format!("Wine ile başlatılıyor: {}", entry.name));
        } else if is_script {
            // Shell komut dosyalarını doğrudan arka planda detached olarak çalıştır
            let p = std::path::Path::new(&entry.path);
            let parent_dir = p.parent().map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|| state.active_pane_dir());
            let script_path = entry.path.clone();
            std::thread::spawn(move || {
                use std::process::{Command, Stdio};
                let _ = Command::new("bash")
                    .arg(&script_path)
                    .current_dir(&parent_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            });
            state.notify.info(format!("Başlatıldı: {}", entry.name));
        } else {
            // Diğer dosyaları xdg-open ile aç
            std::process::Command::new("xdg-open")
                .arg(&entry.path)
                .spawn()
                .ok();
        }
    }
}

fn icon_color_for(t: &AexTheme, entry: &FileEntry) -> Color32 {
    crate::ui::icon_color_for_entry(t, entry)
}

/// Piksel ölçümlü veya özel genişlik ölçümlü çok satırlı dosya ismi sarma fonksiyonu.
/// - Ara satırlarda ASLA '…' bulunmaz.
/// - Sadece ve sadece son satırda (eğer kalan metin sığmıyorsa) '…' eklenir.
/// - Her satırın genişliği `measure` fonksiyonu ile `max_width` sınırına göre kontrol edilir.
pub fn wrap_name_multiline_with_measure<F: Fn(&str) -> f32>(
    name: &str,
    max_lines: usize,
    max_width: f32,
    measure: F,
) -> Vec<String> {
    let max_lines = max_lines.max(1);
    if name.is_empty() || max_width < 1.0 {
        return vec![name.to_string()];
    }

    // Eğer ismin tamamı sığıyorsa doğrudan tek satır olarak döndür
    if measure(name) <= max_width {
        return vec![name.to_string()];
    }

    let chars: Vec<char> = name.chars().collect();
    let mut lines = Vec::new();
    let mut cursor = 0;

    while cursor < chars.len() && lines.len() < max_lines {
        let remaining_len = chars.len() - cursor;
        let is_last_line = lines.len() + 1 == max_lines;
        let remaining_str: String = chars[cursor..].iter().collect();

        // Kalan kısım doğrudan sığıyorsa ekle ve bitir
        if measure(&remaining_str) <= max_width {
            lines.push(remaining_str);
            break;
        }

        // Son satır ise: Sığmıyor, Nemo gibi '…' ile son satırı kes
        if is_last_line {
            let mut low = 0;
            let mut high = remaining_len;
            let mut best = String::from("…");
            while low <= high {
                let mid = (low + high) / 2;
                let candidate: String = format!("{}…", chars[cursor..cursor + mid].iter().collect::<String>());
                if measure(&candidate) <= max_width {
                    best = candidate;
                    low = mid + 1;
                } else {
                    if mid == 0 { break; }
                    high = mid - 1;
                }
            }
            lines.push(best);
            break;
        }

        // Ara satır: Kesinlikle '…' ALAMAZ!
        // max_width içine sığabilecek en uzun karakter sayısını ikili aramayla bul
        let mut low = 1;
        let mut high = remaining_len;
        let mut best_fit = 1;
        while low <= high {
            let mid = (low + high) / 2;
            let candidate: String = chars[cursor..cursor + mid].iter().collect();
            if measure(&candidate) <= max_width {
                best_fit = mid;
                low = mid + 1;
            } else {
                if mid == 0 { break; }
                high = mid - 1;
            }
        }

        // Bulunan best_fit içinde uygun bir kelime/karakter ayırıcı (boşluk, tire, altçizgi, nokta) ara
        let min_break = (best_fit / 3).max(2).min(best_fit);
        let mut split_at = best_fit;
        for i in (min_break..=best_fit).rev() {
            let ch = chars[cursor + i - 1];
            if ch == ' ' || ch == '-' || ch == '_' || ch == '.' {
                split_at = i;
                break;
            }
        }

        let line_chars = &chars[cursor..cursor + split_at];
        let line_str: String = line_chars.iter().collect();
        lines.push(line_str.trim_end().to_string());

        cursor += split_at;
        while cursor < chars.len() && chars[cursor] == ' ' {
            cursor += 1;
        }
    }

    if lines.is_empty() {
        lines.push(name.to_string());
    }

    lines
}

pub fn wrap_name_multiline_measured(
    ui: &Ui,
    name: &str,
    font_id: &FontId,
    max_width: f32,
    max_lines: usize,
) -> Vec<String> {
    wrap_name_multiline_with_measure(name, max_lines, max_width, |text| {
        ui.painter().layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE).size().x
    })
}

pub fn wrap_name_multiline(name: &str, chars_per_line: usize, max_lines: usize) -> Vec<String> {
    wrap_name_multiline_with_measure(name, max_lines, chars_per_line as f32, |s| s.chars().count() as f32)
}

pub fn wrap_name_two_lines(name: &str, chars_per_line: usize) -> (String, Option<String>) {
    let char_count = name.chars().count();
    if char_count <= chars_per_line {
        return (name.to_string(), None);
    }

    let chars: Vec<char> = name.chars().collect();
    let min_split = chars_per_line.saturating_sub(5).max(4);
    let max_split = chars_per_line.min(char_count.saturating_sub(2));

    let mut split_at = chars_per_line;
    for i in (min_split..=max_split).rev() {
        if chars[i] == ' ' || chars[i] == '-' || chars[i] == '_' || chars[i] == '.' {
            split_at = if chars[i] == ' ' { i } else { i + 1 };
            break;
        }
    }

    let line1: String = chars[..split_at].iter().collect();
    let remaining = &chars[split_at..];
    let remaining_trimmed: Vec<char> = if remaining.first() == Some(&' ') {
        remaining[1..].to_vec()
    } else {
        remaining.to_vec()
    };

    let max_line2 = chars_per_line + 4;
    let line2: String = if remaining_trimmed.len() <= max_line2 {
        remaining_trimmed.into_iter().collect()
    } else {
        let cut: String = remaining_trimmed.into_iter().take(max_line2.saturating_sub(1)).collect();
        format!("{}…", cut)
    };

    (line1, Some(line2))
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_chars { return name.to_string(); }
    let cut: String = name.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", cut)
}

#[derive(Clone, Debug)]
struct HoverTimerState {
    last_path: String,
    start_time: std::time::Instant,
}

fn attach_delayed_hover_tooltip(
    resp: &egui::Response,
    ui: &mut Ui,
    t: &AexTheme,
    entry: &FileEntry,
    is_sel: bool,
) {
    let timer_id = egui::Id::new("file_entry_hover_tracker");

    // Do NOT show tooltip if the user just clicked, is clicking, or if the item is already selected
    let any_click = ui.ctx().input(|i| i.pointer.primary_clicked() || i.pointer.secondary_clicked() || i.pointer.primary_down());
    if !resp.hovered() || any_click || is_sel {
        ui.ctx().data_mut(|d| d.remove::<HoverTimerState>(timer_id));
        return;
    }

    let now = std::time::Instant::now();

    let is_ready = ui.ctx().data_mut(|d| {
        let state = d.get_temp_mut_or_insert_with(timer_id, || HoverTimerState {
            last_path: entry.path.clone(),
            start_time: now,
        });

        if state.last_path != entry.path {
            state.last_path = entry.path.clone();
            state.start_time = now;
            false
        } else {
            // Require 750ms of quiet, unclicked hover (Nemo style)
            state.start_time.elapsed() >= std::time::Duration::from_millis(750)
        }
    });

    if is_ready {
        egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(format!("tt_{}", entry.path)), |ui| {
            render_entry_hover_tooltip(ui, t, entry);
        });
    } else {
        // Request repaint so that once the 750ms passes, the tooltip shows up smoothly
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn render_entry_hover_tooltip(ui: &mut Ui, t: &AexTheme, entry: &FileEntry) {
    let c = &t.colors;
    let sp = &t.spacing;
    let s = &t.shape;

    egui::Frame {
        fill: c.bg_surface0(),
        rounding: Rounding::same(s.radius_sm),
        inner_margin: Margin::symmetric(sp.sm, sp.xs),
        stroke: Stroke::new(1.0, c.border()),
        ..Default::default()
    }.show(ui, |ui| {
        ui.set_max_width(320.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(file_icon(t, entry)).color(icon_color_for(t, entry)).size(t.icons.size_sm));
            ui.label(RichText::new(&entry.name).color(c.text_primary()).size(t.typography.size_sm).strong());
        });
        ui.add_space(sp.xs);

        egui::Grid::new(format!("tooltip_grid_{}", entry.path))
            .num_columns(2)
            .spacing([sp.md, 2.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Tür:").color(c.text_hint()).size(t.typography.size_xs));
                ui.label(RichText::new(if entry.is_dir { "Klasör" } else if entry.extension.is_empty() { "Dosya" } else { &entry.extension }).color(c.text_secondary()).size(t.typography.size_xs));
                ui.end_row();

                if !entry.is_dir {
                    ui.label(RichText::new("Boyut:").color(c.text_hint()).size(t.typography.size_xs));
                    ui.label(RichText::new(format!("{} ({} bayt)", human_size(entry.size), entry.size)).color(c.text_secondary()).size(t.typography.size_xs));
                    ui.end_row();
                }

                if !entry.modified.is_empty() {
                    ui.label(RichText::new("Değiştirilme:").color(c.text_hint()).size(t.typography.size_xs));
                    ui.label(RichText::new(&entry.modified).color(c.text_secondary()).size(t.typography.size_xs));
                    ui.end_row();
                }

                if !entry.permissions.is_empty() {
                    ui.label(RichText::new("İzinler:").color(c.text_hint()).size(t.typography.size_xs));
                    ui.label(RichText::new(&entry.permissions).color(c.text_secondary()).size(t.typography.size_xs).monospace());
                    ui.end_row();
                }

                ui.label(RichText::new("Konum:").color(c.text_hint()).size(t.typography.size_xs));
                ui.label(RichText::new(&entry.path).color(c.text_disabled()).size(t.typography.size_xs));
                ui.end_row();
            });
    });
}


// ─────────────────────────────────────────────────────────────────────────────
// State views
// ─────────────────────────────────────────────────────────────────────────────

fn loading_view(ui: &mut Ui, t: &AexTheme) {
    let c = &t.colors;
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            // Spinner character (nerd font veya fallback)
            let frame = (ui.ctx().input(|i| i.time) * 8.0) as usize % 8;
            let frames = ["⣾","⣽","⣻","⢿","⡿","⣟","⣯","⣷"];
            ui.label(
                RichText::new(frames[frame])
                    .color(c.accent())
                    .size(28.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Loading…")
                    .color(c.text_hint())
                    .size(t.typography.size_sm)
            );
            // Spinner için sürekli repaint
            ui.ctx().request_repaint();
        });
    });
}

fn error_view(ui: &mut Ui, t: &AexTheme, err: &str) {
    let c = &t.colors;
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.label(
                RichText::new(icon(t, "error"))
                    .color(c.danger())
                    .size(32.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(err)
                    .color(c.danger())
                    .size(t.typography.size_sm)
            );
        });
    });
}

fn empty_view(ui: &mut Ui, t: &AexTheme, is_search: bool) {
    let c = &t.colors;
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.label(
                RichText::new(if is_search { icon(t, "search") } else { icon(t, "folder_open") })
                    .color(c.text_disabled())
                    .size(40.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(if is_search { "No results" } else { "Empty folder" })
                    .color(c.text_hint())
                    .size(t.typography.size_base)
            );
        });
    });
}

// ── draw_grid_thumbnail ───────────────────────────────────────────────────────
// Returns true if a thumbnail was drawn (so caller skips the fallback icon)

use crate::backend::thumbnails::{get_thumbnail, jpeg_to_color_image};

fn draw_grid_thumbnail(
    ui: &mut Ui,
    t: &AexTheme,
    entry: &FileEntry,
    rect: Rect,
    item_w: f32,
    item_h: f32,
    thumb_q: crate::backend::thumbnails::ThumbQuality,
    is_cut: bool,
    opacity: f32,
) -> bool {


    let ext      = entry.extension.to_lowercase();
    let is_gif   = ext == "gif";
    let is_media = matches!(ext.as_str(),
        "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"
        |"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts"
        |"mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma");

    if !is_media || entry.is_dir { return false; }

    let icon_area_h = (item_h * 0.52).round();
    let max_thumb_w = (item_w - 12.0).max(24.0);
    let max_thumb_h = (icon_area_h - 6.0).max(24.0);
    let thumb_center = Pos2::new(rect.center().x, rect.min.y + 3.0 + max_thumb_h / 2.0);
    let tint = Color32::from_rgba_unmultiplied(255, 255, 255,
        if is_cut { 102 } else { (255.0 * opacity) as u8 });

    // ── GIF: animated frames ─────────────────────────────────────────────
    if is_gif {
        let gif_id  = egui::Id::new(format!("gif_cache_{}", entry.path));
        let time_id = egui::Id::new(format!("gif_time_{}", entry.path));

        let frames: Option<Vec<egui::TextureHandle>> =
            ui.ctx().data_mut(|d| d.get_temp(gif_id));

        if let Some(ref flist) = frames {
            if flist.is_empty() { return false; }
            let delays: Vec<u64> = ui.ctx().data_mut(|d|
                d.get_temp(egui::Id::new(format!("gif_delays_{}", entry.path)))
                .unwrap_or_else(|| vec![100u64; flist.len()]));
            let total_ms = delays.iter().sum::<u64>().max(1);
            let start: std::time::Instant = ui.ctx().data_mut(|d|
                d.get_temp(time_id).unwrap_or_else(|| {
                    let now = std::time::Instant::now();
                    d.insert_temp(time_id, now); now
                }));
            let elapsed = (start.elapsed().as_millis() as u64) % total_ms;
            let mut acc = 0u64; let mut fidx = 0;
            for (i, &d) in delays.iter().enumerate() {
                acc += d; if elapsed < acc { fidx = i; break; } fidx = i;
            }
            let h = &flist[fidx];
            let tex_size = h.size_vec2();
            let aspect = if tex_size.y > 0.0 { (tex_size.x / tex_size.y).clamp(0.2, 5.0) } else { 1.0 };
            let (tw, th) = if aspect >= (max_thumb_w / max_thumb_h) {
                (max_thumb_w, (max_thumb_w / aspect).round())
            } else {
                (((max_thumb_h * aspect).round()).min(max_thumb_w), max_thumb_h)
            };
            let tr = Rect::from_center_size(thumb_center, Vec2::new(tw, th));
            ui.painter().rect_filled(tr, Rounding::same(4.0), Color32::from_black_alpha(25));
            ui.painter().image(h.id(), tr,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
            ui.painter().rect_stroke(tr, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_white_alpha(35)));
            ui.ctx().request_repaint();
            return true;
        } else {
            // Load GIF frames in background
            let req_id = egui::Id::new(format!("gif_req_{}", entry.path));
            let loading: bool = ui.ctx().data_mut(|d| d.get_temp(req_id).unwrap_or(false));
            if !loading {
                ui.ctx().data_mut(|d| d.insert_temp(req_id, true));
                let path_c = entry.path.clone();
                let ctx_c  = ui.ctx().clone();
                std::thread::spawn(move || {
                    let frames = crate::ui::preview_panel::load_gif_frames(&path_c);
                    if !frames.is_empty() {
                        let mut delays: Vec<u64> = Vec::new();
                        let mut raw: Vec<egui::ColorImage> = Vec::new();
                        for frame in frames {
                            delays.push(frame.delay_ms);
                            raw.push(egui::ColorImage::from_rgba_unmultiplied(
                                [frame.width as usize, frame.height as usize],
                                &frame.rgba,
                            ));
                        }
                        ctx_c.data_mut(|d| {
                            d.insert_temp(
                                egui::Id::new(format!("gif_raw_{}", path_c)),
                                (raw, delays, path_c.clone()),
                            );
                        });
                        ctx_c.request_repaint();
                    }
                });
            }
            // Upload if ready
            type RawGif = (Vec<egui::ColorImage>, Vec<u64>, String);
            let raw: Option<RawGif> = ui.ctx().data_mut(|d|
                d.get_temp(egui::Id::new(format!("gif_raw_{}", entry.path))));
            if let Some((images, delays, p)) = raw {
                let handles: Vec<egui::TextureHandle> = images.into_iter().enumerate()
                    .map(|(i, ci)| ui.ctx().load_texture(
                        format!("gif_frame_{}_{}", p, i), ci, egui::TextureOptions::LINEAR))
                    .collect();
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new(format!("gif_cache_{}", p)), handles);
                    d.insert_temp(egui::Id::new(format!("gif_delays_{}", p)), delays);
                    d.remove::<RawGif>(egui::Id::new(format!("gif_raw_{}", p)));
                });
                ui.ctx().request_repaint();
            }
            return false; // First frame not ready yet
        }
    }

    // ── Static image: disk-cached thumbnail ──────────────────────────────
    let cache_key = format!("thumb_{}_{}", entry.path, thumb_q.suffix());
    let thumb_id  = egui::Id::new(&cache_key);
    let handle: Option<egui::TextureHandle> =
        ui.ctx().data_mut(|d| d.get_temp(thumb_id));

    if let Some(ref h) = handle {
        let tex_size = h.size_vec2();
        let aspect = if tex_size.y > 0.0 { (tex_size.x / tex_size.y).clamp(0.2, 5.0) } else { 1.0 };
        let (tw, th) = if aspect >= (max_thumb_w / max_thumb_h) {
            (max_thumb_w, (max_thumb_w / aspect).round())
        } else {
            (((max_thumb_h * aspect).round()).min(max_thumb_w), max_thumb_h)
        };
        let tr = Rect::from_center_size(thumb_center, Vec2::new(tw, th));
        ui.painter().rect_filled(tr, Rounding::same(4.0), Color32::from_black_alpha(25));
        ui.painter().image(h.id(), tr,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
        ui.painter().rect_stroke(tr, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_white_alpha(35)));
        return true;
    }

    // Load from disk or generate in background
    let req_id = egui::Id::new(format!("treq_{}_{}", entry.path, thumb_q.suffix()));
    let loading: bool = ui.ctx().data_mut(|d| d.get_temp(req_id).unwrap_or(false));
    if !loading {
        ui.ctx().data_mut(|d| d.insert_temp(req_id, true));
        let path_c = entry.path.clone();
        let ctx_c  = ui.ctx().clone();
        let key    = cache_key.clone();
        std::thread::spawn(move || {
            if let Some(jpeg) = get_thumbnail(&path_c, thumb_q) {
                if let Some(ci) = jpeg_to_color_image(&jpeg) {
                    ctx_c.data_mut(|d| {
                        d.insert_temp(egui::Id::new(format!("traw_{}", key)),
                            (ci, key.clone()));
                    });
                    ctx_c.request_repaint();
                }
            }
        });
    }
    // Upload raw if ready
    let raw: Option<(egui::ColorImage, String)> =
        ui.ctx().data_mut(|d| d.get_temp(egui::Id::new(format!("traw_{}", cache_key))));
    if let Some((ci, k)) = raw {
        let h = ui.ctx().load_texture(
            format!("thumb_{}", k), ci, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| {
            d.insert_temp(egui::Id::new(&k), h);
            d.remove::<(egui::ColorImage, String)>(
                egui::Id::new(format!("traw_{}", k)));

            // LRU texture memory management: keep max 300 textures in egui memory
            let lru_id = egui::Id::new("grid_thumb_lru_keys");
            let mut lru: std::collections::VecDeque<String> = d.get_temp(lru_id).unwrap_or_default();
            lru.push_back(k.clone());
            while lru.len() > 300 {
                if let Some(old_key) = lru.pop_front() {
                    d.remove::<egui::TextureHandle>(egui::Id::new(&old_key));
                }
            }
            d.insert_temp(lru_id, lru);
        });
        ui.ctx().request_repaint();
    }
    false // Still loading
}

// ─────────────────────────────────────────────────────────────────────────────
// Inactive Pane Snapshot Rendering (100% pixel and font-identical to active pane)
// ─────────────────────────────────────────────────────────────────────────────

pub fn render_snapshot(
    ui: &mut Ui,
    t: &AexTheme,
    snap: &crate::state::PaneSnapshot,
) {
    render_snapshot_tagged(ui, t, snap, "right");
}

pub fn render_snapshot_tagged(
    ui: &mut Ui,
    t: &AexTheme,
    snap: &crate::state::PaneSnapshot,
    pane_tag: &str,
) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let entries: Vec<&FileEntry> = snap.dir.entries.iter()
        .filter(|e| snap.dir.show_hidden || !e.hidden)
        .collect();

    if entries.is_empty() {
        empty_view(ui, t, false);
        return;
    }

    match snap.view_mode {
        ViewMode::Grid => {
            let item_size = (t.layout.grid_item_size * snap.view_scale).clamp(40.0, 240.0);
            let gap = t.layout.grid_gap;
            let available_w = ui.available_width();
            let cols = ((available_w + gap) / (item_size + gap)).max(1.0) as usize;

            let icon_font  = (item_size * 0.36).clamp(16.0, 64.0);
            let name_font  = (item_size * 0.115).clamp(9.0, 14.0);
            let badge_font = (item_size * 0.085).clamp(8.0, 11.0);

            let thumb_q = if item_size < 100.0 {
                crate::backend::thumbnails::ThumbQuality::Low
            } else if item_size < 170.0 {
                crate::backend::thumbnails::ThumbQuality::Mid
            } else {
                crate::backend::thumbnails::ThumbQuality::High
            };

            // Pane-tagged ID: Sol/Sağ panel geçişinde scroll asla sıfırlanmaz
            egui::ScrollArea::vertical()
                .id_salt(Id::new(format!("file_grid_{}", pane_tag)).with(&snap.nav.current_path))
                .auto_shrink([false; 2])
                .drag_to_scroll(false)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    let item_w = item_size;
                    let item_h = (item_size * 1.18).round();
                    egui::Frame::none().inner_margin(Margin::same(gap)).show(ui, |ui| {
                        egui::Grid::new("inactive_file_grid")
                            .min_col_width(item_w)
                            .max_col_width(item_w)
                            .spacing(Vec2::splat(gap))
                            .show(ui, |ui| {
                                for (idx, entry) in entries.iter().enumerate() {
                                    if idx > 0 && idx % cols == 0 {
                                        ui.end_row();
                                    }
                                    let is_sel = snap.selection.is_selected(&entry.path);
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(item_w, item_h),
                                        Sense::hover(),
                                    );
                                    if ui.is_rect_visible(rect) {
                                        let bg = if is_sel {
                                            Color32::from_rgba_unmultiplied(
                                                c.accent().r(), c.accent().g(), c.accent().b(), 38,
                                            )
                                        } else if resp.hovered() {
                                            Color32::from_rgba_unmultiplied(
                                                c.bg_surface2().r(), c.bg_surface2().g(), c.bg_surface2().b(), 95,
                                            )
                                        } else {
                                            Color32::TRANSPARENT
                                        };

                                        ui.painter().rect_filled(rect, Rounding::same(s.radius_md), bg);
                                        if is_sel {
                                            ui.painter().rect_stroke(rect, Rounding::same(s.radius_md), Stroke::new(1.5_f32, c.accent()));
                                        }

                                        let visual_center = rect.min + Vec2::new(item_w / 2.0, item_h * 0.35);
                                        let drew_thumb = draw_grid_thumbnail(ui, t, entry, rect, item_w, item_h, thumb_q, false, 1.0);

                                        if !drew_thumb {
                                            let icon_str = file_icon(t, entry);
                                            let icon_color = icon_color_for(t, entry);

                                            ui.painter().text(
                                                visual_center,
                                                Align2::CENTER_CENTER,
                                                icon_str,
                                                FontId::new(icon_font, FontFamily::Monospace),
                                                icon_color,
                                            );
                                        }

                                        if entry.is_symlink {
                                            ui.painter().text(
                                                rect.min + Vec2::new(item_size * 0.76, item_size * 0.18),
                                                Align2::CENTER_CENTER,
                                                icon(t, "symlink"),
                                                FontId::new(badge_font, FontFamily::Monospace),
                                                c.info(),
                                            );
                                        }

                                        let has_badge = !entry.is_dir && entry.size > 0 && item_size >= 60.0;
                                        let name_center_y = if has_badge { item_size * 0.72 } else { item_size * 0.76 };
                                        let name_rect_center = rect.min + Vec2::new(item_size / 2.0, name_center_y);

                                        let max_chars = ((item_size - 10.0) / (name_font * 0.60)).max(6.0) as usize;
                                        let name = truncate_name(&entry.name, max_chars);
                                        ui.painter().text(
                                            name_rect_center,
                                            Align2::CENTER_CENTER,
                                            &name,
                                            FontId::new(name_font, FontFamily::Proportional),
                                            c.text_primary(),
                                        );

                                        if has_badge {
                                            let badge_center = rect.min + Vec2::new(item_size / 2.0, item_size * 0.88);
                                            ui.painter().text(
                                                badge_center,
                                                Align2::CENTER_CENTER,
                                                &human_size(entry.size),
                                                FontId::new(badge_font, FontFamily::Proportional),
                                                c.text_hint(),
                                            );
                                        }
                                    }
                                }
                            });
                    });
                });
        }
        _ => {
            let row_h = (t.layout.item_height * snap.view_scale).clamp(18.0, 80.0);
            let show_thumb = row_h >= 24.0;
            egui::ScrollArea::vertical()
                .id_salt(Id::new(format!("file_list_{}", pane_tag)).with(&snap.nav.current_path))
                .auto_shrink([false; 2])
                .drag_to_scroll(false)
                .show(ui, |ui| {
                    for entry in entries {
                        let is_sel = snap.selection.is_selected(&entry.path);
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row_h),
                            Sense::hover(),
                        );
                        if ui.is_rect_visible(rect) {
                            let bg = if is_sel {
                                Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 38)
                            } else if resp.hovered() {
                                Color32::from_rgba_unmultiplied(c.bg_surface2().r(), c.bg_surface2().g(), c.bg_surface2().b(), 95)
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);
                            if is_sel {
                                ui.painter().rect_stroke(rect, Rounding::same(s.radius_sm), Stroke::new(1.0_f32, c.accent()));
                            }

                            let icon_x  = rect.min.x + sp.md + 4.0;
                            let icon_cx = icon_x + t.icons.size_sm / 2.0;

                            let ext = entry.extension.to_lowercase();
                            let is_media = !entry.is_dir && matches!(ext.as_str(),
                                "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"
                                |"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts"
                                |"mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma");
                            let mut drew_thumb = false;

                            if is_media && show_thumb {
                                let th_sz  = (row_h - 4.0).max(16.0);
                                let th_id  = egui::Id::new(format!("thumb_{}_lo", entry.path));
                                let handle: Option<egui::TextureHandle> =
                                    ui.ctx().data_mut(|d| d.get_temp(th_id));
                                if let Some(ref h) = handle {
                                    let tex_sz = h.size();
                                    let aspect = if tex_sz[1] > 0 { tex_sz[0] as f32 / tex_sz[1] as f32 } else { 1.0 };
                                    let (fit_w, fit_h) = if aspect >= 1.0 {
                                        (th_sz, (th_sz / aspect).max(1.0))
                                    } else {
                                        ((th_sz * aspect).max(1.0), th_sz)
                                    };
                                    let tr = Rect::from_center_size(
                                        Pos2::new(icon_cx, rect.center().y), Vec2::new(fit_w, fit_h));
                                    ui.painter().image(h.id(), tr,
                                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                    drew_thumb = true;
                                } else {
                                    use crate::backend::thumbnails::{get_thumbnail, jpeg_to_color_image, ThumbQuality};
                                    let req = egui::Id::new(format!("lstreq_{}", entry.path));
                                    let loading: bool = ui.ctx().data_mut(|d| d.get_temp(req).unwrap_or(false));
                                    if !loading {
                                        ui.ctx().data_mut(|d| d.insert_temp(req, true));
                                        let p = entry.path.clone();
                                        let ctx2 = ui.ctx().clone();
                                        std::thread::spawn(move || {
                                            if let Some(j) = get_thumbnail(&p, ThumbQuality::Low) {
                                                if let Some(ci) = jpeg_to_color_image(&j) {
                                                    ctx2.data_mut(|d| d.insert_temp(egui::Id::new(format!("lstraw_{}", p)), (ci, p.clone())));
                                                    ctx2.request_repaint();
                                                }
                                            }
                                        });
                                    }
                                    let raw: Option<(egui::ColorImage, String)> =
                                        ui.ctx().data_mut(|d| d.get_temp(egui::Id::new(format!("lstraw_{}", entry.path))));
                                    if let Some((ci, p)) = raw {
                                        let h = ui.ctx().load_texture(format!("lthumb_{}", p), ci, egui::TextureOptions::LINEAR);
                                        ui.ctx().data_mut(|d| {
                                            d.insert_temp(egui::Id::new(format!("thumb_{}_lo", p)), h);
                                            d.remove::<(egui::ColorImage, String)>(egui::Id::new(format!("lstraw_{}", p)));
                                        });
                                        ui.ctx().request_repaint();
                                    }
                                }
                            }

                            if !drew_thumb {
                                ui.painter().text(
                                    Pos2::new(icon_cx, rect.center().y),
                                    Align2::CENTER_CENTER,
                                    file_icon(t, entry),
                                    FontId::new(t.icons.size_sm, FontFamily::Monospace),
                                    icon_color_for(t, entry),
                                );
                            }

                            // Name
                            ui.painter().text(
                                rect.min + Vec2::new(sp.md + 26.0, rect.height() / 2.0),
                                Align2::LEFT_CENTER,
                                &entry.name,
                                FontId::new(t.typography.size_sm, FontFamily::Proportional),
                                if is_sel { c.text_primary() } else { c.text_secondary() },
                            );

                            // Size
                            if !entry.is_dir && entry.size > 0 {
                                ui.painter().text(
                                    rect.max - Vec2::new(sp.md + 20.0, rect.height() / 2.0),
                                    Align2::RIGHT_CENTER,
                                    &human_size(entry.size),
                                    FontId::new(t.typography.size_xs, FontFamily::Monospace),
                                    c.text_hint(),
                                );
                            }
                        }
                    }
                });
        }
    }
}

fn git_badge_for<'a>(state: &'a AppState, entry: &FileEntry) -> Option<(&'a str, Color32)> {
    if !state.git_status.is_repo { return None; }
    let ch = state.git_status.files.get(&entry.path)
        .or_else(|| state.git_status.files.get(&entry.name))?;
    let color = match ch.as_str() {
        "M" => Color32::from_rgb(255, 209, 102),
        "A" => Color32::from_rgb(90, 255, 142),
        "D" => Color32::from_rgb(255, 90, 90),
        "R" => Color32::from_rgb(130, 100, 255),
        "?" => Color32::from_rgb(136, 152, 170),
        _   => Color32::from_rgb(136, 152, 170),
    };
    Some((ch.as_str(), color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_hit_area_tight_detection() {
        let thumb_hit = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(50.0, 50.0)).expand(5.0);
        let label_hit = Rect::from_min_max(Pos2::new(20.0, 60.0), Pos2::new(40.0, 80.0)).expand(5.0);

        let hit_area = GridHitArea { thumb_hit, label_hit };

        // Inside thumbnail area -> hit!
        assert!(hit_area.contains(Pos2::new(30.0, 30.0)));
        // In 5px thumbnail margin -> hit!
        assert!(hit_area.contains(Pos2::new(6.0, 6.0)));

        // Inside label pill -> hit!
        assert!(hit_area.contains(Pos2::new(30.0, 70.0)));
        // In 5px label margin -> hit!
        assert!(hit_area.contains(Pos2::new(16.0, 70.0)));

        // Empty corner outside the 5px margin -> NOT a hit!
        assert!(!hit_area.contains(Pos2::new(0.0, 0.0)));
        assert!(!hit_area.contains(Pos2::new(100.0, 100.0)));
        assert!(!hit_area.contains(Pos2::new(2.0, 70.0)));
        assert!(!hit_area.contains(Pos2::new(80.0, 70.0)));

        // Rubber-band intersection
        let touching_rb = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0));
        assert!(hit_area.intersects(&touching_rb));

        let non_touching_rb = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(4.0, 4.0));
        assert!(!hit_area.intersects(&non_touching_rb));
    }

    #[test]
    fn test_nemo_connected_hitbox_no_dead_zones() {
        // Thumbnail at (10, 10) to (90, 70)
        let visual_thumb = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(90.0, 70.0));
        // Label at (20, 85) to (80, 105)
        let label_top_y = 85.0_f32;
        let label_bottom_y = 105.0_f32;
        let label_pill_rect = Rect::from_min_max(Pos2::new(20.0, label_top_y), Pos2::new(80.0, label_bottom_y));

        let thumb_margin = 2.0_f32;
        let label_margin = 2.0_f32;

        let thumb_hit = Rect::from_min_max(
            Pos2::new(visual_thumb.min.x - thumb_margin, visual_thumb.min.y - thumb_margin),
            Pos2::new(visual_thumb.max.x + thumb_margin, (label_top_y + 1.0).max(visual_thumb.max.y + thumb_margin)),
        );
        let label_hit = Rect::from_min_max(
            Pos2::new(label_pill_rect.min.x - label_margin, visual_thumb.max.y.min(label_top_y - label_margin)),
            Pos2::new(label_pill_rect.max.x + label_margin, label_bottom_y + label_margin),
        );

        let hit_area = GridHitArea { thumb_hit, label_hit };

        // 1. Inside thumbnail image (with 2px margin)
        assert!(hit_area.contains(Pos2::new(50.0, 40.0)));
        assert!(hit_area.contains(Pos2::new(8.5, 40.0))); // Left 2px margin
        assert!(hit_area.contains(Pos2::new(50.0, 8.5))); // Top 2px margin

        // 2. In the gap between thumbnail and label (y = 72.0, 78.0, 84.0) -> NO DEAD ZONE!
        assert!(hit_area.contains(Pos2::new(50.0, 72.0)));
        assert!(hit_area.contains(Pos2::new(50.0, 78.0)));
        assert!(hit_area.contains(Pos2::new(50.0, 84.0)));

        // 3. Inside label pill and text
        assert!(hit_area.contains(Pos2::new(50.0, 95.0)));
        assert!(hit_area.contains(Pos2::new(18.5, 95.0))); // Left 2px margin
        assert!(hit_area.contains(Pos2::new(50.0, 106.5))); // Bottom 2px margin

        // 4. Outside empty margins -> NOT a hit (allows deselecting & rubber-band)
        assert!(!hit_area.contains(Pos2::new(5.0, 40.0))); // Far left of thumbnail
        assert!(!hit_area.contains(Pos2::new(95.0, 40.0))); // Far right of thumbnail
        assert!(!hit_area.contains(Pos2::new(50.0, 5.0))); // Above thumbnail
        assert!(!hit_area.contains(Pos2::new(50.0, 110.0))); // Below label
        assert!(!hit_area.contains(Pos2::new(5.0, 78.0))); // Far left of gap
        assert!(!hit_area.contains(Pos2::new(95.0, 78.0))); // Far right of gap
    }

    #[test]
    fn test_wrap_name_two_lines() {
        // Fits on single line
        let (l1, l2) = wrap_name_two_lines("file.txt", 12);
        assert_eq!(l1, "file.txt");
        assert!(l2.is_none());

        // Wraps onto 2 lines cleanly
        let (l1, l2) = wrap_name_two_lines("long_document_report.pdf", 14);
        assert!(l2.is_some());
        assert!(!l1.is_empty());
        assert!(!l2.unwrap().is_empty());

        // Extreme length wraps with ellipsis
        let (l1, l2) = wrap_name_two_lines("super_hyper_extremely_long_unreasonably_extended_filename_for_test.txt", 12);
        assert!(l2.is_some());
        let l2_str = l2.unwrap();
        assert!(l2_str.ends_with('…'));
    }

    #[test]
    fn test_wrap_name_multiline() {
        // Fits on single line
        let lines = wrap_name_multiline("photo.png", 14, 3);
        assert_eq!(lines, vec!["photo.png"]);

        // Wraps onto 2 lines
        let lines = wrap_name_multiline("my_trip_photos.zip", 12, 3);
        assert_eq!(lines.len(), 2);

        // Wraps onto 3 lines with ellipsis on last
        let lines = wrap_name_multiline("very_important_financial_audit_quarter_four.xlsx", 12, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines[2].ends_with('…'));
    }

    #[test]
    fn test_distinct_extension_icons_and_colors() {
        let theme = crate::theme::defaults::default_theme();
        let make_entry = |name: &str, ext: &str, is_exec: bool| FileEntry {
            name: name.to_string(),
            path: format!("/tmp/{}", name),
            is_dir: false,
            is_symlink: false,
            size: 1024,
            modified: "".to_string(),
            created: "".to_string(),
            permissions: "rwxr-xr-x".to_string(),
            mime_type: "application/octet-stream".to_string(),
            extension: ext.to_string(),
            hidden: false,
            is_executable: is_exec,
        };

        let exe_entry = make_entry("setup.exe", "exe", true);
        let dll_entry = make_entry("lib.dll", "dll", false);
        let msi_entry = make_entry("installer.msi", "msi", false);
        let bat_entry = make_entry("script.bat", "bat", true);
        let reg_entry = make_entry("settings.reg", "reg", false);
        let unk_entry = make_entry("unknown.xyz123", "xyz123", false);

        // Distinct icons
        assert_eq!(crate::ui::file_icon(&theme, &exe_entry), crate::ui::icon(&theme, "exe"));
        assert_eq!(crate::ui::file_icon(&theme, &dll_entry), crate::ui::icon(&theme, "dll"));
        assert_eq!(crate::ui::file_icon(&theme, &msi_entry), crate::ui::icon(&theme, "msi"));
        assert_eq!(crate::ui::file_icon(&theme, &bat_entry), crate::ui::icon(&theme, "bat"));
        assert_eq!(crate::ui::file_icon(&theme, &reg_entry), crate::ui::icon(&theme, "reg"));
        // Unknown extension gets fallback dog-eared paper
        assert_eq!(crate::ui::file_icon(&theme, &unk_entry), crate::ui::icon(&theme, "file"));

        // Distinct colors
        assert_eq!(icon_color_for(&theme, &exe_entry), Color32::from_rgb(0, 180, 240));
        assert_eq!(icon_color_for(&theme, &dll_entry), Color32::from_rgb(180, 100, 240));
        assert_eq!(icon_color_for(&theme, &msi_entry), Color32::from_rgb(245, 140, 40));
        assert_eq!(icon_color_for(&theme, &bat_entry), Color32::from_rgb(80, 200, 120));
        assert_eq!(icon_color_for(&theme, &reg_entry), Color32::from_rgb(40, 200, 190));
        assert_eq!(icon_color_for(&theme, &unk_entry), theme.colors.text_secondary());
    }

    #[test]
    fn test_windows_icon_and_color() {
        test_distinct_extension_icons_and_colors();
    }

    #[test]
    fn test_format_nemo_size() {
        assert_eq!(format_nemo_size(500), "500 B");
        assert_eq!(format_nemo_size(1024), "1.0 kB");
        assert_eq!(format_nemo_size(74137), "72.4 kB");
        assert_eq!(format_nemo_size(1048576), "1.0 MB");
        assert_eq!(format_nemo_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_entry_type_label() {
        let make_entry = |ext: &str, is_dir: bool| FileEntry {
            name: "test".to_string(),
            path: "/tmp/test".to_string(),
            is_dir,
            is_symlink: false,
            size: 100,
            modified: "".to_string(),
            created: "".to_string(),
            permissions: "rwxr-xr-x".to_string(),
            mime_type: "".to_string(),
            extension: ext.to_string(),
            hidden: false,
            is_executable: false,
        };

        assert_eq!(entry_type_label(&make_entry("", true)), "Folder");
        assert_eq!(entry_type_label(&make_entry("png", false)), "Image");
        assert_eq!(entry_type_label(&make_entry("jpg", false)), "Image");
        assert_eq!(entry_type_label(&make_entry("webp", false)), "Image");
        assert_eq!(entry_type_label(&make_entry("mp4", false)), "Video");
        assert_eq!(entry_type_label(&make_entry("mp3", false)), "Audio");
        assert_eq!(entry_type_label(&make_entry("pdf", false)), "PDF Document");
        assert_eq!(entry_type_label(&make_entry("zip", false)), "Archive");
        assert_eq!(entry_type_label(&make_entry("rs", false)), "Document");
    }

    #[test]
    fn test_list_view_aspect_ratio_fitting() {
        let th_sz: f32 = 32.0;

        // Landscape image (16:9, e.g. 1920x1080)
        let aspect_landscape: f32 = 1920.0 / 1080.0;
        let (fit_w, fit_h) = if aspect_landscape >= 1.0 {
            (th_sz, (th_sz / aspect_landscape).max(1.0_f32))
        } else {
            ((th_sz * aspect_landscape).max(1.0_f32), th_sz)
        };
        assert_eq!(fit_w, 32.0);
        assert!((fit_h - 18.0).abs() < 1.0); // 32 / 1.777 = 18.0

        // Portrait image (9:16, e.g. 1080x1920)
        let aspect_portrait: f32 = 1080.0 / 1920.0;
        let (fit_w, fit_h) = if aspect_portrait >= 1.0 {
            (th_sz, (th_sz / aspect_portrait).max(1.0_f32))
        } else {
            ((th_sz * aspect_portrait).max(1.0_f32), th_sz)
        };
        assert_eq!(fit_h, 32.0);
        assert!((fit_w - 18.0).abs() < 1.0); // 32 * 0.5625 = 18.0

        // Square image (1:1)
        let aspect_square: f32 = 1.0;
        let (fit_w, fit_h) = if aspect_square >= 1.0 {
            (th_sz, (th_sz / aspect_square).max(1.0_f32))
        } else {
            ((th_sz * aspect_square).max(1.0_f32), th_sz)
        };
        assert_eq!(fit_w, 32.0);
        assert_eq!(fit_h, 32.0);
    }

    #[test]
    fn test_media_grid_style_toggle() {
        let mut state = AppState::new();
        state.set_media_grid_style(MediaGridStyle::NemoGrid);
        assert_eq!(state.media_grid_style, MediaGridStyle::NemoGrid);
        state.set_media_grid_style(MediaGridStyle::Masonry);
        assert_eq!(state.media_grid_style, MediaGridStyle::Masonry);
        state.set_media_grid_style(MediaGridStyle::NemoGrid);
        assert_eq!(state.media_grid_style, MediaGridStyle::NemoGrid);
    }

    #[test]
    fn test_compact_view_media_extensions() {
        let is_compact_media = |ext: &str| -> bool {
            matches!(ext,
                "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"|"tiff"|"avif"
                |"mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts")
        };

        // All supported image formats
        for ext in &["png", "jpg", "jpeg", "webp", "bmp", "gif", "ico", "svg", "tiff", "avif"] {
            assert!(is_compact_media(ext), "Expected image extension {} to match", ext);
        }

        // All supported video formats
        for ext in &["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v", "ts"] {
            assert!(is_compact_media(ext), "Expected video extension {} to match", ext);
        }

        // Non-media formats should not match
        for ext in &["txt", "pdf", "rs", "zip", "exe", "iso"] {
            assert!(!is_compact_media(ext), "Non-media extension {} should not match", ext);
        }
    }

    #[test]
    fn test_grid_item_multiline_wrapping_and_pill_geometry() {
        let name_font = 10.0_f32;
        let line_spacing = name_font * 1.20; // 12.0
        let max_text_w = 100.0_f32;

        let chars_per_line = ((max_text_w) / (name_font * 0.52)).max(8.0) as usize;
        assert_eq!(chars_per_line, 19);

        // Single line file
        let single_lines = wrap_name_multiline("short.txt", chars_per_line, 2);
        assert_eq!(single_lines.len(), 1);
        let single_pill_h = single_lines.len() as f32 * line_spacing + 4.0;
        assert_eq!(single_pill_h, 16.0);

        // Multi-line file
        let multi_lines = wrap_name_multiline("long_filename_that_definitely_wraps_across_lines.pdf", chars_per_line, 2);
        assert_eq!(multi_lines.len(), 2);
        let multi_pill_h = multi_lines.len() as f32 * line_spacing + 4.0;
        assert_eq!(multi_pill_h, 28.0);
        assert!(multi_pill_h > single_pill_h);

        // Centering math: start_y offset from y_name
        let y_name = 80.0_f32;
        let start_y_single = y_name - ((single_lines.len() - 1) as f32 * line_spacing) / 2.0;
        assert_eq!(start_y_single, 80.0); // Exact center for 1 line

        let start_y_multi = y_name - ((multi_lines.len() - 1) as f32 * line_spacing) / 2.0;
        assert_eq!(start_y_multi, 74.0); // -6.0 for line 0
        let line_1_y = start_y_multi + 1.0 * line_spacing;
        assert_eq!(line_1_y, 86.0); // +6.0 for line 1
        assert_eq!((start_y_multi + line_1_y) / 2.0, y_name); // Symmetrically centered!
    }

    #[test]
    fn test_tightened_icon_box_for_non_media() {
        let ctx = egui::Context::default();
        let make_entry = |name: &str, ext: &str, is_dir: bool| FileEntry {
            name: name.to_string(),
            path: format!("/tmp/{}", name),
            is_dir,
            is_symlink: false,
            size: 1024,
            modified: "".to_string(),
            created: "".to_string(),
            permissions: "rwxr-xr-x".to_string(),
            mime_type: "".to_string(),
            extension: ext.to_string(),
            hidden: false,
            is_executable: false,
        };

        let folder = make_entry("Documents", "", true);
        let text_doc = make_entry("notes.txt", "txt", false);
        let rust_code = make_entry("main.rs", "rs", false);

        let item_w = 120.0_f32;
        let max_thumb_w = 108.0_f32;
        let max_thumb_h = 60.0_f32;
        let thumb_center = Pos2::new(100.0, 50.0);
        let expected_sz = (item_w * 0.50).clamp(26.0, 64.0); // 60.0

        for entry in &[folder, text_doc, rust_code] {
            let rect = get_grid_thumb_rect(
                &ctx,
                entry,
                thumb_center,
                item_w,
                max_thumb_w,
                max_thumb_h,
                crate::backend::thumbnails::ThumbQuality::Low,
            );
            // Tightened around glyph: 60x60, NOT max_thumb_w (108px)
            assert_eq!(rect.width(), expected_sz);
            assert_eq!(rect.height(), expected_sz);
            assert_eq!(rect.center(), thumb_center);
            assert!(rect.width() < max_thumb_w);
        }
    }

    #[test]
    fn test_visual_symmetry_mathematical_centering() {
        let rect = Rect::from_min_size(Pos2::new(50.0, 100.0), Vec2::new(140.0, 175.0));
        let item_w = rect.width();
        let item_h = rect.height();

        let icon_area_h = (item_h * 0.52).round();
        let label_top_y = rect.min.y + icon_area_h + 3.0;
        let visual_center = Pos2::new(rect.center().x, (rect.min.y + 3.0 + label_top_y) / 2.0);

        // Icon center X must mathematically equal rect.center().x
        assert_eq!(visual_center.x, rect.center().x);

        // Dikey simetri: İkon merkezi rect.min.y + 3.0 ile label_top_y'nin tam ortasında olmalı
        let top_gap = visual_center.y - (rect.min.y + 3.0);
        let bottom_gap = label_top_y - visual_center.y;
        assert!((top_gap - bottom_gap).abs() < 0.001, "İkonun üst ve alt boşluğu birebir eşit ve simetrik olmalı: {} vs {}", top_gap, bottom_gap);

        // Name pill center X must mathematically equal rect.center().x
        let name_text_w = 80.0_f32;
        let name_pill_h = 24.0_f32;
        let y_name = label_top_y + name_pill_h / 2.0;
        let label_pill_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, y_name),
            Vec2::new(name_text_w, name_pill_h),
        );
        assert_eq!(label_pill_rect.center().x, rect.center().x);

        // Connected hitbox must also remain symmetrical
        let icon_box_sz = (item_w * 0.50).clamp(26.0, 64.0);
        let visual_thumb = Rect::from_center_size(visual_center, Vec2::splat(icon_box_sz));
        assert_eq!(visual_thumb.center().x, rect.center().x);

        let thumb_margin = 2.0_f32;
        let thumb_hit = Rect::from_min_max(
            Pos2::new(visual_thumb.min.x - thumb_margin, visual_thumb.min.y - thumb_margin),
            Pos2::new(visual_thumb.max.x + thumb_margin, (label_top_y + 1.0).max(visual_thumb.max.y + thumb_margin)),
        );
        assert_eq!(thumb_hit.center().x, rect.center().x);
    }

    #[test]
    fn test_intermediate_lines_never_contain_ellipsis() {
        let long_name = "super_architecture_specification_document_v2_final_release_notes.pdf";
        // 3 satır sınırı ile sar
        let lines = wrap_name_multiline(long_name, 14, 3);
        assert_eq!(lines.len(), 3);
        // Ara satırlar (0 ve 1) ASLA ellipsis içermemelidir!
        assert!(!lines[0].contains('…'), "Ara satır 0 ellipsis içeremez: {}", lines[0]);
        assert!(!lines[1].contains('…'), "Ara satır 1 ellipsis içeremez: {}", lines[1]);
        // Yalnızca ve yalnızca son satır (2) kesildiğinde ellipsis almalıdır
        assert!(lines[2].ends_with('…'), "Son satır 2 kesildiğinde '…' ile bitmelidir: {}", lines[2]);

        // 2 satır sınırı ile
        let lines_2 = wrap_name_multiline(long_name, 14, 2);
        assert_eq!(lines_2.len(), 2);
        assert!(!lines_2[0].contains('…'), "2 satırlı modda satır 0 asla ellipsis içeremez: {}", lines_2[0]);
        assert!(lines_2[1].ends_with('…'), "2 satırlı modda son satır ellipsis ile bitmelidir: {}", lines_2[1]);
    }

    #[test]
    fn test_has_item_below_false_shows_all_lines_no_ellipsis() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let font_id = FontId::new(10.0, FontFamily::Proportional);
        let max_text_w = 90.0_f32;

        let long_name = "very_long_project_database_backup_archive_2026_q4_uncompressed.tar.gz";

        // can_expand == false durumu: max_lines = 3 (standart öğeler 3 satır)
        let lines_with_below = ctx.fonts(|f| {
            wrap_name_multiline_with_measure(long_name, 3, max_text_w, |s| {
                f.layout_no_wrap(s.to_string(), font_id.clone(), Color32::WHITE).size().x
            })
        });
        assert_eq!(lines_with_below.len(), 3);
        assert!(!lines_with_below[0].contains('…'), "Satır 0 asla ellipsis içeremez");
        assert!(!lines_with_below[1].contains('…'), "Satır 1 asla ellipsis içeremez");
        assert!(lines_with_below[2].ends_with('…'), "Satır 2 kesildiği için ellipsis almalı");

        // can_expand == true durumu: max_lines = 20 (altında öğe yok, tüm isim satır atlayarak tam gözükür)
        let lines_no_below = ctx.fonts(|f| {
            wrap_name_multiline_with_measure(long_name, 20, max_text_w, |s| {
                f.layout_no_wrap(s.to_string(), font_id.clone(), Color32::WHITE).size().x
            })
        });
        // 4 veya daha fazla satır oluşturmalı
        assert!(lines_no_below.len() >= 3, "Altında öğe yokken en az 3 satıra bölünmeli, oluşan: {}", lines_no_below.len());
        // HİÇBİR satırda (son satır dahil!) ellipsis olmamalıdır çünkü ismin tamamı sığmıştır!
        for (i, line) in lines_no_below.iter().enumerate() {
            assert!(!line.contains('…'), "Satır {} ellipsis içermemelidir: '{}'", i, line);
        }

        // Birleştirilen metin orijinal dosya adının tüm içeriğini korumalıdır
        let reconstructed = lines_no_below.concat();
        assert_eq!(reconstructed, long_name);
    }

    #[test]
    fn test_has_item_below_geometry_and_pill_expansion() {
        let name_font = 10.0_f32;
        let line_spacing = name_font * 1.20; // 12.0
        let item_h = 100.0_f32;
        let icon_area_h = (item_h * 0.52).round(); // 52.0
        let label_top_y_offset = icon_area_h + 3.0; // 55.0
        let label_avail_h = (item_h - 3.0 - label_top_y_offset).max(18.0); // 42.0

        // 5 satırlık uzun bir isim
        let five_line_count = 5;

        // can_expand == false iken: max 3 satır, name_pill_h label_avail_h ile sınırlı
        let pill_h_standard = (3.0_f32 * line_spacing + 4.0).min(label_avail_h - 2.0); // 40.0
        assert_eq!(pill_h_standard, 40.0);

        // can_expand == true iken: name_pill_h satır sayısıyla büyür!
        let pill_h_expanded = five_line_count as f32 * line_spacing + 4.0; // 5 * 12.0 + 4.0 = 64.0
        assert_eq!(pill_h_expanded, 64.0);
        assert!(pill_h_expanded > pill_h_standard);

        // total_card_h de buna göre büyür
        let sub_font = (name_font * 0.86).clamp(8.5, 10.5); // 8.6
        let sub_line_step = sub_font * 1.25;
        let needed_h = label_top_y_offset + pill_h_expanded + sub_font * 0.7 + 2.0 + sub_line_step + sub_font * 0.7 + 8.0;
        let total_card_h = needed_h.max(item_h);
        assert!(total_card_h > item_h, "Kart yüksekliği genişleyen isme göre büyümelidir: {} > {}", total_card_h, item_h);
    }

    #[test]
    fn test_last_row_only_expansion_rule() {
        // 6 sütunlu grid, 7 eleman
        let cols = 6;
        let total_count = 7;
        let total_rows = (total_count + cols - 1) / cols; // 2 satır
        assert_eq!(total_rows, 2);

        // Satır 0 (row_idx = 0):
        let row_0_is_last = (0 + 1) >= total_rows;
        assert!(!row_0_is_last, "Satır 0 son satır değildir");
        // İndeks 1..6 için has_item_below false olsa bile can_expand FALSE olmalı!
        for idx in 0..6 {
            let has_item_below = (idx + cols) < total_count;
            let can_expand = row_0_is_last && !has_item_below;
            assert!(!can_expand, "Satır 0'daki öğe {} ASLA genişleyemez", idx);
        }

        // Satır 1 (row_idx = 1):
        let row_1_is_last = (1 + 1) >= total_rows;
        assert!(row_1_is_last, "Satır 1 son satırdır");
        let idx = 6;
        let has_item_below = (idx + cols) < total_count;
        let can_expand = row_1_is_last && !has_item_below;
        assert!(can_expand, "Son satırdaki altında öğe olmayan öğe {} genişleyebilir", idx);
    }

    #[test]
    fn test_max_chars_increased_with_three_lines_and_wider_text() {
        let name_font = 10.0_f32;
        let item_w = 100.0_f32;

        // Eski sınırlar: max_lines = 2, max_text_w = item_w - 8.0 (92.0)
        let old_chars_per_line = ((item_w - 8.0) / (name_font * 0.52)) as usize; // ~17 chars
        let old_capacity = old_chars_per_line * 2; // ~34 chars

        // Yeni sınırlar: max_lines = 3, max_text_w = item_w + 4.0 (104.0)
        let new_chars_per_line = ((item_w + 4.0) / (name_font * 0.52)) as usize; // ~20 chars
        let new_capacity = new_chars_per_line * 3; // ~60 chars

        // Kapasite artışı en az 15 karakter olmalı!
        assert!(new_capacity >= old_capacity + 15, "Yeni kapasite ({}) eskiden ({}) en az 15 karakter fazla olmalı", new_capacity, old_capacity);
    }
}

