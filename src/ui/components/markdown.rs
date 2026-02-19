use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::ui::colors::THEME;

fn detect_code_blocks(content: &str) -> Vec<(String, bool)> {
    let mut segments = Vec::new();
    let mut in_code_block = false;
    let mut current_segment = String::new();

    for line in content.lines() {
        if line.starts_with("```") {
            if in_code_block {
                segments.push((current_segment.clone(), true));
                current_segment.clear();
                in_code_block = false;
            } else {
                if !current_segment.is_empty() {
                    segments.push((current_segment.clone(), false));
                    current_segment.clear();
                }
                in_code_block = true;
            }
        } else {
            if !current_segment.is_empty() {
                current_segment.push('\n');
            }
            current_segment.push_str(line);
        }
    }

    if !current_segment.is_empty() {
        segments.push((current_segment, false));
    }

    segments
}

fn render_code_block_lines(code: &str, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let code_width = width.saturating_sub(4);

    for line in code.lines() {
        let line_width = line.width();
        if line_width <= code_width {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", line),
                Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
            )]));
        } else {
            let chars: Vec<char> = line.chars().collect();
            for chunk in chars.chunks(code_width) {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", chunk.iter().collect::<String>()),
                    Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
                )]));
            }
        }
    }

    lines
}

fn render_inline_code_spans(segment: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_inline_code = false;

    for c in segment.chars() {
        if c == '`' {
            if in_inline_code {
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
                ));
                current.clear();
                in_inline_code = false;
            } else {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default().fg(THEME.fg)));
                    current.clear();
                }
                in_inline_code = true;
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        if in_inline_code {
            spans.push(Span::styled(
                format!("`{}", current),
                Style::default().fg(THEME.fg),
            ));
        } else {
            spans.push(Span::styled(current, Style::default().fg(THEME.fg)));
        }
    }

    spans
}

fn render_text_line(line: &str, width: usize) -> Vec<Line<'static>> {
    let trimmed = line.trim_start();

    if trimmed.is_empty() {
        return vec![Line::from("")];
    }

    let header_line = if let Some(stripped) = trimmed.strip_prefix("### ") {
        Some(Line::from(vec![Span::styled(
            stripped.to_string(),
            Style::default()
                .fg(THEME.purple)
                .add_modifier(Modifier::BOLD),
        )]))
    } else if let Some(stripped) = trimmed.strip_prefix("## ") {
        Some(Line::from(vec![Span::styled(
            stripped.to_string(),
            Style::default()
                .fg(THEME.purple)
                .add_modifier(Modifier::BOLD),
        )]))
    } else if let Some(stripped) = trimmed.strip_prefix("# ") {
        Some(Line::from(vec![Span::styled(
            stripped.to_string(),
            Style::default()
                .fg(THEME.purple)
                .add_modifier(Modifier::BOLD),
        )]))
    } else {
        None
    };

    if let Some(line) = header_line {
        return vec![line];
    }

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return vec![Line::from(vec![
            Span::styled("• ", Style::default().fg(THEME.purple)),
            Span::styled(trimmed[2..].to_string(), Style::default().fg(THEME.fg)),
        ])];
    }

    let spans = render_inline_code_spans(trimmed);
    let total_width: usize = spans.iter().map(|s| s.content.width()).sum();

    if total_width <= width {
        return vec![Line::from(spans)];
    }

    let mut lines = Vec::new();
    let mut current_spans = Vec::new();
    let mut current_width = 0;

    for span in spans {
        let span_width = span.content.width();

        if current_width + span_width > width && !current_spans.is_empty() {
            lines.push(Line::from(current_spans));
            current_spans = Vec::new();
            current_width = 0;
        }

        if span_width > width {
            let chars: Vec<char> = span.content.chars().collect();
            for chunk in chars.chunks(width) {
                let chunk_str: String = chunk.iter().collect();
                let chunk_width = chunk_str.width();

                if current_width + chunk_width > width && !current_spans.is_empty() {
                    lines.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }

                current_spans.push(Span::styled(chunk_str, span.style));
                current_width += chunk_width;
            }
        } else {
            current_spans.push(span);
            current_width += span_width;
        }
    }

    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}

pub fn render_markdown(content: &str, width: usize) -> Vec<Line<'static>> {
    let segments = detect_code_blocks(content);
    let mut lines = Vec::new();

    for (segment, is_code) in segments {
        if is_code {
            lines.extend(render_code_block_lines(&segment, width));
        } else {
            for line in segment.lines() {
                lines.extend(render_text_line(line, width));
            }
        }
    }

    lines
}
