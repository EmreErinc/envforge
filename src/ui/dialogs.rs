use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{AddField, App, ConfirmAction, HealthSeverity, MatrixCellStatus};

/// Render a centered popup area.
pub fn centered_popup(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(vertical[1])[1]
}

/// Render the edit popup.
pub fn render_edit_popup(f: &mut Frame, app: &App) {
    let area = centered_popup(f.area(), 60, 20);
    f.render_widget(Clear, area);

    let visible = app.visible_entries();
    let key_name = visible
        .get(app.selected)
        .map(|e| e.key.as_str())
        .unwrap_or("?");

    let text = vec![
        Line::from(vec![
            Span::styled("Key: ", Style::default().fg(Color::Yellow)),
            Span::raw(super::sanitize::sanitize_for_display(key_name)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Value: ", Style::default().fg(Color::Yellow)),
            Span::raw(super::sanitize::sanitize_for_display(app.input.value())),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Ctrl+G: generate secret  Enter: save  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Edit Value ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(popup, area);
}

/// Render the add dialog.
pub fn render_add_popup(f: &mut Frame, app: &App, field: &AddField) {
    let area = centered_popup(f.area(), 60, 30);
    f.render_widget(Clear, area);

    let key_style = if *field == AddField::Key {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let value_style = if *field == AddField::Value {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let target_name = match app.add_target {
        super::app::AddTarget::Profile => &app.config.profiles.active,
        super::app::AddTarget::Shared => "shared",
    };
    let target_color = match app.add_target {
        super::app::AddTarget::Profile => Color::Magenta,
        super::app::AddTarget::Shared => Color::Green,
    };

    let text = vec![
        Line::from(vec![
            Span::styled("Target: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[{}]", target_name),
                Style::default()
                    .fg(target_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  (Ctrl+T to toggle)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Key:   ", key_style),
            Span::raw(super::sanitize::sanitize_for_display(
                app.add_key_input.value(),
            )),
            if *field == AddField::Key {
                Span::styled("│", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Value: ", value_style),
            Span::raw(super::sanitize::sanitize_for_display(
                app.add_value_input.value(),
            )),
            if *field == AddField::Value {
                Span::styled("│", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab: switch field  Ctrl+G: generate secret  Enter: next/save  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Add Variable ")
            .border_style(Style::default().fg(Color::Green)),
    );

    f.render_widget(popup, area);
}

/// Render a confirmation dialog.
pub fn render_confirm_popup(f: &mut Frame, action: &ConfirmAction) {
    let area = centered_popup(f.area(), 50, 15);
    f.render_widget(Clear, area);

    // Sanitize the key name — it can carry control/escape bytes when
    // sourced from a parsed/imported file.
    let message = match action {
        ConfirmAction::Delete(key) => format!(
            "Delete '{}'?",
            crate::ui::sanitize::sanitize_for_display(key)
        ),
        ConfirmAction::Move(key) => format!(
            "Move '{}' to reference file?",
            crate::ui::sanitize::sanitize_for_display(key)
        ),
        ConfirmAction::Save => "Save all changes?".to_string(),
        ConfirmAction::Quit => "Quit with unsaved changes?".to_string(),
    };

    let text = vec![
        Line::from(""),
        Line::from(Span::raw(&message)),
        Line::from(""),
        Line::from(Span::styled(
            "[y] Yes  [n] No",
            Style::default().fg(Color::Yellow),
        )),
    ];

    let color = match action {
        ConfirmAction::Delete(_) => Color::Red,
        ConfirmAction::Quit => Color::Red,
        _ => Color::Yellow,
    };

    let popup = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm ")
                .border_style(Style::default().fg(color)),
        );

    f.render_widget(popup, area);
}

/// Render the diff preview.
pub fn render_diff_preview(f: &mut Frame, app: &App) {
    let area = centered_popup(f.area(), 80, 70);
    f.render_widget(Clear, area);

    let lines: Vec<Line> = app
        .diff_content
        .lines()
        .map(|line| {
            // Strip control/escape sequences before the diff hits the
            // terminal (values are already value-redacted upstream).
            let line = crate::ui::sanitize::sanitize_for_display(line);
            if line.starts_with('+') && !line.starts_with("+++") {
                Line::from(Span::styled(line, Style::default().fg(Color::Green)))
            } else if line.starts_with('-') && !line.starts_with("---") {
                Line::from(Span::styled(line, Style::default().fg(Color::Red)))
            } else if line.starts_with("@@") {
                Line::from(Span::styled(line, Style::default().fg(Color::Cyan)))
            } else {
                Line::from(line)
            }
        })
        .collect();

    let popup = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Diff Preview (Esc to close) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(popup, area);
}

/// Render a path input dialog (used for import/export).
pub fn render_path_input(f: &mut Frame, app: &App, title: &str, prompt: &str) {
    let area = centered_popup(f.area(), 60, 20);
    f.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(prompt, Style::default().fg(Color::Yellow))),
        Line::from(""),
        Line::from(vec![
            Span::raw(super::sanitize::sanitize_for_display(app.input.value())),
            Span::styled("│", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: confirm  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", title))
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(popup, area);
}

/// Render the profile selector popup (p).
pub fn render_profile_selector(f: &mut Frame, app: &App, selected_idx: usize) {
    let names = app.config.profiles.profile_names();
    let area = centered_popup(f.area(), 75, 45);
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Select a profile to activate:",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (i, name) in names.iter().enumerate() {
        let is_active = *name == app.config.profiles.active;
        let is_selected = i == selected_idx;
        let num_badge = if i < 9 { format!("  [{}] ", i + 1) } else { "      ".to_string() };
        let marker = if is_selected { "▸ " } else { "  " };

        let file_path = app.config.profiles.entries.get(name)
            .map(|e| e.file.clone())
            .unwrap_or_else(|| format!("~/.env_managed.{}", name));

        let name_style = if is_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(num_badge, Style::default().fg(Color::Yellow)),
            Span::styled(marker, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:<15}", name), name_style),
            Span::styled(format!(" {:<24}", file_path), Style::default().fg(Color::DarkGray)),
        ];

        if is_active {
            spans.push(Span::styled(" [ACTIVE]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
        }

        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " 1-9: Select profile | \u{2191}/\u{2193}: Navigate | Enter: Confirm | Esc: Cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let title = format!(" Switch Profile ({} profiles available) ", names.len());
    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Magenta)),
    );

    f.render_widget(popup, area);
}



/// Build a help shortcut line with styled key and description.
fn help_shortcut(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<10}", key), Style::default().fg(Color::White)),
        Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray)),
    ])
}

/// Build a section header for help pages.
fn help_section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

/// Build the footer line shown on every help page.
fn help_footer() -> Line<'static> {
    Line::from(Span::styled(
        "Tab: next page  |  1/2/3: jump to page  |  Esc: close",
        Style::default().fg(Color::DarkGray),
    ))
}

/// Build page 1: Keyboard Shortcuts.
fn help_page_shortcuts() -> Vec<Line<'static>> {
    vec![
        help_section("Navigation & Palette"),
        help_shortcut("j/\u{2193}", "Move down"),
        help_shortcut("k/\u{2191}", "Move up"),
        help_shortcut("p / P", "Profile Switcher (press 1-9 to pick)"),
        help_shortcut("Ctrl+P / :", "Command Palette (Fuzzy Search)"),
        help_shortcut("Ctrl+1..9 / Opt+1..9", "Quick profile jump"),
        help_shortcut("/", "Search / filter"),
        help_shortcut("Esc", "Clear search / close dialog"),
        Line::from(""),
        help_section("Actions"),
        help_shortcut("a", "Add new variable"),
        help_shortcut("e", "Edit selected value"),
        help_shortcut("d", "Delete (soft-delete)"),
        help_shortcut("u", "Undo last operation"),
        help_shortcut("c", "Copy value to clipboard"),
        help_shortcut("K", "Copy key name to clipboard"),
        help_shortcut("C", "Copy KEY=VALUE to clipboard"),
        help_shortcut("m", "Move to reference file"),
        help_shortcut("G / Ctrl+G", "Secret Generator (Password/Token)"),
        help_shortcut("v", "Toggle multi-select mode"),
        Line::from(""),
        help_section("Display & Files"),
        help_shortcut("Tab / i", "Toggle Bottom Inspector Drawer"),
        help_shortcut("Space", "Toggle secret value masking"),
        help_shortcut("I", "Import from .env file"),
        help_shortcut("E", "Export to .env file"),
        help_shortcut("S / Ctrl+S", "Save changes"),
        Line::from(""),
        help_shortcut("?", "This help"),
        help_shortcut("q", "Quit"),
        Line::from(""),
        help_footer(),
    ]
}

/// Build page 2: CLI Quick Reference.
fn help_page_cli() -> Vec<Line<'static>> {
    vec![
        help_section("CLI Commands (run from terminal)"),
        Line::from(""),
        help_section("Variable Management"),
        help_shortcut("list", "List all variables"),
        help_shortcut("get KEY", "Get a value"),
        help_shortcut("set K=V", "Set or create"),
        help_shortcut("delete KEY", "Soft-delete"),
        help_shortcut("explain K", "Show all info about a key"),
        Line::from(""),
        help_section("AI Safety"),
        help_shortcut("fence", "Create AI ignore rules"),
        help_shortcut("scan --mcp", "Scan AI tool configs"),
        help_shortcut("mcp harden", "Fix AI tool configs"),
        help_shortcut("run --vol.", "Secrets in memory only"),
        help_shortcut("proxy", "Credential proxy"),
        Line::from(""),
        help_section("Secret Managers"),
        help_shortcut("secrets pull", "Pull from provider"),
        help_shortcut("secrets push", "Push to provider"),
        help_shortcut("rotate KEY", "Rotate a secret"),
        Line::from(""),
        help_section("Sync & Backup"),
        help_shortcut("sync p/p", "Cross-machine sync"),
        help_shortcut("snapshot", "Backup env state"),
        help_shortcut("check", "Run all health checks"),
        Line::from(""),
        Line::from(Span::styled(
            "  All commands: envforge --help",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        help_footer(),
    ]
}

/// Build page 3: About.
fn help_page_about() -> Vec<Line<'static>> {
    let version = env!("CARGO_PKG_VERSION");
    vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("EnvForge v{}", version),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "The AI-safe environment variable manager.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Written in Rust.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "MIT License.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "https://github.com/emreerinc/envforge",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(""),
        help_footer(),
    ]
}

/// Render the multi-page help screen.
pub fn render_help(f: &mut Frame, page: usize) {
    let area = centered_popup(f.area(), 65, 80);
    f.render_widget(Clear, area);

    let text = match page {
        0 => help_page_shortcuts(),
        1 => help_page_cli(),
        2 => help_page_about(),
        _ => help_page_shortcuts(),
    };

    let title = match page {
        0 => " Help [1/3] Shortcuts ",
        1 => " Help [2/3] CLI Reference ",
        2 => " Help [3/3] About ",
        _ => " Help ",
    };

    let popup = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(popup, area);
}

/// Render the Command Palette popup (Ctrl+P / :).
pub fn render_command_palette(f: &mut Frame, app: &App) {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    let area = centered_popup(f.area(), 65, 55);
    f.render_widget(Clear, area);

    let profiles = app.config.profiles.profile_names();
    let all_items = super::palette::build_palette_items(&profiles, &app.config.profiles.active);

    let matcher = SkimMatcherV2::default();
    let filtered_items: Vec<_> = if app.palette_query.is_empty() {
        all_items
    } else {
        let mut scored: Vec<_> = all_items
            .into_iter()
            .filter_map(|item| {
                matcher
                    .fuzzy_match(&item.label, &app.palette_query)
                    .map(|score| (score, item))
            })
            .collect();
        scored.sort_by_key(|b| std::cmp::Reverse(b.0));
        scored.into_iter().map(|(_, item)| item).collect()
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(&app.palette_query),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(Span::styled("───────────────────────────────────────────────────", Style::default().fg(Color::DarkGray))),
    ];

    if filtered_items.is_empty() {
        lines.push(Line::from(Span::styled("  No matching commands", Style::default().fg(Color::DarkGray))));
    } else {
        for (i, item) in filtered_items.iter().take(8).enumerate() {
            let is_selected = i == app.palette_selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let category_badge = match item.category {
                super::palette::PaletteCategory::Profile => " [profile]",
                super::palette::PaletteCategory::System => " [action]",
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}", prefix, item.label), style),
                Span::styled(category_badge, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Enter: run command  Esc: cancel", Style::default().fg(Color::DarkGray))));

    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Command Palette (Ctrl+P / :) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(popup, area);
}

/// Render the Secret Generator popup (G).
pub fn render_secret_generator(f: &mut Frame, app: &App) {
    let area = centered_popup(f.area(), 60, 50);
    f.render_widget(Clear, area);

    let format_str = match app.secret_gen_opts.format {
        super::secret_gen::SecretGenFormat::AlphaNumericSpecial => "AlphaNumeric + Symbols",
        super::secret_gen::SecretGenFormat::AlphaNumericOnly => "AlphaNumeric Only",
        super::secret_gen::SecretGenFormat::Hex => "Hex",
        super::secret_gen::SecretGenFormat::Base64 => "Base64",
        super::secret_gen::SecretGenFormat::UuidV4 => "UUID v4",
    };

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Format (Left/Right): ", Style::default().fg(Color::Yellow)),
            Span::styled(format_str, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Length (Up/Down):    ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}", app.secret_gen_opts.length), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Preview: ", Style::default().fg(Color::Yellow))),
        Line::from(Span::styled(format!("    {}", app.generated_secret), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled(
            "  c: copy to clipboard  r: regenerate  Enter: apply secret  Esc: back",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Secret Generator (G) ")
            .border_style(Style::default().fg(Color::Green)),
    );

    f.render_widget(popup, area);
}

/// Render the health audit dialog (H).
pub fn render_health_audit_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_popup(area, 75, 60);
    f.render_widget(Clear, popup_area);

    let report = &app.health_report;

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " Audit Summary: ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} Errors, {} Warnings", report.error_count, report.warning_count),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    if report.issues.is_empty() {
        lines.push(Line::from(Span::styled(
            "  ✔ No health or schema issues detected.",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )));
    } else {
        for (i, issue) in report.issues.iter().enumerate() {
            let is_selected = i == report.selected_index;
            let marker = if is_selected { "▸ " } else { "  " };

            let (badge_str, badge_color) = match issue.severity {
                HealthSeverity::Error => ("[ERROR]", Color::Red),
                HealthSeverity::Warning => ("[WARNING]", Color::Yellow),
                HealthSeverity::Info => ("[INFO]", Color::Cyan),
            };

            let line_style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let sanitized_key = super::sanitize::sanitize_for_display(&issue.key);
            let sanitized_msg = super::sanitize::sanitize_for_display(&issue.message);

            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<10} ", badge_str),
                    Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<20} ", sanitized_key), line_style),
                Span::styled(sanitized_msg, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[↑/↓] Navigate  [Enter] Jump to Variable  [Esc] Close",
        Style::default().fg(Color::DarkGray),
    )));

    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Environment & Schema Health Audit (H) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(popup, popup_area);
}

fn format_matrix_cell_value(key: &str, status: &MatrixCellStatus, app: &App) -> (String, Color) {
    match status {
        MatrixCellStatus::Missing => ("[MISSING]".to_string(), Color::Red),
        MatrixCellStatus::Overridden(val) => {
            let display_val = if app.is_masked(key) {
                "•••••".to_string()
            } else {
                let sanitized = super::sanitize::sanitize_for_display(val);
                if sanitized.chars().count() > 12 {
                    format!("{}...", sanitized.chars().take(10).collect::<String>())
                } else {
                    sanitized
                }
            };
            if display_val.is_empty() {
                ("[OVERRIDDEN]".to_string(), Color::Yellow)
            } else {
                (format!("[OVERRIDDEN] {}", display_val), Color::Yellow)
            }
        }
        MatrixCellStatus::Set(val) => {
            let display_val = if app.is_masked(key) {
                "•••••".to_string()
            } else {
                let sanitized = super::sanitize::sanitize_for_display(val);
                if sanitized.chars().count() > 12 {
                    format!("{}...", sanitized.chars().take(10).collect::<String>())
                } else {
                    sanitized
                }
            };
            if display_val.is_empty() {
                ("[SET]".to_string(), Color::Green)
            } else {
                (format!("[SET] {}", display_val), Color::Green)
            }
        }
    }
}

/// Render the multi-environment profile matrix dialog (M).
pub fn render_profile_matrix_dialog(f: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::{Cell, Row, Table, TableState};

    let popup_area = centered_popup(area, 85, 75);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Profile Matrix & Multi-Environment Grid (M) ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Table
            Constraint::Length(1), // Footer help text
        ])
        .split(inner_area);

    // Build Header
    let mut header_cells = vec![
        Cell::from("KEY").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("SHARED").style(
            if app.matrix_data.selected_col == 0 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            },
        ),
    ];

    for (i, p_name) in app.matrix_data.profiles.iter().enumerate() {
        let is_selected_col = app.matrix_data.selected_col == i + 1;
        let cell_style = if is_selected_col {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        };
        header_cells.push(Cell::from(p_name.to_uppercase()).style(cell_style));
    }

    let header_row = Row::new(header_cells).height(1);

    // Build Constraints
    let mut constraints = vec![Constraint::Length(22), Constraint::Length(18)];
    for _ in &app.matrix_data.profiles {
        constraints.push(Constraint::Length(18));
    }

    // Build Rows
    let mut table_rows = Vec::new();
    for (r_idx, row_data) in app.matrix_data.rows.iter().enumerate() {
        let is_row_selected = r_idx == app.matrix_data.selected_row;

        let key_display = super::sanitize::sanitize_for_display(&row_data.key);
        let (key_str, key_style) = if is_row_selected {
            (
                format!("▸ {}", key_display),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                format!("  {}", key_display),
                Style::default().fg(Color::White),
            )
        };

        let mut row_cells = vec![Cell::from(key_str).style(key_style)];

        // Shared column (col 0)
        let is_shared_selected = is_row_selected && app.matrix_data.selected_col == 0;
        let (shared_text, shared_color) = format_matrix_cell_value(&row_data.key, &row_data.shared_status, app);
        let shared_style = if is_shared_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(shared_color)
        };
        row_cells.push(Cell::from(shared_text).style(shared_style));

        // Profile columns (col 1..=profiles.len())
        for (c_idx, (_, status)) in row_data.profile_statuses.iter().enumerate() {
            let is_cell_selected = is_row_selected && app.matrix_data.selected_col == c_idx + 1;
            let (cell_text, cell_color) = format_matrix_cell_value(&row_data.key, status, app);
            let cell_style = if is_cell_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(cell_color)
            };
            row_cells.push(Cell::from(cell_text).style(cell_style));
        }

        table_rows.push(Row::new(row_cells));
    }

    let table = Table::new(table_rows, constraints)
        .header(header_row)
        .column_spacing(1);

    let mut state = TableState::default();
    if !app.matrix_data.rows.is_empty() {
        state.select(Some(app.matrix_data.selected_row));
    }

    f.render_stateful_widget(table, chunks[0], &mut state);

    // Footer Help Text
    let footer_text = Line::from(vec![
        Span::styled("[↑/↓/←/→]", Style::default().fg(Color::Cyan)),
        Span::raw(" Navigate Grid  "),
        Span::styled("[c]", Style::default().fg(Color::Cyan)),
        Span::raw(" Scaffold Missing Key  "),
        Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
        Span::raw(" Close"),
    ]);

    let footer = Paragraph::new(footer_text).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(footer, chunks[1]);
}




