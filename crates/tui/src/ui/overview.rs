//! Overview 화면 — KPI 카드 3개 + 카테고리 분포 (수평 바).

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Loadable};
use crate::dto::FailedTxAnalysis;
use crate::format;

/// Overview 본문을 그린다.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).split(area);
    render_kpis(f, rows[0], app);
    render_distribution(f, rows[1], app);
}

fn render_kpis(f: &mut Frame<'_>, area: Rect, app: &App) {
    let cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

    let block_val = match &app.latest_block {
        Loadable::Loaded(Some(n)) => format::group_thousands(*n),
        Loadable::Loaded(None) => "—".to_string(),
        Loadable::Failed(_) => "error".to_string(),
        _ => "…".to_string(),
    };
    kpi_card(f, cols[0], "Latest block", &block_val, Color::Cyan);

    let (total_val, total_color) = match &app.analysis {
        Loadable::Loaded(v) => (
            format::group_thousands(v.iter().map(|a| a.failure_count).sum()),
            Color::White,
        ),
        Loadable::Failed(_) => ("error".to_string(), Color::Red),
        _ => ("…".to_string(), Color::DarkGray),
    };
    kpi_card(f, cols[1], "Total failed tx", &total_val, total_color);

    let (cat_val, cat_color) = match &app.analysis {
        Loadable::Loaded(v) => match v.iter().max_by_key(|a| a.failure_count) {
            Some(top) => (
                format::error_category_label(&top.error_category).to_string(),
                format::error_category_color(&top.error_category),
            ),
            None => ("none".to_string(), Color::DarkGray),
        },
        Loadable::Failed(_) => ("error".to_string(), Color::Red),
        _ => ("…".to_string(), Color::DarkGray),
    };
    kpi_card(f, cols[2], "Top category", &cat_val, cat_color);
}

fn kpi_card(f: &mut Frame<'_>, area: Rect, title: &str, value: &str, color: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "));
    let para = Paragraph::new(format!("\n{value}"))
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(para, area);
}

fn render_distribution(f: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Failure distribution by category ");
    match &app.analysis {
        Loadable::Loaded(v) if !v.is_empty() => {
            let inner = block.inner(area);
            f.render_widget(block, area);
            let lines = distribution_lines(v, inner.width);
            f.render_widget(Paragraph::new(lines), inner);
        }
        Loadable::Loaded(_) => super::render_block_banner(
            f,
            area,
            block,
            "No failed transactions indexed yet.",
            Color::DarkGray,
        ),
        Loadable::Failed(e) => {
            super::render_block_banner(f, area, block, &format!("Error: {e}"), Color::Red)
        }
        _ => super::render_block_banner(f, area, block, "Loading…", Color::Yellow),
    }
}

/// 카테고리별 수평 바 라인 — 빈도 desc 정렬, 카테고리 색 적용.
fn distribution_lines(analysis: &[FailedTxAnalysis], width: u16) -> Vec<Line<'static>> {
    const LABEL_W: usize = 24;
    const STATS_W: usize = 42;
    let max = analysis
        .iter()
        .map(|a| a.failure_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let bar_w = (width as usize).saturating_sub(LABEL_W + STATS_W).max(1);

    let mut sorted: Vec<&FailedTxAnalysis> = analysis.iter().collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.failure_count));

    sorted
        .iter()
        .map(|a| {
            let cat = format::normalize_category(&a.error_category);
            let color = format::error_category_color(cat);
            let label = format::error_category_label(cat);
            let scaled = (a.failure_count as f64 / max as f64) * bar_w as f64;
            let filled =
                (scaled.round() as usize).clamp(if a.failure_count > 0 { 1 } else { 0 }, bar_w);
            Line::from(vec![
                Span::raw(format!("{label:<LABEL_W$}")),
                Span::styled("█".repeat(filled), Style::default().fg(color)),
                Span::raw(format!(
                    " {} ({}) · {} gas · {}",
                    a.failure_count,
                    format::format_pct_str(&a.pct_of_total),
                    format::format_compact(format::to_number(&a.avg_gas_wasted)),
                    format::time_ago(&a.most_recent_failure),
                )),
            ])
        })
        .collect()
}
