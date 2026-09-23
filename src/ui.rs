use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap,
    },
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, Focus},
    db::PAGE_SIZE,
};

const BG: Color = Color::Rgb(16, 21, 31);
const PANEL: Color = Color::Rgb(21, 28, 40);
const FG: Color = Color::Rgb(216, 225, 238);
const MUTED: Color = Color::Rgb(126, 145, 168);
const BORDER: Color = Color::Rgb(48, 63, 83);
const ACCENT: Color = Color::Rgb(99, 217, 197);
const GOLD: Color = Color::Rgb(233, 190, 116);
const RED: Color = Color::Rgb(245, 132, 139);
const SELECTED: Color = Color::Rgb(34, 56, 68);

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG).fg(FG)), area);
    if area.width < 60 || area.height < 18 {
        frame.render_widget(
            Paragraph::new("SQLite TUI\nResize the terminal to at least 60 × 18.\nCtrl+C to quit.")
                .style(Style::default().fg(ACCENT))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let [header, body, status, hints] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let mode = if app.read_only {
        " READ ONLY "
    } else {
        " READ / WRITE "
    };
    let transaction = if app.db.in_transaction() {
        "  ● TRANSACTION OPEN"
    } else {
        ""
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ◈ SQLITE ", Style::default().fg(ACCENT).bold()),
            Span::styled(" / ", Style::default().fg(BORDER)),
            Span::styled(safe(&app.label), Style::default().fg(FG)),
            Span::styled(format!("  {mode}"), Style::default().fg(GOLD)),
            Span::styled(transaction, Style::default().fg(GOLD).bold()),
        ])),
        header,
    );

    let sidebar_width = (area.width / 4).clamp(18, 30);
    let [sidebar, workspace] =
        Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(1)]).areas(body);
    draw_objects(frame, app, sidebar);
    let editor_height = (body.height / 3).clamp(5, 10);
    let [data, editor] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(editor_height)]).areas(workspace);
    draw_data(frame, app, data);
    draw_editor(frame, app, editor);

    frame.render_widget(
        Paragraph::new(format!(" {}", safe(&app.status)))
            .style(Style::default().fg(if app.error { RED } else { ACCENT })),
        status,
    );
    let hint = if app.searching {
        " Type to filter · Enter open · Esc clear"
    } else if app.focus == Focus::Editor {
        " F5 / ^R run   Enter newline   ^U clear   ^P/^N history   Tab switch   Esc tables"
    } else {
        " Tab switch   i SQL   / filter   s schema   [ ] page   Enter inspect   ? help   q quit"
    };
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(MUTED)),
        hints,
    );
    if app.popup.is_some() {
        draw_popup(frame, app, area);
    }
}

fn panel(title: String, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { ACCENT } else { BORDER }))
        .title(Span::styled(
            title,
            Style::default()
                .fg(if focused { ACCENT } else { MUTED })
                .bold(),
        ))
        .style(Style::default().bg(PANEL).fg(FG))
}

fn draw_objects(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = panel(
        format!(" Objects · {} ", app.filtered.len()),
        app.focus == Focus::Objects,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [search, list_area, note] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(inner);
    let text = if app.filter.is_empty() && !app.searching {
        " / filter objects".into()
    } else {
        format!(" / {}{}", app.filter, if app.searching { "▏" } else { "" })
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(if app.searching { ACCENT } else { MUTED })),
        search,
    );
    if app.filtered.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.objects.is_empty() {
                " No tables yet.\n Press i to write SQL."
            } else {
                " No matching objects."
            })
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: false }),
            list_area,
        );
    } else {
        let items = app
            .filtered
            .iter()
            .map(|i| {
                let object = &app.objects[*i];
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if object.kind == "view" {
                            " ◇ "
                        } else {
                            " ▦ "
                        },
                        Style::default().fg(if object.kind == "view" { GOLD } else { ACCENT }),
                    ),
                    Span::raw(safe(&object.name)),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(Style::default().bg(SELECTED).fg(ACCENT).bold())
                .highlight_symbol("▎"),
            list_area,
            &mut app.list,
        );
    }
    frame.render_widget(
        Paragraph::new(" ▦ table   ◇ view\n Enter open · s schema")
            .style(Style::default().fg(MUTED)),
        note,
    );
}

fn draw_data(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = match &app.opened {
        Some(name) => format!(" {} · page {} ", safe(name), app.page + 1),
        None => " Query results ".into(),
    };
    let block = panel(title, app.focus == Focus::Data);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [grid, detail] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    if app.result.columns.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.result.affected.is_some() {
                "\n  Statement completed.\n  Open a table or run a SELECT to see rows."
            } else {
                "\n  Your data, a few keystrokes away.\n  Open a table or press i to write a query."
            })
            .style(Style::default().fg(MUTED)),
            grid,
        );
        return;
    }
    let widths: Vec<u16> = app
        .result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let width = app
                .result
                .rows
                .iter()
                .filter_map(|row| row.get(i))
                .map(|v| safe(v).width())
                .max()
                .unwrap_or(0)
                .max(name.width());
            width.clamp(8, 32) as u16
        })
        .collect();
    app.first_column = app.first_column.min(app.column);
    let available = grid.width.saturating_sub(7);
    let mut visible = visible_columns(&widths, app.first_column, available);
    if app.column >= app.first_column + visible.len() {
        app.first_column = app.column;
        visible = visible_columns(&widths, app.first_column, available);
    }
    let mut constraints = vec![Constraint::Length(5)];
    constraints.extend(visible.iter().map(|(_, width)| Constraint::Length(*width)));
    let header = Row::new(
        std::iter::once(Cell::from("  #")).chain(
            visible
                .iter()
                .map(|(i, _)| Cell::from(safe(&app.result.columns[*i]))),
        ),
    )
    .style(Style::default().fg(GOLD).bold())
    .bottom_margin(1);
    let rows = app.result.rows.iter().enumerate().map(|(r, row)| {
        let row_number = r
            + 1
            + if app.opened.is_some() {
                app.page * PAGE_SIZE
            } else {
                0
            };
        let cells =
            std::iter::once(Cell::from(row_number.to_string()).style(Style::default().fg(MUTED)))
                .chain(visible.iter().map(|(c, width)| {
                    let mut style = Style::default();
                    if row[*c] == "NULL" {
                        style = style.fg(MUTED).add_modifier(Modifier::ITALIC);
                    }
                    Cell::from(truncate(&safe(&row[*c]), usize::from(*width))).style(style)
                }));
        Row::new(cells).style(Style::default().bg(if r % 2 == 0 { PANEL } else { BG }))
    });
    let table = Table::new(rows, constraints)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(SELECTED))
        .cell_highlight_style(Style::default().bg(ACCENT).fg(BG).bold())
        .highlight_symbol("▎");
    app.table
        .select_column(Some(app.column - app.first_column + 1));
    frame.render_stateful_widget(table, grid, &mut app.table);
    if app.result.rows.is_empty() {
        let empty = Rect {
            y: grid.y.saturating_add(2),
            height: grid.height.saturating_sub(2),
            ..grid
        };
        frame.render_widget(
            Paragraph::new("  No rows returned").style(Style::default().fg(MUTED)),
            empty,
        );
    }
    let value = app
        .table
        .selected()
        .and_then(|i| app.result.rows.get(i))
        .and_then(|row| row.get(app.column))
        .map(|s| safe(s))
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!(
            " {} / {} columns · {}: {}",
            app.column + 1,
            app.result.columns.len(),
            safe(&app.result.columns[app.column]),
            value
        ))
        .style(Style::default().fg(MUTED)),
        detail,
    );
}

fn visible_columns(widths: &[u16], start: usize, available: u16) -> Vec<(usize, u16)> {
    let mut remaining = available;
    let mut columns = vec![];
    for (i, width) in widths.iter().enumerate().skip(start) {
        if remaining == 0 || (!columns.is_empty() && *width > remaining) {
            break;
        }
        let width = (*width).min(remaining);
        columns.push((i, width));
        remaining = remaining.saturating_sub(width + 1);
    }
    columns
}

fn draw_editor(frame: &mut Frame, app: &App, area: Rect) {
    let active = app.focus == Focus::Editor && app.popup.is_none();
    let block = panel(" SQL editor · F5 to run ".into(), active);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let (row, _) = app.editor.position();
    let before = app.editor.text[..app.editor.cursor]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .replace('\t', "    ");
    let cursor_x = before.width();
    let scroll_y = row.saturating_sub(usize::from(inner.height) - 1);
    let gutter = 5u16.min(inner.width.saturating_sub(1));
    let text_width = inner.width.saturating_sub(gutter);
    let scroll_x = cursor_x.saturating_sub(usize::from(text_width).saturating_sub(1));
    let lines: Vec<Line> = app
        .editor
        .text
        .split('\n')
        .map(|line| highlight_sql(&line.replace('\t', "    ")))
        .collect();
    let text_area = Rect {
        x: inner.x + gutter,
        width: text_width,
        ..inner
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines)).scroll((
            scroll_y.min(u16::MAX as usize) as u16,
            scroll_x.min(u16::MAX as usize) as u16,
        )),
        text_area,
    );
    let numbers: Vec<Line> = (scroll_y..scroll_y + usize::from(inner.height))
        .map(|i| Line::from(format!(" {:>2} ", i + 1)))
        .collect();
    frame.render_widget(
        Paragraph::new(numbers).style(Style::default().fg(MUTED)),
        Rect {
            width: gutter,
            ..inner
        },
    );
    if app.editor.text.is_empty() && !active {
        frame.render_widget(
            Paragraph::new("Press i to write SQL…").style(Style::default().fg(MUTED)),
            text_area,
        );
    }
    if active && !app.searching {
        frame.set_cursor_position((
            text_area.x + (cursor_x - scroll_x) as u16,
            inner.y + (row - scroll_y) as u16,
        ));
    }
}

fn highlight_sql(line: &str) -> Line<'static> {
    let keywords = [
        "SELECT", "FROM", "WHERE", "ORDER", "BY", "GROUP", "LIMIT", "OFFSET", "AS", "JOIN", "LEFT",
        "ON", "AND", "OR", "NOT", "NULL", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
        "CREATE", "TABLE", "DROP", "ALTER", "DISTINCT", "DESC", "ASC", "BEGIN", "COMMIT",
        "ROLLBACK", "WITH", "PRAGMA",
    ];
    Line::from(
        line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|token| {
                let word = token.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                Span::styled(
                    token.to_owned(),
                    Style::default().fg(
                        if keywords.contains(&word.to_ascii_uppercase().as_str()) {
                            ACCENT
                        } else {
                            FG
                        },
                    ),
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn draw_popup(frame: &mut Frame, app: &mut App, area: Rect) {
    let popup = app.popup.as_mut().unwrap();
    let width = area.width.saturating_sub(8).min(90);
    let height = area.height.saturating_sub(4).min(32);
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let block = panel(popup.title.clone(), true).title_bottom(" ↑ ↓ scroll · Esc close ");
    let inner = block.inner(rect);
    let text: String = popup
        .text
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '�'
            } else {
                c
            }
        })
        .collect();
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let max_scroll = paragraph
        .line_count(inner.width)
        .saturating_sub(usize::from(inner.height));
    popup.scroll = popup.scroll.min(max_scroll.min(u16::MAX as usize) as u16);
    frame.render_widget(Clear, rect);
    frame.render_widget(paragraph.block(block).scroll((popup.scroll, 0)), rect);
}

fn safe(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' | '\r' => '↵',
            '\t' => '→',
            c if c.is_control() => '�',
            c => c,
        })
        .collect()
}

fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.into();
    }
    let mut used = 0;
    let mut result = String::new();
    for c in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w >= width {
            break;
        }
        result.push(c);
        used += w;
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use ratatui::{Terminal, backend::TestBackend};
    use std::time::Duration;
    #[test]
    fn renders_at_multiple_terminal_sizes_and_with_popups() {
        let mut app = App::new(
            Database::demo(Duration::from_secs(2)).unwrap(),
            "demo.db".into(),
            false,
        )
        .unwrap();
        for (width, height) in [(120, 36), (80, 24), (60, 18), (20, 5), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|f| draw(f, &mut app)).unwrap();
            app.key(crossterm::event::KeyCode::Char('?').into());
            terminal.draw(|f| draw(f, &mut app)).unwrap();
            app.key(crossterm::event::KeyCode::Esc.into());
        }
    }
    #[test]
    fn truncates_using_terminal_width() {
        assert_eq!(truncate("你好世界", 5), "你好…");
        assert_eq!(safe("hello\n\u{1b}"), "hello↵�");
    }
}
