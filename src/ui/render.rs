use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::app::{App, NotificationLevel, ViewMode};
use super::dialogs;
use super::table;

/// Render the complete TUI.
pub fn render(f: &mut Frame, app: &App) {
    let constraints = if app.inspector_open {
        vec![
            Constraint::Length(3), // header
            Constraint::Min(5),    // table
            Constraint::Length(5), // bottom inspector drawer
            Constraint::Length(3), // footer
        ]
    } else {
        vec![
            Constraint::Length(3), // header
            Constraint::Min(5),    // table
            Constraint::Length(3), // footer
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_table(f, app, chunks[1]);
    if app.inspector_open {
        render_bottom_inspector(f, app, chunks[2]);
        render_footer(f, app, chunks[3]);
    } else {
        render_footer(f, app, chunks[2]);
    }

    match &app.mode {
        ViewMode::Editing => dialogs::render_edit_popup(f, app),
        ViewMode::Adding(field) => dialogs::render_add_popup(f, app, field),
        ViewMode::Confirming(action) => dialogs::render_confirm_popup(f, action),
        ViewMode::DiffPreview => dialogs::render_diff_preview(f, app),
        ViewMode::Help => dialogs::render_help(f, app.help_page),
        ViewMode::Importing => {
            dialogs::render_path_input(f, app, "Import from .env", "Enter path to .env file:")
        }
        ViewMode::Exporting => {
            dialogs::render_path_input(f, app, "Export to .env", "Enter output path:")
        }
        ViewMode::ProfileSelector(idx) => dialogs::render_profile_selector(f, app, *idx),
        ViewMode::FirstRun => render_first_run(f),
        ViewMode::CommandPalette => dialogs::render_command_palette(f, app),
        ViewMode::SecretGenerator => dialogs::render_secret_generator(f, app),
        ViewMode::Normal | ViewMode::Searching | ViewMode::MultiSelect => {}
    }
}

fn render_first_run(f: &mut Frame) {
    use ratatui::widgets::{Clear, Wrap};

    let area = f.area();
    let popup_area = ratatui::layout::Rect {
        x: area.width.saturating_sub(70).saturating_div(2).max(2),
        y: area.height.saturating_sub(14).saturating_div(2).max(2),
        width: 66.min(area.width.saturating_sub(4)),
        height: 14.min(area.height.saturating_sub(4)),
    };

    f.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(Span::styled(
            " EnvForge Security Setup ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  EnvForge can protect your secrets from AI tools and"),
        Line::from("  accidental exposure. We recommend enabling the fence"),
        Line::from("  to block AI assistants (Copilot, Cursor, Claude Code)"),
        Line::from("  from reading your environment files."),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [1]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quick protect — create AI ignore rules (recommended)"),
        ]),
        Line::from(vec![
            Span::styled("  [2]", Style::default().fg(Color::Yellow)),
            Span::raw(" Skip for now — remind me later with 'envforge fence'"),
        ]),
        Line::from(vec![
            Span::styled("  [q]", Style::default().fg(Color::DarkGray)),
            Span::raw(" Exit — configure later"),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" First Run "),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, popup_area);
}

fn render_bottom_inspector(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let selected_entry = app.selected_entry();
    let text = if let Some(entry) = selected_entry {
        let details = super::inspector::InspectorDetails::from_entry(&entry, &app.config);
        let status_str = if details.is_active { "Active" } else { "Commented Out" };
        let value_display = if app.is_masked(&details.key) {
            "•••••••••• (Press Space to reveal)".to_string()
        } else {
            super::sanitize::sanitize_for_display(&details.raw_value)
        };

        vec![
            Line::from(vec![
                Span::styled(format!(" Key: {} ", details.key), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" [{}] ", status_str), Style::default().fg(if details.is_active { Color::Green } else { Color::Gray })),
                Span::styled(format!(" Location: {} ", details.location_str), Style::default().fg(Color::DarkGray)),
                Span::styled(format!(" Security: {:.1} bits ({}) ", details.entropy, details.rating), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled(" Value: ", Style::default().fg(Color::Yellow)),
                Span::raw(value_display),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(" No variable selected", Style::default().fg(Color::DarkGray)))]
    };

    let block = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Inspector (Tab / i) ")
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(block, area);
}


fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title = if app.mode == ViewMode::Searching {
        format!(" EnvForge  /{}█", app.search_query)
    } else {
        " EnvForge ".to_string()
    };

    let file_names: Vec<String> = app
        .shell_files
        .iter()
        .map(|sf| {
            sf.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let files_str = if file_names.is_empty() {
        "No files loaded".to_string()
    } else {
        file_names.join(" | ")
    };

    let profile_badge = format!(" [{}] ", app.config.profiles.active);

    let header_text = vec![Line::from(vec![
        Span::styled(
            &title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            &profile_badge,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(files_str, Style::default().fg(Color::DarkGray)),
    ])];

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(header, area);
}

fn render_table(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (table_widget, mut state) = table::build_table(app);
    f.render_stateful_widget(table_widget, area, &mut state);
}

fn render_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let visible_count = app.visible_entries().len();
    let total_count = app.entries.len();

    let mut spans = vec![];

    // Fence status + read-only target summary
    if app.fence_enabled {
        spans.push(Span::styled(
            " [fence:on] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        // Show which targets are active, e.g. "fence: cursor_ignore,copilot (2/5)"
        if !app.fence_resolved_targets.is_empty() {
            let summary = crate::ops::fence::fence_target_summary(&app.fence_resolved_targets);
            spans.push(Span::styled(
                format!(" {summary} "),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    if app.has_unsaved_changes {
        spans.push(Span::styled(
            " [unsaved] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if !app.undo_stack.is_empty() {
        spans.push(Span::styled(
            format!(" {} undoable ", app.undo_stack.len()),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if let Some(notif) = &app.notification {
        let color = match notif.level {
            NotificationLevel::Success => Color::Green,
            NotificationLevel::Warning => Color::Yellow,
            NotificationLevel::Error => Color::Red,
        };
        spans.push(Span::styled(
            format!(" {} ", notif.message),
            Style::default().fg(color),
        ));
        spans.push(Span::raw(" │ "));
    }

    if app.search_query.is_empty() {
        spans.push(Span::styled(
            format!("{} vars", total_count),
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(
            format!("{}/{} vars (filtered)", visible_count, total_count),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let status_line = Line::from(spans);

    let shortcuts = Line::from(vec![
        Span::styled(" [Ctrl+P]", Style::default().fg(Color::Cyan)),
        Span::raw(" Palette "),
        Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
        Span::raw(" Inspect "),
        Span::styled("[G]", Style::default().fg(Color::Cyan)),
        Span::raw(" GenSecret "),
        Span::styled("[v]", Style::default().fg(Color::Cyan)),
        Span::raw(" MultiSelect "),
        Span::styled("[p]", Style::default().fg(Color::Cyan)),
        Span::raw(" Profiles "),
        Span::styled("[I]", Style::default().fg(Color::Cyan)),
        Span::raw(" Import "),
        Span::styled("[E]", Style::default().fg(Color::Cyan)),
        Span::raw(" Export "),
        Span::styled("[a]", Style::default().fg(Color::Cyan)),
        Span::raw("dd "),
        Span::styled("[e]", Style::default().fg(Color::Cyan)),
        Span::raw("dit "),
        Span::styled("[/]", Style::default().fg(Color::Cyan)),
        Span::raw("search "),
        Span::styled("[S]", Style::default().fg(Color::Cyan)),
        Span::raw("ave "),
        Span::styled("[?]", Style::default().fg(Color::Cyan)),
        Span::raw("help "),
        Span::styled("[q]", Style::default().fg(Color::Cyan)),
        Span::raw("uit"),
    ]);

    let footer = Paragraph::new(vec![shortcuts, status_line]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(footer, area);
}
