//! Failed-TX 화면 — 필터 라인 + 테이블 + 페이지네이션 푸터.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, Loadable};
use crate::format;

/// Failed-TX 본문을 그린다.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    render_filter(f, rows[0], app);
    render_table(f, rows[1], app);
    render_footer(f, rows[2], app);
}

fn render_filter(f: &mut Frame<'_>, area: Rect, app: &App) {
    let cat = match app.filter.category {
        Some(c) => format::error_category_label(c),
        None => "ALL",
    };
    let line = Line::from(vec![
        Span::styled(" Category: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("[{cat}]"), Style::default().fg(Color::Cyan)),
        Span::styled("  Window: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[{}]", app.filter.window.label()),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            "   [c] category  [t] window",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_table(f: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Failed transactions ");
    match &app.failed {
        Loadable::Loaded(p) if !p.data.is_empty() => {
            let header = Row::new(vec!["tx hash", "category", "gas", "revert reason", "age"])
                .style(
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::BOLD),
                );
            let rows: Vec<Row> = p
                .data
                .iter()
                .map(|t| {
                    let cat = format::normalize_category(&t.error_category);
                    Row::new(vec![
                        Cell::from(format::truncate_hash(&t.tx_hash)),
                        Cell::from(Span::styled(
                            format::error_category_label(cat).to_string(),
                            Style::default().fg(format::error_category_color(cat)),
                        )),
                        Cell::from(format::group_thousands(t.gas_used)),
                        Cell::from(t.revert_reason.clone().unwrap_or_else(|| "—".to_string())),
                        Cell::from(format::time_ago(&t.timestamp)),
                    ])
                })
                .collect();
            let widths = [
                Constraint::Length(14),
                Constraint::Length(22),
                Constraint::Length(10),
                Constraint::Min(16),
                Constraint::Length(10),
            ];
            let table = Table::new(rows, widths)
                .header(header)
                .block(block)
                .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("▶ ");
            let mut state = app.table_state.clone();
            f.render_stateful_widget(table, area, &mut state);
        }
        Loadable::Loaded(_) => super::render_block_banner(
            f,
            area,
            block,
            "No failed transactions match the current filter.",
            Color::DarkGray,
        ),
        Loadable::Failed(e) => {
            super::render_block_banner(f, area, block, &format!("Error: {e}"), Color::Red)
        }
        _ => super::render_block_banner(f, area, block, "Loading…", Color::Yellow),
    }
}

fn render_footer(f: &mut Frame<'_>, area: Rect, app: &App) {
    let text = match &app.failed {
        Loadable::Loaded(p) => {
            let pg = &p.pagination;
            let limit = pg.limit.max(1);
            let from = if pg.total == 0 { 0 } else { pg.offset + 1 };
            let to = pg.offset + pg.count;
            let page = pg.offset / limit + 1;
            let pages = (((pg.total as f64) / (limit as f64)).ceil() as i64).max(1);
            format!(
                " page {page}/{pages} · showing {from}–{to} of {} · [n] next  [b] prev",
                pg.total
            )
        }
        _ => " —".to_string(),
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
