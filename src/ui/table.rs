use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

use crate::ops::source_display_name;

use super::app::{App, TableRow};

const MASK: &str = "•••••";

/// Build the ENV table widget with group support.
pub fn build_table(app: &App) -> (Table<'_>, TableState) {
    let rows_data = app.visible_rows();

    let header = Row::new(vec![
        Cell::from("KEY").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("VALUE").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("LOCATION").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .height(1);

    let rows: Vec<Row> = rows_data
        .iter()
        .enumerate()
        .map(|(i, table_row)| match table_row {
            TableRow::GroupHeader {
                name,
                count,
                collapsed,
            } => {
                let indicator = if *collapsed { "▸" } else { "▾" };
                let label = format!("{} {} ({})", indicator, name, count);
                let style = if i == app.selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                Row::new(vec![Cell::from(label), Cell::from(""), Cell::from("")]).style(style)
            }
            TableRow::Entry(entry) => {
                let is_active = entry.location != crate::ops::EntryLocation::Commented;
                let status_icon = if is_active { "■" } else { "□" };

                let value_display = if app.is_masked(i, &entry.key) {
                    MASK.to_string()
                } else {
                    truncate_value(&entry.value, 50)
                };

                let location = source_display_name(&app.config, &entry.source_file);
                let is_duplicate = app.duplicate_keys.contains(&entry.key);

                let style = if i == app.selected {
                    if is_active {
                        Style::default().bg(Color::DarkGray).fg(Color::White)
                    } else {
                        Style::default().bg(Color::DarkGray).fg(Color::Gray)
                    }
                } else if !is_active {
                    Style::default().fg(Color::DarkGray)
                } else if is_duplicate {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };

                // Indent entries within groups
                let indent = if app.grouping_enabled && app.search_query.is_empty() {
                    "  "
                } else {
                    ""
                };

                let status_style = if is_active {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                // Build key spans with fuzzy highlight if searching
                let fuzzy_results = app.fuzzy_results();
                let matched_indices: Option<&Vec<usize>> = fuzzy_results
                    .iter()
                    .find(|m| m.entry.key == entry.key)
                    .map(|m| &m.matched_indices);

                let key_spans = if let Some(indices) = matched_indices {
                    if !indices.is_empty() {
                        build_highlighted_spans(&entry.key, indices, style)
                    } else {
                        vec![Span::styled(
                            entry.key.clone(),
                            style.patch(Style::default()),
                        )]
                    }
                } else {
                    vec![Span::styled(
                        entry.key.clone(),
                        style.patch(Style::default()),
                    )]
                };

                let mut spans = vec![Span::styled(
                    format!("{}{} ", indent, status_icon),
                    status_style,
                )];
                spans.extend(key_spans);
                let key_cell = Cell::from(Line::from(spans));

                Row::new(vec![
                    key_cell,
                    Cell::from(value_display),
                    Cell::from(location),
                ])
                .style(style)
            }
        })
        .collect();

    let widths = [
        Constraint::Percentage(35),
        Constraint::Percentage(50),
        Constraint::Percentage(15),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Environment Variables "),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = TableState::default();
    state.select(Some(app.selected));

    (table, state)
}

fn truncate_value(value: &str, max_len: usize) -> String {
    if value.len() > max_len {
        format!("{}…", &value[..max_len])
    } else {
        value.to_string()
    }
}

/// Build spans with matched characters highlighted.
fn build_highlighted_spans<'a>(text: &str, indices: &[usize], base_style: Style) -> Vec<Span<'a>> {
    let highlight_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    let mut last_end = 0;
    let chars: Vec<char> = text.chars().collect();

    for &idx in indices {
        if idx >= chars.len() {
            continue;
        }
        if idx > last_end {
            let normal: String = chars[last_end..idx].iter().collect();
            spans.push(Span::styled(normal, base_style.patch(Style::default())));
        }
        spans.push(Span::styled(chars[idx].to_string(), highlight_style));
        last_end = idx + 1;
    }

    if last_end < chars.len() {
        let remaining: String = chars[last_end..].iter().collect();
        spans.push(Span::styled(remaining, base_style.patch(Style::default())));
    }

    spans
}
