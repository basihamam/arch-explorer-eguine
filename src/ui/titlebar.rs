// ─────────────────────────────────────────────────────────────────────────────
// src/ui/titlebar.rs — Custom window titlebar matching screenshot design
// ─────────────────────────────────────────────────────────────────────────────

use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    let c = &t.colors;
    let h = 34.0;  // titlebar height matching screenshot

    let fill_color = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_mantle().r(), c.bg_mantle().g(), c.bg_mantle().b(),
            (t.effects.toolbar_opacity * 255.0) as u8,
        )
    } else {
        c.bg_mantle()
    };

    egui::TopBottomPanel::top("titlebar")
        .exact_height(h)
        .show_separator_line(false)
        .frame(egui::Frame {
            fill: fill_color,
            inner_margin: Margin::ZERO,
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.set_height(h);

            let full_rect = ui.available_rect_before_wrap();
            let right_w = 90.0_f32;
            let right_rect = Rect::from_min_size(
                Pos2::new(full_rect.right() - right_w, full_rect.top()),
                Vec2::new(right_w, h),
            );

            let center_w = 560.0_f32;
            let left_w = 140.0_f32;
            let available_center = (full_rect.width() - left_w - right_w - 24.0).max(0.0);
            let actual_center_w = center_w.min(available_center);
            let show_center = actual_center_w >= 140.0;
            let center_x = if show_center {
                let ideal_center_x = full_rect.center().x - (actual_center_w / 2.0);
                let min_x = left_w + 12.0;
                let max_x = full_rect.right() - right_w - actual_center_w - 6.0;
                ideal_center_x.clamp(min_x, max_x.max(min_x))
            } else {
                -1000.0
            };
            let center_rect = if show_center {
                Rect::from_min_size(
                    Pos2::new(center_x, full_rect.top()),
                    Vec2::new(actual_center_w, h),
                )
            } else {
                Rect::NOTHING
            };

            // Drag area for window movement — only if not hovering buttons
            let p_pos = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
            let over_buttons = p_pos.map(|p| center_rect.contains(p) || right_rect.contains(p)).unwrap_or(false);

            if !over_buttons {
                let drag_resp = ui.interact(
                    ui.available_rect_before_wrap(),
                    ui.id().with("titlebar_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.dragged() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(
                        !ctx.input(|i| i.viewport().maximized.unwrap_or(false))
                    ));
                }
            }

            // ── Left: App Icon & Name ──────────────────────────────────────
            ui.horizontal_centered(|ui| {
                ui.add_space(t.spacing.md);
                ui.label(
                    RichText::new("◈")
                        .color(c.accent())
                        .size(15.0)
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Arch Explorer")
                        .color(c.text_primary())
                        .size(t.typography.size_sm)
                        .strong(),
                );
            });

            // ── Center: Action buttons (Disks, Network, Panes, Plugins, Commands, Settings) ──
            if show_center {
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(center_rect), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                        // Disks
                        let disks_active = state.panels.active == crate::state::ActivePanel::Mount;
                        if action_btn(ui, t, action_icon(t, "disks"), "Disks", disks_active).clicked() {
                            if disks_active { state.panels.close(); }
                            else { state.panels.open_mount(); }
                        }

                        // Network
                        let net_active = state.panels.active == crate::state::ActivePanel::Network;
                        if action_btn(ui, t, action_icon(t, "network"), "Network", net_active).clicked() {
                            if net_active { state.panels.close(); }
                            else { state.panels.open_network(); }
                        }

                        // Panes (Dual Pane toggle)
                        let panes_active = state.dual_pane;
                        if action_btn(ui, t, action_icon(t, "panes"), "Panes", panes_active).clicked() {
                            if state.dual_pane { state.close_dual_pane(); }
                            else { state.open_dual_pane(); }
                        }

                        // Plugins
                        let plugins_active = false;
                        if action_btn(ui, t, action_icon(t, "plugins"), "Plugins", plugins_active).clicked() {
                            state.notify.info("No external plugins configured".to_string());
                        }

                        // Commands
                        let cmd_active = state.panels.active == crate::state::ActivePanel::Commands;
                        if action_btn(ui, t, action_icon(t, "commands"), "Commands", cmd_active).clicked() {
                            if cmd_active { state.panels.close(); }
                            else { state.panels.open_commands(); }
                        }

                        // Settings
                        let set_active = state.panels.active == crate::state::ActivePanel::Settings;
                        if action_btn(ui, t, action_icon(t, "settings"), "Settings", set_active).clicked() {
                            if set_active { state.panels.close(); }
                            else { state.panels.open_settings(); }
                        }
                    });
                });
            }

            // ── Right: Window Controls ─────────────────────────────────────
            let right_w = 90.0_f32;
            let right_rect = Rect::from_min_size(
                Pos2::new(full_rect.right() - right_w, full_rect.top()),
                Vec2::new(right_w, h),
            );

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    // Minimize
                    if window_ctrl_btn(ui, t, "—", c.text_hint(), false).on_hover_text("Minimize").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                    // Maximize
                    let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    if window_ctrl_btn(ui, t, if is_max { "❐" } else { "□" }, c.text_hint(), false)
                        .on_hover_text(if is_max { "Restore" } else { "Maximize" })
                        .clicked()
                    {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_max));
                    }
                    // Close
                    if window_ctrl_btn(ui, t, "✕", c.danger(), true).on_hover_text("Close (Alt+F4)").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });

            // Bottom border line
            ui.painter().hline(
                full_rect.left()..=full_rect.right(),
                full_rect.bottom(),
                Stroke::new(1.0_f32, c.border()),
            );
        });
}

fn action_btn(ui: &mut Ui, t: &AexTheme, icon_str: &str, label: &str, is_active: bool) -> Response {
    let c = &t.colors;
    let s = &t.shape;

    let color = if is_active {
        c.accent()
    } else {
        c.text_secondary()
    };

    let font_id = FontId::new(t.typography.size_xs, FontFamily::Proportional);
    let text_w = ui.painter().layout_no_wrap(label.to_string(), font_id.clone(), color).size().x;
    let btn_w = (text_w + 32.0).max(76.0);
    let btn_h = 24.0_f32;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(btn_w, btn_h), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if is_active {
            Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 35)
        } else if resp.hovered() {
            c.bg_surface0()
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);

        let cy = rect.center().y;
        let icon_x = rect.min.x + 11.0;
        let text_x = rect.min.x + 23.0;

        ui.painter().text(
            Pos2::new(icon_x, cy),
            Align2::CENTER_CENTER,
            icon_str,
            FontId::new(t.typography.size_xs + 1.0, FontFamily::Monospace),
            color,
        );
        ui.painter().text(
            Pos2::new(text_x, cy),
            Align2::LEFT_CENTER,
            label,
            font_id,
            color,
        );
    }

    resp
}

fn window_ctrl_btn(ui: &mut Ui, t: &AexTheme, label: &str, color: Color32, is_close: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if is_close && resp.hovered() {
            Color32::from_rgb(232, 17, 35)
        } else if resp.hovered() {
            Color32::from_rgba_unmultiplied(255, 255, 255, 20)
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, Rounding::ZERO, bg);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(12.0, FontFamily::Proportional),
            if is_close && resp.hovered() { Color32::WHITE } else { color },
        );
    }

    resp
}

fn action_icon(t: &AexTheme, key: &str) -> &'static str {
    match t.icons.style {
        crate::theme::types::IconStyle::Nerd => match key {
            "disks"    => "\u{f1c0}",
            "network"  => "\u{f1e0}",
            "panes"    => "\u{f0db}",
            "plugins"  => "\u{f005}",
            "commands" => "\u{f120}",
            "settings" => "\u{f013}",
            _ => "",
        },
        _ => match key {
            "disks"    => "⛁",
            "network"  => "☊",
            "panes"    => "⚏",
            "plugins"  => "★",
            "commands" => ">_",
            "settings" => "⚙",
            _ => "",
        },
    }
}
