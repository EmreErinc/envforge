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

/// Render the help screen.
pub fn render_help(f: &mut Frame) {
    let area = centered_popup(f.area(), 60, 70);
    f.render_widget(Clear, area);

    let text = vec![
        Line::from(Span::styled(
            "EnvForge Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  j/↓     Move down"),
        Line::from("  k/↑     Move up"),
        Line::from("  Space   Toggle active/passive"),
        Line::from("  e       Edit selected value"),
        Line::from("  a       Add new variable"),
        Line::from("  d       Delete (soft)"),
        Line::from("  r       Restore deleted"),
        Line::from("  c       Copy value"),
        Line::from("  C       Copy KEY=VALUE"),
        Line::from("  m       Move to reference file"),
        Line::from("  u       Undo last operation"),
        Line::from("  I       Import from .env file"),
        Line::from("  E       Export to .env file"),
        Line::from("  P       Switch profile"),
        Line::from("  g       Toggle grouping"),
        Line::from("  →/Enter Expand group"),
        Line::from("  ←       Collapse group"),
        Line::from("  v       Toggle value mask"),
        Line::from("  /       Search/filter"),
        Line::from("  S       Save changes"),
        Line::from("  ?       This help"),
        Line::from("  q       Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc or ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(popup, area);
}
