//! Detail 화면 — 콜트리(root_cause 강조) + 디코딩된 함수 + 진단.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Loadable};
use crate::dto::{DecodedFunction, FailedTxDetail};
use crate::format;

/// Detail 본문을 그린다.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Failure diagnosis ");
    match &app.detail {
        Loadable::Loaded(d) => {
            let inner = block.inner(area);
            f.render_widget(block, area);
            let para = Paragraph::new(detail_lines(d)).scroll((app.detail_scroll, 0));
            f.render_widget(para, inner);
        }
        Loadable::Failed(e) => {
            super::render_block_banner(f, area, block, &format!("Error: {e}"), Color::Red)
        }
        Loadable::Loading => super::render_block_banner(f, area, block, "Loading…", Color::Yellow),
        Loadable::Idle => super::render_block_banner(
            f,
            area,
            block,
            "Select a failed transaction (Enter on the Failed Tx tab).",
            Color::DarkGray,
        ),
    }
}

fn label_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn kv(key: &str, val: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<12} "), label_style()),
        Span::raw(val.to_string()),
    ])
}

/// 상세 진단을 표시용 라인 목록으로 조립한다.
fn detail_lines(d: &FailedTxDetail) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let cat = format::normalize_category(&d.failed.error_category);

    lines.push(kv("tx hash", &d.failed.tx_hash));
    lines.push(Line::from(vec![
        Span::styled(format!("{:<12} ", "category"), label_style()),
        Span::styled(
            format::error_category_label(cat).to_string(),
            Style::default()
                .fg(format::error_category_color(cat))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(kv(
        "revert",
        d.failed.revert_reason.as_deref().unwrap_or("—"),
    ));
    lines.push(kv(
        "function",
        d.failed.failing_function.as_deref().unwrap_or("—"),
    ));
    lines.push(kv("gas used", &format::group_thousands(d.failed.gas_used)));
    lines.push(kv("when", &format::time_ago(&d.failed.timestamp)));
    lines.push(Line::from(""));

    // ── Call tree ──
    let root_id = d.root_cause.as_ref().map(|r| r.trace_id);
    let mut title = format!("Call tree ({} frames)", d.call_tree.len());
    if d.call_tree_truncated {
        title.push_str(" — truncated");
    }
    lines.push(Line::from(Span::styled(title, header_style())));
    for frame in &d.call_tree {
        let indent = "  ".repeat(frame.call_depth.max(0) as usize);
        let to = frame.to_addr.as_deref().unwrap_or("(create)");
        let mut text = format!(
            "{indent}{} {} → {}  gas {}",
            frame.call_type,
            format::truncate_hash(&frame.from_addr),
            format::truncate_hash(to),
            format::group_thousands(frame.gas_used)
        );
        if frame.value != "0" && !frame.value.is_empty() {
            text.push_str(&format!("  value {}", frame.value));
        }
        if let Some(err) = &frame.error {
            text.push_str(&format!("  ⮕ revert: {err}"));
        }
        let style = if Some(frame.trace_id) == root_id {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines.push(Line::from(""));

    // ── Decoded functions ──
    if let Some(df) = &d.failing_function_decoded {
        push_decoded(&mut lines, "Failing function", df);
    }
    if let Some(df) = &d.root_cause_decoded {
        push_decoded(&mut lines, "Root cause function", df);
    }

    // ── Diagnosis ──
    lines.push(Line::from(Span::styled("Diagnosis", header_style())));
    match &d.diagnosis {
        Some(dg) => {
            lines.push(kv("why", &dg.message));
            if let Some(act) = &dg.recommended_action {
                lines.push(kv("fix", act));
            }
            if let Some(src) = &dg.source {
                lines.push(kv("source", src));
            }
        }
        None => lines.push(Line::from(Span::styled(
            "  (no diagnosis seeded for this category)",
            label_style(),
        ))),
    }

    lines
}

/// 디코딩된 함수 한 블록(제목/메타/인자)을 라인에 덧붙인다.
fn push_decoded(lines: &mut Vec<Line<'static>>, title: &str, df: &DecodedFunction) {
    lines.push(Line::from(Span::styled(title.to_string(), header_style())));
    lines.push(kv("selector", &df.selector));
    lines.push(kv("name", &df.name));
    lines.push(kv("signature", &df.signature));
    if let Some(src) = &df.source {
        lines.push(kv("source", src));
    }
    match &df.args {
        Some(args) if !args.is_empty() => {
            lines.push(Line::from(Span::styled("  args:", label_style())));
            for (i, a) in args.iter().enumerate() {
                lines.push(Line::from(format!(
                    "    [{i}] {}: {}",
                    a.ty,
                    value_to_string(&a.value)
                )));
            }
        }
        Some(_) => lines.push(Line::from(Span::styled("  args: (none)", label_style()))),
        None => lines.push(Line::from(Span::styled(
            "  args: (not decoded)",
            label_style(),
        ))),
    }
    lines.push(Line::from(""));
}

/// `serde_json::Value`를 사람이 읽는 한 줄 문자열로 (재귀, 배열 펼침).
fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(value_to_string).collect();
            format!("[{}]", parts.join(", "))
        }
        other => other.to_string(),
    }
}
