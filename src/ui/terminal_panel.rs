// src/ui/terminal_panel.rs
// Authentic minimalist integrated terminal drawer

use egui::*;
use crate::theme::types::AexTheme;
use crate::state::{AppState, TerminalMsgKind, TerminalLine};
use crate::app::BackendMsg;
use std::sync::mpsc::Sender;

fn get_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "aiuser".to_string())
}

fn get_hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "myarch".to_string())
}

fn get_folder_name(cwd: &str) -> String {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if !home.is_empty() && (cwd == home || cwd == format!("{}/", home)) {
        "~".to_string()
    } else if cwd == "/" {
        "/".to_string()
    } else {
        std::path::Path::new(cwd)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.to_string())
    }
}

fn is_interactive_command(cmd: &str) -> bool {
    let first = cmd.split_whitespace().next().unwrap_or("");
    let base = std::path::Path::new(first).file_name().and_then(|n| n.to_str()).unwrap_or(first);
    matches!(base,
        "cava" | "htop" | "btop" | "top" | "nvtop" |
        "vim" | "vi" | "nvim" | "nano" | "emacs" |
        "fzf" | "ranger" | "ncdu" | "lazygit" | "tig" |
        "less" | "more" | "watch" | "nethogs" | "glances" | "yazi"
    )
}

fn detect_system_terminal() -> String {
    let terminals = ["kitty", "alacritty", "foot", "wezterm", "xterm", "xfce4-terminal", "konsole", "gnome-terminal"];
    for t in &terminals {
        if std::process::Command::new("which").arg(t).output()
            .map(|o| o.status.success()).unwrap_or(false)
        {
            return t.to_string();
        }
    }
    "xterm".to_string()
}

pub fn prompt_prefix(cwd: &str) -> String {
    let u = get_username();
    let h = get_hostname();
    let f = get_folder_name(cwd);
    format!("[{}@{}  {}]$ ", u, h, f)
}

pub fn render(
    ctx: &egui::Context,
    t: &AexTheme,
    state: &mut AppState,
    tx: &Sender<BackendMsg>,
) {
    if !state.terminal.open {
        return;
    }

    let c  = &t.colors;
    let sp = &t.spacing;

    let panel_fill = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_mantle().r(),
            c.bg_mantle().g(),
            c.bg_mantle().b(),
            (t.effects.panel_opacity * 255.0) as u8,
        )
    } else {
        c.bg_mantle()
    };

    egui::TopBottomPanel::bottom("bottom_terminal_panel")
        .height_range(100.0..=650.0)
        .default_height(state.terminal.height)
        .resizable(true)
        .frame(egui::Frame {
            fill: panel_fill,
            stroke: Stroke::new(1.0_f32, c.border()),
            inner_margin: Margin::symmetric(sp.sm, sp.xs),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // Track height smoothly without fighting the resize drag
            if !ui.input(|i| i.pointer.primary_down()) {
                state.terminal.height = ui.available_height().clamp(100.0, 650.0);
            }

            // ── Minimal Header ──────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.label(
                    RichText::new("[terminal]")
                        .color(c.accent())
                        .size(t.typography.size_xs)
                        .monospace()
                        .strong()
                );

                if state.terminal.is_running {
                    let fr = (ui.ctx().input(|i| i.time) * 8.0) as usize % 8;
                    ui.label(
                        RichText::new(["⣾","⣽","⣻","⢿","⡿","⣟","⣯","⣷"][fr])
                            .color(c.accent())
                            .size(11.0)
                    );
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("✕").color(c.text_hint()).size(12.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                    ).on_hover_text("Close terminal (exit)").clicked() {
                        state.terminal.open = false;
                    }
                });
            });

            ui.add_space(2.0);
            let r = ui.min_rect();
            ui.painter().hline(r.left()..=r.right(), r.bottom(), Stroke::new(1.0_f32, c.border()));
            ui.add_space(2.0);

            // ── Terminal Stream Area (History + Current Prompt inline) ────────
            let mut scroll_area = egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true);

            let active_prompt = prompt_prefix(&state.terminal.cwd);

            scroll_area.show(ui, |ui| {
                // Tight line height for terminal
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 1.0);

                // Click on background focuses the terminal input
                let bg_rect = ui.max_rect();
                let bg_id = ui.id().with("term_bg_focus");
                let bg_clicked = ui.interact(bg_rect, bg_id, Sense::click()).clicked();

                for line in &state.terminal.history {
                    let color = match line.kind {
                        TerminalMsgKind::Input   => c.accent(),
                        TerminalMsgKind::Output  => c.text_primary(),
                        TerminalMsgKind::Error   => c.danger(),
                        TerminalMsgKind::Info    => c.text_hint(),
                    };
                    ui.label(
                        RichText::new(&line.text)
                            .color(color)
                            .size(t.typography.size_xs + 1.0)
                            .monospace()
                    );
                }

                // Up / Down arrow history navigation
                if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                    if !state.terminal.cmd_history.is_empty() {
                        let next_idx = match state.terminal.history_cursor {
                            None => state.terminal.cmd_history.len() - 1,
                            Some(idx) if idx > 0 => idx - 1,
                            Some(_) => 0,
                        };
                        state.terminal.history_cursor = Some(next_idx);
                        state.terminal.input = state.terminal.cmd_history[next_idx].clone();
                    }
                }
                if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                    if let Some(idx) = state.terminal.history_cursor {
                        if idx + 1 < state.terminal.cmd_history.len() {
                            let next_idx = idx + 1;
                            state.terminal.history_cursor = Some(next_idx);
                            state.terminal.input = state.terminal.cmd_history[next_idx].clone();
                        } else {
                            state.terminal.history_cursor = None;
                            state.terminal.input.clear();
                        }
                    }
                }

                // ── Active Prompt & Borderless Input Line ───────────────────────
                let mut focus_requested = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    ui.label(
                        RichText::new(&active_prompt)
                            .color(c.accent())
                            .size(t.typography.size_xs + 1.0)
                            .monospace()
                            .strong()
                    );

                    let edit_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.terminal.input)
                            .font(FontId::new(t.typography.size_xs + 1.0, FontFamily::Monospace))
                            .text_color(c.text_primary())
                            .frame(false)
                            .desired_width(ui.available_width().max(60.0))
                    );

                    state.terminal.is_focused = edit_resp.has_focus();

                    // Auto-focus terminal on explicit click or when should_focus is requested
                    if state.terminal.should_focus || bg_clicked {
                        edit_resp.request_focus();
                        state.terminal.should_focus = false;
                    }

                    // Keep repainting periodically for blinking cursor
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));

                    // Scroll to bottom when output arrives or user types
                    if state.terminal.scroll_to_bottom {
                        edit_resp.scroll_to_me(Some(Align::BOTTOM));
                        state.terminal.scroll_to_bottom = false;
                    }

                    let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
                    let should_submit = enter_pressed && (edit_resp.has_focus() || edit_resp.lost_focus());
                    if should_submit {
                        let raw = state.terminal.input.trim().to_string();
                        state.terminal.input.clear();
                        state.terminal.history_cursor = None;
                        state.terminal.scroll_to_bottom = true;

                        if !raw.is_empty() {
                            state.terminal.cmd_history.push(raw.clone());
                            let cmd_line = format!("{}{}", active_prompt, raw);

                            if raw == "clear" {
                                state.terminal.history.clear();
                            } else if raw == "exit" {
                                state.terminal.open = false;
                            } else if raw == "cd" || raw.starts_with("cd ") {
                                let target = raw.strip_prefix("cd").unwrap().trim();
                                let new_dir = if target.is_empty() || target == "~" {
                                    dirs::home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "/".to_string())
                                } else if target.starts_with('/') {
                                    target.to_string()
                                } else {
                                    std::path::Path::new(&state.terminal.cwd)
                                        .join(target)
                                        .to_string_lossy()
                                        .to_string()
                                };

                                state.terminal.history.push(TerminalLine {
                                    kind: TerminalMsgKind::Input,
                                    text: cmd_line,
                                });

                                if let Ok(canon) = std::fs::canonicalize(&new_dir) {
                                    let canon_str = canon.to_string_lossy().to_string();
                                    state.terminal.cwd = canon_str.clone();
                                    state.navigate(canon_str);
                                } else {
                                    state.terminal.history.push(TerminalLine {
                                        kind: TerminalMsgKind::Error,
                                        text: format!("bash: cd: {}: No such file or directory", target),
                                    });
                                }
                            } else if is_interactive_command(&raw) {
                                let term = detect_system_terminal();
                                let cwd_c = state.terminal.cwd.clone();
                                let cmd_c = raw.clone();
                                state.terminal.history.push(TerminalLine {
                                    kind: TerminalMsgKind::Input,
                                    text: cmd_line,
                                });
                                state.terminal.history.push(TerminalLine {
                                    kind: TerminalMsgKind::Info,
                                    text: format!("[terminal] '{}' etkileşimli bir uygulama olduğu için {} ile harici terminalde başlatıldı.", raw, term),
                                });
                                std::thread::spawn(move || {
                                    let mut c = std::process::Command::new(&term);
                                    c.current_dir(&cwd_c);
                                    match term.as_str() {
                                        "kitty" => {
                                            c.args(["--title", &cmd_c, "bash", "-c", &format!("{}; echo; read -p 'Devam etmek için Enter tuşuna basın...'", cmd_c)]);
                                        }
                                        "alacritty" => {
                                            c.args(["-e", "bash", "-c", &format!("{}; echo; read -p 'Devam etmek için Enter tuşuna basın...'", cmd_c)]);
                                        }
                                        _ => {
                                            c.args(["-e", "bash", "-c", &format!("{}; echo; read -p 'Devam etmek için Enter tuşuna basın...'", cmd_c)]);
                                        }
                                    }
                                    let _ = c.spawn();
                                });
                            } else {
                                let cwd = state.terminal.cwd.clone();
                                let cmd = raw.clone();
                                let tx2 = tx.clone();
                                let ctx_clone = ctx.clone();
                                state.terminal.is_running = true;
                                state.terminal.history.push(TerminalLine {
                                    kind: TerminalMsgKind::Input,
                                    text: cmd_line,
                                });

                                std::thread::spawn(move || {
                                    let mut child = std::process::Command::new("bash");
                                    child.args(["-c", &cmd])
                                        .current_dir(&cwd)
                                        .env("TERM", "xterm-256color")
                                        .env("COLORTERM", "truecolor")
                                        .env("LINES", "24")
                                        .env("COLUMNS", "80")
                                        .stdin(std::process::Stdio::null());
                                    let out = child.output();

                                    let mut lines = Vec::new();
                                    match out {
                                        Ok(res) => {
                                            let stdout = String::from_utf8_lossy(&res.stdout);
                                            for line in stdout.lines() {
                                                lines.push(TerminalLine {
                                                    kind: TerminalMsgKind::Output,
                                                    text: line.to_string(),
                                                });
                                            }
                                            let stderr = String::from_utf8_lossy(&res.stderr);
                                            for line in stderr.lines() {
                                                lines.push(TerminalLine {
                                                    kind: TerminalMsgKind::Error,
                                                    text: line.to_string(),
                                                });
                                            }
                                            if lines.is_empty() && !res.status.success() {
                                                lines.push(TerminalLine {
                                                    kind: TerminalMsgKind::Error,
                                                    text: format!("Command exited with status {}", res.status),
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            lines.push(TerminalLine {
                                                kind: TerminalMsgKind::Error,
                                                text: format!("bash: {}: command not found ({})", cmd, e),
                                            });
                                        }
                                    }
                                    tx2.send(BackendMsg::TerminalOutput { lines, sync_nav: None }).ok();
                                    ctx_clone.request_repaint();
                                });
                            }
                        }
                        focus_requested = true;
                    }

                    if focus_requested {
                        edit_resp.request_focus();
                    }
                });
            });
        });
}
