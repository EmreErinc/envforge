use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{AddField, App, ConfirmAction};

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
            Span::raw(key_name),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Value: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.input.value()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: save  Esc: cancel",
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
            Span::raw(app.add_key_input.value()),
            if *field == AddField::Key {
                Span::styled("│", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Value: ", value_style),
            Span::raw(app.add_value_input.value()),
            if *field == AddField::Value {
                Span::styled("│", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab: switch field  Ctrl+T: toggle target  Enter: next/save  Esc: cancel",
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

    let message = match action {
        ConfirmAction::Delete(key) => format!("Delete '{}'?", key),
        ConfirmAction::Move(key) => format!("Move '{}' to reference file?", key),
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
            Span::raw(app.input.value()),
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

/// Render the profile selector popup.
pub fn render_profile_selector(f: &mut Frame, app: &App, selected_idx: usize) {
    let names = app.config.profiles.profile_names();
    let height = (names.len() as u16 + 6).min(20);
    let area = centered_popup(f.area(), 40, height.max(10));
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Select a profile:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    for (i, name) in names.iter().enumerate() {
        let is_active = *name == app.config.profiles.active;
        let is_selected = i == selected_idx;
        let marker = if is_selected { "▸ " } else { "  " };
        let badge = if is_active { " (active)" } else { "" };

        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{}{}{}", marker, name, badge),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter: select  Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Switch Profile ")
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
        help_section("Navigation"),
        help_shortcut("j/\u{2193}", "Move down"),
        help_shortcut("k/\u{2191}", "Move up"),
        help_shortcut("G", "Jump to bottom"),
        help_shortcut("gg", "Jump to top"),
        help_shortcut("/", "Search / filter"),
        help_shortcut("Esc", "Clear search / close dialog"),
        Line::from(""),
        help_section("Actions"),
        help_shortcut("Space", "Toggle active/passive"),
        help_shortcut("e", "Edit selected value"),
        help_shortcut("a", "Add new variable"),
        help_shortcut("d", "Delete (soft-delete)"),
        help_shortcut("r", "Restore deleted variable"),
        help_shortcut("u", "Undo last operation"),
        help_shortcut("c", "Copy value to clipboard"),
        help_shortcut("K", "Copy key name to clipboard"),
        help_shortcut("C", "Copy KEY=VALUE to clipboard"),
        help_shortcut("m", "Move to reference file"),
        Line::from(""),
        help_section("Display"),
        help_shortcut("v", "Toggle value masking"),
        help_shortcut("g", "Toggle grouping"),
        help_shortcut("\u{2192}/Enter", "Expand group"),
        help_shortcut("\u{2190}", "Collapse group"),
        Line::from(""),
        help_section("Profile & Files"),
        help_shortcut("P", "Switch profile"),
        help_shortcut("I", "Import from .env file"),
        help_shortcut("E", "Export to .env file"),
        help_shortcut("S", "Save changes (Ctrl+S also works)"),
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
