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
    let mut first_segment = true;

    for line in content.lines() {
        if line.starts_with("```") && line.trim() == "```" {
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
                first_segment = true;
            }
        } else {
            if in_code_block {
                if !first_segment {
                    current_segment.push('\n');
                } else {
                    first_segment = false;
                }
                current_segment.push_str(line);
            } else {
                if !current_segment.is_empty() {
                    current_segment.push('\n');
                }
                current_segment.push_str(line);
            }
        }
    }

    if !current_segment.is_empty() {
        segments.push((current_segment, in_code_block));
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
                    spans.extend(render_inline_bold_spans(&current));
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
                current.clone(),
                Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
            ));
        } else {
            spans.extend(render_inline_bold_spans(&current));
        }
    }

    spans
}

fn render_inline_bold_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_bold = false;

    for c in text.chars() {
        if c == '*' {
            if in_bold {
                if !current.is_empty() {
                    spans.push(Span::styled(
                        current.clone(),
                        Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
                    ));
                    current.clear();
                }
                in_bold = false;
            } else {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default().fg(THEME.fg)));
                    current.clear();
                }
                in_bold = true;
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        if in_bold {
            spans.push(Span::styled(
                current.clone(),
                Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(current.clone(), Style::default().fg(THEME.fg)));
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
            stripped.trim_end().to_string(),
            Style::default()
                .fg(THEME.purple)
                .add_modifier(Modifier::BOLD),
        )]))
    } else if let Some(stripped) = trimmed.strip_prefix("## ") {
        Some(Line::from(vec![Span::styled(
            stripped.trim_end().to_string(),
            Style::default()
                .fg(THEME.purple)
                .add_modifier(Modifier::BOLD),
        )]))
    } else if let Some(stripped) = trimmed.strip_prefix("# ") {
        Some(Line::from(vec![Span::styled(
            stripped.trim_end().to_string(),
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

    if trimmed.starts_with("- ") {
        let content = &trimmed[2..];
        let spans = render_inline_code_spans(content);
        let mut final_spans = vec![Span::styled("• ", Style::default().fg(THEME.purple))];
        final_spans.extend(spans);
        return wrap_lines(final_spans, width);
    }

    if trimmed.starts_with("* ") {
        let content = &trimmed[2..];
        let spans = render_inline_code_spans(content);
        let mut final_spans = vec![Span::styled("• ", Style::default().fg(THEME.purple))];
        final_spans.extend(spans);
        return wrap_lines(final_spans, width);
    }

    let spans = render_inline_code_spans(trimmed);
    wrap_lines(spans, width)
}

fn wrap_lines(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
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

fn is_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ")
}

fn is_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("- ") || trimmed.starts_with("* ")
}

pub fn render_markdown(content: &str, width: usize) -> Vec<Line<'static>> {
    let segments = detect_code_blocks(content);
    let mut lines = Vec::new();

    for (segment, is_code) in segments {
        if is_code {
            lines.extend(render_code_block_lines(&segment, width));
        } else {
            let input_lines: Vec<&str> = segment.lines().collect();
            let mut i = 0;

            while i < input_lines.len() {
                let line = input_lines[i];

                if line.trim().is_empty() {
                    lines.push(Line::from(""));
                    i += 1;
                    continue;
                }

                if is_header(line) || is_list_item(line) {
                    lines.extend(render_text_line(line, width));
                    i += 1;
                    continue;
                }

                let mut paragraph = String::new();
                while i < input_lines.len() {
                    let current_line = input_lines[i].trim();
                    if current_line.is_empty()
                        || is_header(input_lines[i])
                        || is_list_item(input_lines[i])
                    {
                        break;
                    }
                    if !paragraph.is_empty() {
                        paragraph.push(' ');
                    }
                    paragraph.push_str(current_line);
                    i += 1;
                }

                if !paragraph.is_empty() {
                    lines.extend(render_text_line(&paragraph, width));
                }
            }
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_code() {
        let result = render_markdown("This is `inline code` test", 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_unclosed_inline_code() {
        let result = render_markdown("This is `unclosed code", 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_code_blocks() {
        let result = render_markdown("```rust\nfn main() {}\n```", 100);
        assert!(result.len() >= 1);
    }

    #[test]
    fn test_headers() {
        let result = render_markdown("# Header 1", 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_list() {
        let result = render_markdown("- Item 1", 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_bold_text() {
        let result = render_markdown("This is *bold* text", 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_code_block_with_content() {
        let result = render_markdown("```bash\necho hello\n```", 100);
        assert!(result.len() >= 1);
    }

    #[test]
    fn test_mixed_markdown() {
        let result = render_markdown(
            "# Title\n\n- Item 1\n- Item 2\n\nCode: `example`\n\n```rust\nfn test() {}\n```",
            100,
        );
        assert!(result.len() >= 5);
    }

    #[test]
    fn test_paragraph_grouping() {
        let result = render_markdown("Line 1\nLine 2\nLine 3", 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_paragraph_with_blank_line() {
        let result = render_markdown("Line 1\nLine 2\n\nLine 3", 100);
        assert_eq!(result.len(), 3);
    }
}
