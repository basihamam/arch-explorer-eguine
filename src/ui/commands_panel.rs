// src/ui/commands_panel.rs
// Özel komutlar yöneticisi — commands.toml UI
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use crate::backend::commands::{UserCommand, load_user_commands, save_user_command, run_user_command};
use super::icon;

#[derive(Clone, PartialEq)]
enum CmdTab { List, New }

#[derive(Clone)]
struct CmdPanelState {
    tab:     CmdTab,
    commands: Vec<UserCommand>,
    // New/edit form
    editing: Option<usize>,
    form:    UserCommand,
    // Run output
    last_output: Option<(bool, String)>,
}

impl CmdPanelState {
    fn new() -> Self {
        Self {
            tab: CmdTab::List,
            commands: load_user_commands(),
            editing: None,
            form: blank_cmd(),
            last_output: None,
        }
    }
    fn reset_form(&mut self) { self.form = blank_cmd(); self.editing = None; }
}

fn blank_cmd() -> UserCommand {
    UserCommand {
        id: uuid_short(),
        name: String::new(),
        description: String::new(),
        exec: String::new(),
        icon: "terminal".into(),
        show_in_context_menu: true,
        show_in_toolbar: false,
        shortcut: None,
        working_dir: None,
        terminal: false,
        category: "tools".into(),
    }
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!("cmd_{}", SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis())
}

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();
    let pw = 560.0_f32; let ph = 520.0_f32;
    let d_rect = Rect::from_center_size(screen.center(), Vec2::new(pw, ph));

    egui::Area::new(Id::new("cmd_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0,0,0,130));
            let r = ui.allocate_rect(screen, Sense::click());
            let click_pos = ctx.input(|i| i.pointer.interact_pos());
            if r.clicked() && click_pos.map(|p| !d_rect.contains(p) && p.y > 36.0).unwrap_or(false) {
                state.panels.close();
            }
        });
    let state_id = Id::new("cmd_panel_state");
    let mut ps: CmdPanelState = ctx.data_mut(|d|
        d.get_temp(state_id).unwrap_or_else(CmdPanelState::new));

    egui::Area::new(Id::new("cmd_panel"))
        .fixed_pos(d_rect.min)
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::ZERO,
                shadow: epaint::Shadow { offset: Vec2::new(0.0,8.0), blur: 24.0,
                    spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw);
                ui.set_height(ph);

                // Header
                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md),
                    fill: Color32::TRANSPARENT, ..Default::default() }
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} Custom Commands", icon(t, "terminal")))
                            .color(c.accent()).size(t.typography.size_lg).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("✕").clicked() { state.panels.close(); }
                        });
                    });
                });

                // Tabs
                ui.horizontal(|ui| {
                    ui.add_space(sp.xl);
                    for (tab, label) in [(CmdTab::List, "Commands"), (CmdTab::New, "New Command")] {
                        let active = ps.tab == tab;
                        if ui.add(Button::new(RichText::new(label)
                            .color(if active { c.accent() } else { c.text_secondary() })
                            .size(t.typography.size_sm))
                            .fill(if active {
                                Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20)
                            } else { Color32::TRANSPARENT })
                            .stroke(Stroke::NONE)).clicked() {
                            ps.tab = tab;
                        }
                    }
                });
                let r = ui.min_rect();
                ui.painter().hline(r.left()..=r.right(), r.bottom(),
                    Stroke::new(1.0_f32, c.border()));

                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md),
                    fill: Color32::TRANSPARENT, ..Default::default() }
                .show(ui, |ui| {
                    ui.set_width(pw - sp.xl * 2.0);
                    match ps.tab {
                        CmdTab::List => tab_list(ui, t, state, &mut ps),
                        CmdTab::New  => tab_new(ui, t, &mut ps),
                    }
                });
            });
        });

    ctx.data_mut(|d| d.insert_temp(state_id, ps));
    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

// ── Command list tab ──────────────────────────────────────────────────────────

fn tab_list(ui: &mut Ui, t: &AexTheme, state: &mut AppState, ps: &mut CmdPanelState) {
    let c  = &t.colors; let s = &t.shape; let sp = &t.spacing;

    if ps.commands.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new(icon(t, "terminal")).color(c.text_disabled()).size(32.0));
            ui.add_space(sp.sm);
            ui.label(RichText::new("No commands yet").color(c.text_hint()).size(t.typography.size_sm));
            ui.add_space(sp.sm);
            if ui.add(Button::new(RichText::new("+ New Command").color(c.accent()).size(t.typography.size_sm))
                .fill(Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20))
                .rounding(Rounding::same(s.radius_sm)).stroke(Stroke::new(1.0_f32, c.accent()))).clicked() {
                ps.tab = CmdTab::New;
            }
        });
        return;
    }

    // Last output
    if let Some((ok, ref out)) = ps.last_output.clone() {
        let color = if ok { c.success() } else { c.danger() };
        egui::Frame { fill: c.bg_crust(), rounding: Rounding::same(s.radius_sm),
            inner_margin: Margin::same(sp.sm), ..Default::default() }
        .show(ui, |ui| {
            ui.set_width(f32::INFINITY);
            egui::ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
                ui.label(RichText::new(out).color(color).size(t.typography.size_xs).monospace());
            });
        });
        ui.add_space(sp.sm);
    }

    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
        let cmds = ps.commands.clone();
        let mut to_delete: Option<usize> = None;
        let mut to_edit:   Option<usize> = None;

        for (i, cmd) in cmds.iter().enumerate() {
            egui::Frame { fill: c.bg_surface0(), rounding: Rounding::same(s.radius_md),
                inner_margin: Margin::same(sp.sm), ..Default::default() }
            .show(ui, |ui| {
                ui.set_width(f32::INFINITY);
                ui.horizontal(|ui| {
                    // Icon
                    ui.label(RichText::new(icon(t, &cmd.icon)).color(c.accent()).size(16.0));
                    ui.add_space(sp.xs);
                    // Name + desc
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&cmd.name).color(c.text_primary())
                            .size(t.typography.size_sm).strong());
                        if !cmd.description.is_empty() {
                            ui.label(RichText::new(&cmd.description).color(c.text_hint())
                                .size(t.typography.size_xs));
                        }
                        ui.label(RichText::new(&cmd.exec).color(c.text_hint())
                            .size(t.typography.size_xs).monospace());
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Delete
                        if ui.add(Button::new(RichText::new("✕").color(c.danger()).size(12.0))
                            .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                            to_delete = Some(i);
                        }
                        // Edit
                        if ui.add(Button::new(RichText::new("✎").color(c.text_hint()).size(12.0))
                            .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                            to_edit = Some(i);
                        }
                        // Run
                        if ui.add(Button::new(RichText::new(format!("{} Run", icon(t, "terminal")))
                            .color(c.accent()).size(t.typography.size_xs))
                            .fill(Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20))
                            .rounding(Rounding::same(s.radius_sm)).stroke(Stroke::NONE)).clicked() {
                            let cmd_id = cmd.id.clone();
                            let sel = state.selected_paths().first().cloned();
                            let cur_dir = state.nav.current_path.clone();
                            std::thread::spawn(move || {
                                crate::backend::commands::run_user_command(cmd_id, sel, cur_dir).ok();
                            });
                        }
                        ui.add_space(sp.xs);
                        // Badges
                        if cmd.show_in_context_menu {
                            ui.label(RichText::new("ctx").color(c.text_hint()).size(9.0));
                        }
                        if cmd.show_in_toolbar {
                            ui.label(RichText::new("bar").color(c.text_hint()).size(9.0));
                        }
                    });
                });
            });
            ui.add_space(sp.xs);
        }

        if let Some(i) = to_delete {
            let id = ps.commands[i].id.clone();
            crate::backend::commands::delete_user_command(id).ok();
            ps.commands = load_user_commands();
        }
        if let Some(i) = to_edit {
            ps.form    = ps.commands[i].clone();
            ps.editing = Some(i);
            ps.tab     = CmdTab::New;
        }
    });

    ui.add_space(sp.sm);
    if ui.add(Button::new(RichText::new("+ New Command").color(c.accent()).size(t.typography.size_sm))
        .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, c.accent()))
        .rounding(Rounding::same(s.radius_sm))).clicked() {
        ps.reset_form(); ps.tab = CmdTab::New;
    }
}

// ── New / Edit command tab ────────────────────────────────────────────────────

fn tab_new(ui: &mut Ui, t: &AexTheme, ps: &mut CmdPanelState) {
    let c  = &t.colors; let s = &t.shape; let sp = &t.spacing;
    let is_edit = ps.editing.is_some();

    field(ui, t, "Name", |ui| {
        ui.add(egui::TextEdit::singleline(&mut ps.form.name)
            .hint_text("Open Terminal Here")
            .desired_width(f32::INFINITY)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional)));
    });

    field(ui, t, "Command", |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut ps.form.exec)
                .hint_text("kitty --directory {dir}  or  git status")
                .desired_width(f32::INFINITY)
                .font(FontId::new(t.typography.size_sm, FontFamily::Monospace)));
        });
    });

    // Placeholder helper tokens
    ui.horizontal_wrapped(|ui| {
        for tok in &["{path}", "{file}", "{dir}", "{name}"] {
            if ui.add(Button::new(RichText::new(*tok).size(t.typography.size_xs)
                .color(c.accent()).monospace())
                .fill(Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),15))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),60)))
                .rounding(Rounding::same(s.radius_sm))).clicked() {
                ps.form.exec.push_str(tok);
            }
        }
    });

    ui.add_space(sp.sm);

    field(ui, t, "Description", |ui| {
        ui.add(egui::TextEdit::singleline(&mut ps.form.description)
            .hint_text("What does this command do?")
            .desired_width(f32::INFINITY)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional)));
    });

    field(ui, t, "Category", |ui| {
        ui.add(egui::TextEdit::singleline(&mut ps.form.category)
            .hint_text("tools, git, media…")
            .desired_width(200.0)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional)));
    });

    field(ui, t, "Shortcut", |ui| {
        let mut sc = ps.form.shortcut.clone().unwrap_or_default();
        if ui.add(egui::TextEdit::singleline(&mut sc)
            .hint_text("ctrl+shift+t  (optional)")
            .desired_width(200.0)
            .font(FontId::new(t.typography.size_sm, FontFamily::Monospace))).changed() {
            ps.form.shortcut = if sc.is_empty() { None } else { Some(sc) };
        }
    });

    ui.add_space(sp.sm);
    ui.horizontal(|ui| {
        ui.checkbox(&mut ps.form.show_in_context_menu,
            RichText::new("Show in right-click menu").color(c.text_secondary()).size(t.typography.size_sm));
        ui.add_space(sp.md);
        ui.checkbox(&mut ps.form.show_in_toolbar,
            RichText::new("Show in toolbar").color(c.text_secondary()).size(t.typography.size_sm));
        ui.add_space(sp.md);
        ui.checkbox(&mut ps.form.terminal,
            RichText::new("Run in terminal").color(c.text_secondary()).size(t.typography.size_sm));
    });

    ui.add_space(sp.lg);
    ui.horizontal(|ui| {
        if ui.add(Button::new(RichText::new("Cancel").color(c.text_secondary()).size(t.typography.size_sm))
            .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
            ps.reset_form(); ps.tab = CmdTab::List;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let can_save = !ps.form.name.is_empty() && !ps.form.exec.is_empty();
            let label = if is_edit { "✓ Save Changes" } else { "✓ Add Command" };
            if ui.add_enabled(can_save, Button::new(
                RichText::new(label).color(Color32::WHITE).size(t.typography.size_sm))
                .fill(if can_save { c.accent() } else { c.bg_surface1() })
                .rounding(Rounding::same(s.radius_sm)).stroke(Stroke::NONE)).clicked() {
                save_user_command(ps.form.clone()).ok();
                // Reload from disk
                ps.commands = load_user_commands();
                ps.reset_form();
                ps.tab = CmdTab::List;
            }
        });
    });
}

fn field(ui: &mut Ui, t: &AexTheme, label: &str, content: impl FnOnce(&mut Ui)) {
    ui.label(RichText::new(label).color(t.colors.text_hint())
        .size(t.typography.size_xs).strong());
    ui.add_space(2.0);
    content(ui);
    ui.add_space(t.spacing.sm);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commands_panel_render() {
        let ctx = Context::default();
        let theme = AexTheme::default();
        let mut state = AppState::new();
        state.panels.open_commands();
        let _ = ctx.run(Default::default(), |ctx| {
            render(ctx, &theme, &mut state);
        });
    }
}


#[test]
fn test_commands_panel_tab_new() {
    let ctx = Context::default();
    let theme = AexTheme::default();
    let mut state = AppState::new();
    state.panels.open_commands();
    let _ = ctx.run(Default::default(), |ctx| {
        render(ctx, &theme, &mut state);
    });
}
