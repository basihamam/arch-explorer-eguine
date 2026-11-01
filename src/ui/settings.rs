// src/ui/settings.rs
// Settings paneli — .aex tema seçici, layout, görünüm ayarları
use egui::*;
use crate::theme::{types::AexTheme, ThemeEngine, ThemeMeta};
use crate::state::AppState;
use super::icon;

#[derive(Clone, PartialEq)]
enum SettingsTab { Appearance, Layout, Behavior, Keybinds, About }

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState, engine: &mut ThemeEngine) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();

    let pw = 640.0_f32;
    let ph = 520.0_f32;
    let px = screen.center().x - pw / 2.0;
    let py = screen.center().y - ph / 2.0;
    let d_rect = Rect::from_min_size(Pos2::new(px, py), Vec2::new(pw, ph));

    // Backdrop
    egui::Area::new(Id::new("settings_backdrop"))
        .fixed_pos(screen.min)
        .order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0,0,0,130));
            let r = ui.allocate_rect(screen, Sense::click());
            let click_pos = ctx.input(|i| i.pointer.interact_pos());
            if r.clicked() && click_pos.map(|p| !d_rect.contains(p) && p.y > 36.0).unwrap_or(false) {
                engine.save_active_theme();
                state.panels.close();
            }
        });

    // Active tab state (egui memory)
    let tab_id = Id::new("settings_tab");
    let mut active_tab = ctx.data_mut(|d| d.get_temp::<SettingsTab>(tab_id).unwrap_or(SettingsTab::Appearance));

    let modal_fill = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_mantle().r(), c.bg_mantle().g(), c.bg_mantle().b(),
            (t.effects.panel_opacity * 255.0) as u8,
        )
    } else {
        c.bg_mantle()
    };

    egui::Area::new(Id::new("settings_panel"))
        .fixed_pos(Pos2::new(px, py))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: modal_fill,
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                shadow: epaint::Shadow {
                    offset: Vec2::new(0.0, 8.0),
                    blur: 24.0,
                    spread: 0.0,
                    color: Color32::from_rgba_unmultiplied(0,0,0,100),
                },
                inner_margin: Margin::ZERO,
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.set_width(pw);
                ui.set_height(ph);

                // ── Header ────────────────────────────────────────────────
                egui::Frame {
                    inner_margin: Margin::symmetric(sp.xl, sp.md),
                    fill: Color32::TRANSPARENT,
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}  Settings", icon(t, "settings")))
                                .color(c.text_primary())
                                .size(t.typography.size_lg)
                                .strong()
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(RichText::new("✕").color(c.text_hint()).size(12.0))
                                    .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)
                            ).clicked() { state.panels.close(); }
                        });
                    });
                });

                let rect = ui.min_rect();
                ui.painter().hline(rect.left()..=rect.right(), rect.bottom(), Stroke::new(1.0_f32, c.border()));

                // ── Tab bar ───────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.add_space(sp.xl);
                    tab_btn(ui, t, "Appearance", SettingsTab::Appearance, &mut active_tab);
                    tab_btn(ui, t, "Layout",     SettingsTab::Layout,     &mut active_tab);
                    tab_btn(ui, t, "Behavior",   SettingsTab::Behavior,   &mut active_tab);
                    tab_btn(ui, t, "Keybinds",   SettingsTab::Keybinds,   &mut active_tab);
                    tab_btn(ui, t, "About",       SettingsTab::About,      &mut active_tab);
                });

                let rect = ui.min_rect();
                ui.painter().hline(rect.left()..=rect.right(), rect.bottom(), Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.sm);

                // ── Tab content ───────────────────────────────────────────
                egui::ScrollArea::vertical()
                    .max_height(ph - 130.0)
                    .show(ui, |ui| {
                        egui::Frame {
                            inner_margin: Margin::symmetric(sp.xl, sp.sm),
                            fill: Color32::TRANSPARENT,
                            ..Default::default()
                        }
                        .show(ui, |ui| {
                            match active_tab {
                                SettingsTab::Appearance => tab_appearance(ui, t, state, engine),
                                SettingsTab::Layout     => tab_layout(ui, t, state, engine),
                                SettingsTab::Behavior   => tab_behavior(ui, t, state, engine),
                                SettingsTab::Keybinds   => tab_keybinds(ui, t, engine),
                                SettingsTab::About      => tab_about(ui, t),
                            }
                        });
                    });

                // ── Footer ────────────────────────────────────────────────
                let rect = ui.available_rect_before_wrap();
                ui.painter().hline(rect.left()..=rect.right(), rect.top(), Stroke::new(1.0_f32, c.border()));
                egui::Frame {
                    inner_margin: Margin::symmetric(sp.xl, sp.sm),
                    fill: Color32::TRANSPARENT,
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Theme: {}", engine.active.meta.name))
                                .color(c.text_hint())
                                .size(t.typography.size_xs)
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(RichText::new("Close").size(t.typography.size_sm).color(c.text_secondary()))
                                    .fill(c.bg_surface0())
                                    .stroke(Stroke::new(1.0_f32, c.border()))
                                    .rounding(Rounding::same(s.radius_sm))
                            ).clicked() {
                                engine.save_active_theme();
                                state.panels.close();
                            }
                            ui.add_space(sp.sm);
                            if ui.add(
                                egui::Button::new(RichText::new("Save & Apply").size(t.typography.size_sm).color(Color32::WHITE))
                                    .fill(c.accent())
                                    .stroke(Stroke::NONE)
                                    .rounding(Rounding::same(s.radius_sm))
                            ).clicked() {
                                engine.save_active_theme();
                                state.notify.success("Tasarım ayarları kaydedildi".to_string());
                            }
                        });
                    });
                });
            });
        });

    ctx.data_mut(|d| d.insert_temp(tab_id, active_tab));

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        engine.save_active_theme();
        state.panels.close();
    }
}

// ── Tab button ───────────────────────────────────────────────────────────────

fn tab_btn(ui: &mut Ui, t: &AexTheme, label: &str, tab: SettingsTab, active: &mut SettingsTab) {
    let c = &t.colors;
    let is_active = *active == tab;
    let resp = ui.add(
        egui::Button::new(
            RichText::new(label)
                .color(if is_active { c.accent() } else { c.text_hint() })
                .size(t.typography.size_sm)
                .strong()
        )
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
    );
    if resp.clicked() { *active = tab; }
    if is_active {
        let r = resp.rect;
        ui.painter().hline(
            r.left()..=r.right(),
            r.bottom() + 2.0,
            Stroke::new(2.0_f32, c.accent()),
        );
    }
}

// ── Appearance Tab ────────────────────────────────────────────────────────────

fn tab_appearance(ui: &mut Ui, t: &AexTheme, state: &mut AppState, engine: &mut ThemeEngine) {
    let c  = &t.colors;
    let sp = &t.spacing;
    let s  = &t.shape;

    section(ui, t, "THEME");
    ui.add_space(sp.sm);

    // Tema listesi — her frame disk tarama yapma, sadece ilk açılışta yükle
    let themes_cache_id = egui::Id::new("settings_themes_cache");
    let themes: Vec<crate::theme::ThemeMeta> = {
        let cached = ui.ctx().data_mut(|d| d.get_temp::<Vec<crate::theme::ThemeMeta>>(themes_cache_id));
        if let Some(c) = cached { c } else {
            engine.refresh_available();
            let t_list = engine.available.clone();
            ui.ctx().data_mut(|d| d.insert_temp(themes_cache_id, t_list.clone()));
            t_list
        }
    };

    // Refresh butonu — manuel yenile
    if ui.add(egui::Button::new(
        RichText::new(format!("{} Refresh themes", icon(t, "refresh")))
            .color(t.colors.text_hint()).size(t.typography.size_xs))
        .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
        engine.refresh_available();
        let t_list = engine.available.clone();
        ui.ctx().data_mut(|d| d.insert_temp(themes_cache_id, t_list));
    }
    ui.add_space(sp.xs);

    let ctx = ui.ctx().clone();
    for theme_meta in &themes {
        let is_active = engine.active_id == theme_meta.id;
        theme_card(ui, t, theme_meta, is_active, || {
            engine.switch_to(&theme_meta.id, &ctx);
            // Refresh cache after switch
            ctx.data_mut(|d| d.remove::<Vec<crate::theme::ThemeMeta>>(themes_cache_id));
        });
        ui.add_space(sp.xs);
    }

    ui.add_space(sp.md);
    ui.label(
        RichText::new(format!("{} Add custom themes: ~/.config/arch-explorer/themes/*.aex", icon(t, "info")))
            .color(c.text_hint())
            .size(t.typography.size_xs)
    );

    ui.add_space(sp.lg);
    section(ui, t, "ICON STYLE");
    ui.add_space(sp.sm);

    use crate::theme::types::IconStyle;
    ui.horizontal(|ui| {
        for (label, style) in [("Nerd Font", IconStyle::Nerd), ("Emoji", IconStyle::Emoji), ("Minimal", IconStyle::Minimal)] {
            let is_active = engine.active.icons.style == style;
            if ui.add(
                egui::Button::new(RichText::new(label).size(t.typography.size_sm)
                    .color(if is_active { Color32::WHITE } else { c.text_secondary() }))
                    .fill(if is_active { c.accent() } else { c.bg_surface0() })
                    .stroke(Stroke::new(1.0_f32, if is_active { c.accent() } else { c.border() }))
                    .rounding(Rounding::same(s.radius_sm))
            ).clicked() {
                engine.active.icons.style = style;
                engine.save_active_theme();
                ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
            }
            ui.add_space(sp.xs);
        }
    });

    ui.add_space(sp.md);
    section(ui, t, "EFFECTS & TRANSPARENCY");
    ui.add_space(sp.sm);

    let mut glass = engine.active.effects.glass_panels;
    if ui.checkbox(&mut glass, RichText::new("Glass panels (compositor blur)").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.effects.glass_panels = glass;
        engine.save_active_theme();
        ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
        ui.ctx().request_repaint();
    }

    ui.add_space(sp.sm);

    // 1. Window Opacity
    ui.horizontal(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new("Window Opacity").color(c.text_secondary()).size(t.typography.size_sm));
        let mut opacity = engine.active.effects.window_opacity;
        if ui.add(egui::Slider::new(&mut opacity, 0.00..=1.00).step_by(0.01).custom_formatter(|n, _| format!("{:.0}%", n * 100.0))).changed() {
            engine.active.effects.window_opacity = opacity;
            engine.save_active_theme();
            ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
            ui.ctx().request_repaint();
        }
    });

    // 2. Sidebar Opacity
    ui.horizontal(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new("Sidebar Opacity").color(c.text_secondary()).size(t.typography.size_sm));
        let mut opacity = engine.active.effects.sidebar_opacity;
        if ui.add(egui::Slider::new(&mut opacity, 0.00..=1.00).step_by(0.01).custom_formatter(|n, _| format!("{:.0}%", n * 100.0))).changed() {
            engine.active.effects.sidebar_opacity = opacity;
            engine.save_active_theme();
            ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
            ui.ctx().request_repaint();
        }
    });

    // 3. Card & Item Opacity
    ui.horizontal(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new("File Cards Opacity").color(c.text_secondary()).size(t.typography.size_sm));
        let mut opacity = engine.active.effects.card_opacity;
        if ui.add(egui::Slider::new(&mut opacity, 0.00..=1.00).step_by(0.01).custom_formatter(|n, _| format!("{:.0}%", n * 100.0))).changed() {
            engine.active.effects.card_opacity = opacity;
            engine.save_active_theme();
            ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
            ui.ctx().request_repaint();
        }
    });

    // 4. Header & Toolbar Opacity
    ui.horizontal(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new("Toolbar & Tabs Opacity").color(c.text_secondary()).size(t.typography.size_sm));
        let mut opacity = engine.active.effects.toolbar_opacity;
        if ui.add(egui::Slider::new(&mut opacity, 0.00..=1.00).step_by(0.01).custom_formatter(|n, _| format!("{:.0}%", n * 100.0))).changed() {
            engine.active.effects.toolbar_opacity = opacity;
            engine.save_active_theme();
            ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
            ui.ctx().request_repaint();
        }
    });

    // 5. Modal & Panel Opacity
    ui.horizontal(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new("Dialog & Panel Opacity").color(c.text_secondary()).size(t.typography.size_sm));
        let mut opacity = engine.active.effects.panel_opacity;
        if ui.add(egui::Slider::new(&mut opacity, 0.00..=1.00).step_by(0.01).custom_formatter(|n, _| format!("{:.0}%", n * 100.0))).changed() {
            engine.active.effects.panel_opacity = opacity;
            engine.save_active_theme();
            ui.ctx().set_style(crate::theme::egui_bridge::aex_to_egui_style(&engine.active));
            ui.ctx().request_repaint();
        }
    });
}

fn theme_card(ui: &mut Ui, t: &AexTheme, meta: &ThemeMeta, is_active: bool, on_select: impl FnOnce()) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    egui::Frame {
        fill: if is_active {
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 20)
        } else {
            c.bg_surface0()
        },
        rounding: Rounding::same(s.radius_md),
        stroke: Stroke::new(1.0_f32, if is_active { c.accent() } else { c.border() }),
        inner_margin: Margin::symmetric(sp.md, sp.sm),
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&meta.name)
                        .color(if is_active { c.accent() } else { c.text_primary() })
                        .size(t.typography.size_sm)
                        .strong()
                );
                ui.label(
                    RichText::new(format!("by {}  {}", meta.author,
                        if meta.builtin { "• built-in" } else { "• custom" }))
                        .color(c.text_hint())
                        .size(t.typography.size_xs)
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if is_active {
                    ui.label(RichText::new(format!("{} Active", super::icon(t, "check"))).color(c.accent()).size(t.typography.size_xs));
                } else {
                    if ui.add(
                        egui::Button::new(RichText::new("Apply").size(t.typography.size_sm))
                            .fill(c.bg_surface1())
                            .stroke(Stroke::new(1.0_f32, c.border()))
                            .rounding(Rounding::same(s.radius_sm))
                    ).clicked() {
                        on_select();
                    }
                }
            });
        });
    });
}

// ── Layout Tab ────────────────────────────────────────────────────────────────

fn tab_layout(ui: &mut Ui, t: &AexTheme, state: &mut AppState, engine: &mut ThemeEngine) {
    let c  = &t.colors;
    let sp = &t.spacing;

    section(ui, t, "PANELS");
    ui.add_space(sp.sm);

    let mut sidebar_visible = engine.active.layout.sidebar_visible;
    if ui.checkbox(&mut sidebar_visible, RichText::new("Show sidebar").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.layout.sidebar_visible = sidebar_visible;
    }

    let mut show_breadcrumb = engine.active.layout.show_breadcrumb;
    if ui.checkbox(&mut show_breadcrumb, RichText::new("Use breadcrumb navigation (Address bar)").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.layout.show_breadcrumb = show_breadcrumb;
    }

    ui.add_space(sp.xs);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Sidebar width").color(c.text_secondary()).size(t.typography.size_sm));
        ui.add_space(sp.md);
        let mut w = engine.active.layout.sidebar_width;
        if ui.add(egui::Slider::new(&mut w, 120.0..=320.0)).changed() {
            engine.active.layout.sidebar_width = w;
        }
    });

    ui.add_space(sp.md);
    section(ui, t, "FILE GRID");
    ui.add_space(sp.sm);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Grid item size").color(c.text_secondary()).size(t.typography.size_sm));
        ui.add_space(sp.md);
        let mut size = engine.active.layout.grid_item_size;
        if ui.add(egui::Slider::new(&mut size, 60.0..=160.0)).changed() {
            engine.active.layout.grid_item_size = size;
        }
    });

    ui.add_space(sp.xs);
    let mut group_dirs = engine.active.file_grid.group_dirs_first;
    if ui.checkbox(&mut group_dirs, RichText::new("Folders first").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.file_grid.group_dirs_first = group_dirs;
        state.dir.sort();
    }

    let mut show_ext = engine.active.file_grid.show_extensions;
    if ui.checkbox(&mut show_ext, RichText::new("Show extensions").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.file_grid.show_extensions = show_ext;
    }

    let mut thumbnails = engine.active.file_grid.thumbnail_enabled;
    if ui.checkbox(&mut thumbnails, RichText::new("Show thumbnails").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.file_grid.thumbnail_enabled = thumbnails;
    }

    ui.add_space(sp.md);
    section(ui, t, "LIST VIEW COLUMNS");
    ui.add_space(sp.sm);

    let mut cols_changed = false;
    ui.label(RichText::new("Liste görünümünde görüntülenecek sütunlar:").color(c.text_hint()).size(t.typography.size_xs));
    ui.add_space(sp.xs);

    cols_changed |= ui.checkbox(&mut state.list_columns.show_type, RichText::new("Dosya Türü (Type)").color(c.text_secondary()).size(t.typography.size_sm)).changed();
    cols_changed |= ui.checkbox(&mut state.list_columns.show_size, RichText::new("Boyut (Size)").color(c.text_secondary()).size(t.typography.size_sm)).changed();
    cols_changed |= ui.checkbox(&mut state.list_columns.show_perm, RichText::new("İzinler (Permissions)").color(c.text_secondary()).size(t.typography.size_sm)).changed();
    cols_changed |= ui.checkbox(&mut state.list_columns.show_modified, RichText::new("Değiştirilme Tarihi (Modified)").color(c.text_secondary()).size(t.typography.size_sm)).changed();
    cols_changed |= ui.checkbox(&mut state.list_columns.show_accessed, RichText::new("Erişim / Oluşturulma (Accessed / Created)").color(c.text_secondary()).size(t.typography.size_sm)).changed();

    if cols_changed {
        state.save_list_columns();
    }
}

// ── Behavior Tab ──────────────────────────────────────────────────────────────

fn tab_behavior(ui: &mut Ui, t: &AexTheme, state: &mut AppState, engine: &mut ThemeEngine) {
    let c  = &t.colors;
    let sp = &t.spacing;

    section(ui, t, "FILE OPERATIONS");
    ui.add_space(sp.sm);

    let mut show_hidden = state.dir.show_hidden;
    if ui.checkbox(&mut show_hidden, RichText::new("Show hidden files by default").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        state.dir.show_hidden = show_hidden;
        state.refresh();
    }

    ui.add_space(sp.md);
    section(ui, t, "PREVIEW PANEL BEHAVIOR");
    ui.add_space(sp.sm);

    ui.horizontal(|ui| {
        if ui.radio(state.preview_mode == crate::state::PreviewBehaviorMode::AlwaysOpen,
            RichText::new("Her Zaman Açık (Always Open)").color(c.text_secondary()).size(t.typography.size_sm)).clicked() {
            state.preview_mode = crate::state::PreviewBehaviorMode::AlwaysOpen;
            state.save_preview_settings();
        }
    });
    ui.horizontal(|ui| {
        if ui.radio(state.preview_mode == crate::state::PreviewBehaviorMode::AutoOnSelect,
            RichText::new("Otomatik (Dosya/Klasör Seçilince Aç / Auto on Select)").color(c.text_secondary()).size(t.typography.size_sm)).clicked() {
            state.preview_mode = crate::state::PreviewBehaviorMode::AutoOnSelect;
            state.save_preview_settings();
        }
    });
    ui.horizontal(|ui| {
        if ui.radio(state.preview_mode == crate::state::PreviewBehaviorMode::Custom,
            RichText::new("Özel Mod (Türe Göre Seçim / Custom by Type)").color(c.text_secondary()).size(t.typography.size_sm)).clicked() {
            state.preview_mode = crate::state::PreviewBehaviorMode::Custom;
            state.save_preview_settings();
        }
    });

    if state.preview_mode == crate::state::PreviewBehaviorMode::Custom {
        ui.add_space(sp.xs);
        ui.indent("preview_custom_types", |ui| {
            ui.label(RichText::new("Önizlemenin otomatik açılacağı türler:").color(c.text_hint()).size(t.typography.size_xs));
            ui.add_space(sp.xs);
            let mut changed = false;
            changed |= ui.checkbox(&mut state.preview_custom_image, RichText::new("🖼 Resimler (Images)").color(c.text_secondary()).size(t.typography.size_xs)).changed();
            changed |= ui.checkbox(&mut state.preview_custom_video, RichText::new("🎬 Videolar (Videos)").color(c.text_secondary()).size(t.typography.size_xs)).changed();
            changed |= ui.checkbox(&mut state.preview_custom_audio, RichText::new("🎵 Sesler (Audio)").color(c.text_secondary()).size(t.typography.size_xs)).changed();
            changed |= ui.checkbox(&mut state.preview_custom_text,  RichText::new("📝 Metin & Kod (Text & Code)").color(c.text_secondary()).size(t.typography.size_xs)).changed();
            changed |= ui.checkbox(&mut state.preview_custom_doc,   RichText::new("📄 Belgeler / PDF (Documents)").color(c.text_secondary()).size(t.typography.size_xs)).changed();
            changed |= ui.checkbox(&mut state.preview_custom_dir,   RichText::new("📁 Klasörler (Directories)").color(c.text_secondary()).size(t.typography.size_xs)).changed();
            if changed {
                state.save_preview_settings();
            }
        });
    }

    ui.add_space(sp.md);
    section(ui, t, "SIDEBAR");
    ui.add_space(sp.sm);

    let mut show_drives = engine.active.sidebar.show_drives;
    if ui.checkbox(&mut show_drives, RichText::new("Show drives").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.sidebar.show_drives = show_drives;
    }

    let mut show_bm = engine.active.sidebar.show_bookmarks;
    if ui.checkbox(&mut show_bm, RichText::new("Show bookmarks").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.sidebar.show_bookmarks = show_bm;
    }

    ui.add_space(sp.md);
    section(ui, t, "ANIMATIONS");
    ui.add_space(sp.sm);

    let mut anim = engine.active.animations.enabled;
    if ui.checkbox(&mut anim, RichText::new("Enable animations").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        engine.active.animations.enabled = anim;
    }

    ui.add_space(sp.md);
    section(ui, t, "FOLDER SIZE CACHE");
    ui.add_space(sp.sm);

    let count = crate::backend::du_cache::cache_entry_count();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} directories cached", count))
            .color(c.text_hint()).size(t.typography.size_xs));
        ui.add_space(sp.sm);
        if ui.add(egui::Button::new(
            RichText::new("Clear Cache").color(c.danger()).size(t.typography.size_xs))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0_f32, c.danger()))).clicked() {
            crate::backend::du_cache::clear_all();
            state.du_sizes.clear();
        }
    });
    ui.label(RichText::new("Cache is auto-invalidated by inotify on file changes")
        .color(c.text_disabled()).size(t.typography.size_xs).italics());

    ui.add_space(sp.md);
    section(ui, t, "THUMBNAIL CACHE");
    ui.add_space(sp.sm);
    let thumb_count = crate::backend::thumbnails::cache_entry_count();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} thumbnails cached  (~{})",
            thumb_count,
            {
                let kb = thumb_count * 20; // rough 20KB avg
                if kb > 1024 { format!("{} MB", kb/1024) } else { format!("{} KB", kb) }
            }))
            .color(c.text_hint()).size(t.typography.size_xs));
        ui.add_space(sp.sm);
        if ui.add(egui::Button::new(
            RichText::new("Clear").color(c.danger()).size(t.typography.size_xs))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0_f32, c.danger()))).clicked() {
            crate::backend::thumbnails::clear_cache();
        }
    });

    ui.add_space(sp.md);
    section(ui, t, "PERFORMANCE & RAM OPTIMIZATION");
    ui.add_space(sp.sm);

    let mut fast_load = state.perf_settings.fast_directory_load;
    if ui.checkbox(&mut fast_load, RichText::new("Hızlı Dizin Taraması (Fast Directory Load)").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        state.perf_settings.fast_directory_load = fast_load;
        state.save_perf_settings();
    }
    ui.label(RichText::new("Aşırı fazla dosya olan klasörlerde gereksiz sistem çağrılarını atlayarak anında yükler")
        .color(c.text_disabled()).size(t.typography.size_xs).italics());

    ui.add_space(sp.xs);
    let mut low_mem = state.perf_settings.low_memory_mode;
    if ui.checkbox(&mut low_mem, RichText::new("Düşük RAM Modu (Low Memory Mode)").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        state.perf_settings.low_memory_mode = low_mem;
        state.save_perf_settings();
    }
    ui.label(RichText::new("Ekrandan çıkan thumbnail ve önizleme dokularını anında bellekten boşaltır")
        .color(c.text_disabled()).size(t.typography.size_xs).italics());

    ui.add_space(sp.xs);
    ui.horizontal(|ui| {
        ui.label(RichText::new("RAM Belleğini Temizle:").color(c.text_secondary()).size(t.typography.size_sm));
        ui.add_space(sp.sm);
        if ui.add(egui::Button::new(
            RichText::new("Önizleme & Doku Belleğini Boşalt").size(t.typography.size_xs))
            .fill(c.bg_surface1())
            .stroke(Stroke::new(1.0_f32, c.border()))).clicked() {
            state.preview_cache.clear();
            ui.ctx().data_mut(|d| {
                d.remove::<std::collections::VecDeque<String>>(egui::Id::new("grid_thumb_lru_keys"));
                d.remove::<std::collections::VecDeque<String>>(egui::Id::new("list_thumb_lru_keys"));
            });
            state.notify.info("Önizleme ve doku bellek havuzu boşaltıldı".into());
        }
    });

    ui.add_space(sp.md);
    section(ui, t, "ROOT PRIVILEGES");
    ui.add_space(sp.sm);

    let mut always_root = state.always_root;
    if ui.checkbox(&mut always_root, RichText::new("Her Zaman Root Olarak Aç (Always Open as Root)").color(c.text_secondary()).size(t.typography.size_sm)).changed() {
        state.always_root = always_root;
        state.save_root_settings();
    }
    ui.label(RichText::new("Etkinleştirildiğinde Arch Explorer her açılışta root yetkisi ister")
        .color(c.text_disabled()).size(t.typography.size_xs).italics());

    ui.add_space(sp.xs);
    ui.horizontal(|ui| {
        let is_elevated = state.is_root_active || crate::backend::fs_ops::get_root_password().is_some();
        if is_elevated {
            ui.label(RichText::new("🔐 Root Yetkisi Aktif (Geçici)").color(c.accent()).size(t.typography.size_sm).strong());
            ui.add_space(sp.sm);
            if ui.add(egui::Button::new(RichText::new("Yetkiyi Bırak").size(t.typography.size_xs))
                .fill(c.bg_surface1())
                .stroke(Stroke::new(1.0, c.border()))).clicked() {
                crate::backend::fs_ops::clear_root_password();
                state.is_root_active = false;
                state.notify.info("Root yetkisi kaldırıldı".into());
                state.refresh();
            }
        } else {
            if ui.add(egui::Button::new(RichText::new("🔑 Root Ol (Bu Oturum İçin)").size(t.typography.size_sm).strong())
                .fill(c.accent())
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(t.shape.radius_sm))).clicked() {
                state.pending_root_auth = Some(crate::state::RootAuthRequest {
                    title: "Bu oturum için Root yetkisi al".to_string(),
                    cmd: "true".to_string(),
                    args: vec![],
                    reload: true,
                    auth_type: Some("become_root".to_string()),
                });
                state.pending_root_wrong = false;
            }
        }
    });
    ui.label(RichText::new("Geçici root yetkisi uygulama kapatılınca kendiliğinden sonlanır")
        .color(c.text_disabled()).size(t.typography.size_xs).italics());
}

// ── Keybinds Tab ─────────────────────────────────────────────────────────────

fn tab_keybinds(ui: &mut Ui, t: &AexTheme, engine: &mut ThemeEngine) {
    let c  = &t.colors;
    let sp = &t.spacing;

    section(ui, t, "KEYBOARD SHORTCUTS");
    ui.add_space(sp.sm);
    ui.label(RichText::new("Edit ~/.config/arch-explorer/themes/your-theme.aex to customize keybinds").color(c.text_hint()).size(t.typography.size_xs));
    ui.add_space(sp.md);

    let kb = &engine.active.keybinds;
    let rows = [
        ("Back",           &kb.back),
        ("Forward",        &kb.forward),
        ("Go Up",          &kb.go_up),
        ("Home",           &kb.go_home),
        ("New Folder",     &kb.new_folder),
        ("New File",       &kb.new_file),
        ("Copy",           &kb.copy),
        ("Cut",            &kb.cut),
        ("Paste",          &kb.paste),
        ("Undo",           &kb.undo),
        ("Rename",         &kb.rename),
        ("Delete",         &kb.delete),
        ("Select All",     &kb.select_all),
        ("Search",         &kb.search),
        ("Toggle Hidden",  &kb.toggle_hidden),
        ("Toggle Preview", &kb.toggle_preview),
        ("Settings",       &kb.settings),
        ("Refresh",        &kb.refresh),
        ("Terminal",       &kb.open_terminal),
    ];

    for (label, binding) in &rows {
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(*label).color(c.text_secondary()).size(t.typography.size_sm));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::Frame {
                    fill: c.bg_surface0(),
                    rounding: Rounding::same(t.shape.radius_sm),
                    stroke: Stroke::new(1.0_f32, c.border()),
                    inner_margin: Margin::symmetric(sp.sm, 2.0),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.label(RichText::new(*binding).color(c.text_hint()).size(t.typography.size_xs).monospace());
                });
            });
        });
        ui.add_space(sp.xs);
    }
}

// ── About Tab ─────────────────────────────────────────────────────────────────

fn tab_about(ui: &mut Ui, t: &AexTheme) {
    let c  = &t.colors;
    let sp = &t.spacing;

    ui.add_space(sp.lg);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("Arch Explorer").color(c.text_primary()).size(t.typography.size_2xl).strong());
        ui.add_space(sp.xs);
        ui.label(RichText::new("v0.2.0 — egui native").color(c.text_hint()).size(t.typography.size_sm));
        ui.add_space(sp.lg);

        let stats = [
            ("Renderer",  "egui + wgpu (Wayland native)"),
            ("RAM",       "~50-60mb PSS (no webkit)"),
            ("Theming",   ".aex format (TOML based)"),
            ("Backend",   "Pure Rust, async tokio"),
            ("Mounts",    "zbus → ntfs3 → ntfs-3g → pkexec → sudo"),
        ];

        for (k, v) in &stats {
            ui.horizontal(|ui| {
                ui.label(RichText::new(*k).color(c.text_hint()).size(t.typography.size_sm));
                ui.add_space(sp.md);
                ui.label(RichText::new(*v).color(c.text_secondary()).size(t.typography.size_sm));
            });
            ui.add_space(sp.xs);
        }

        ui.add_space(sp.xl);
        ui.label(RichText::new("by lordblue").color(c.text_hint()).size(t.typography.size_xs));
        ui.label(RichText::new("AEX theme engine — extractable as standalone crate").color(c.text_hint()).size(t.typography.size_xs));
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section(ui: &mut Ui, t: &AexTheme, label: &str) {
    let c = &t.colors;
    ui.label(
        RichText::new(label)
            .color(c.text_hint())
            .size(t.typography.size_xs)
            .strong()
    );
    let rect = ui.min_rect();
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.border()),
    );
}
