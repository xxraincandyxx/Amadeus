use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::ui::theme_manager::get_colors;

/// Segment of parsed markdown content
enum Segment {
    Text(String),
    Code {
        code: String,
        language: Option<String>,
    },
}

fn detect_code_blocks(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut in_code_block = false;
    let mut current_segment = String::new();
    let mut current_language: Option<String> = None;
    let mut first_segment = true;

    for line in content.lines() {
        if line.starts_with("```") {
            if in_code_block {
                segments.push(Segment::Code {
                    code: current_segment.clone(),
                    language: current_language.take(),
                });
                current_segment.clear();
                in_code_block = false;
            } else {
                if !current_segment.is_empty() {
                    segments.push(Segment::Text(current_segment.clone()));
                    current_segment.clear();
                }
                // Extract language from the code fence line
                let lang = line.trim_start_matches('`').trim();
                current_language = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                };
                in_code_block = true;
                first_segment = true;
            }
        } else if in_code_block {
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

    if !current_segment.is_empty() {
        if in_code_block {
            segments.push(Segment::Code {
                code: current_segment,
                language: current_language,
            });
        } else {
            segments.push(Segment::Text(current_segment));
        }
    }

    segments
}

fn render_code_block_lines(
    code: &str,
    language: Option<&str>,
    width: usize,
    colors: &crate::ui::semantic_colors::SemanticColors,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Reserve space for line numbers (4 chars: "  1 |")
    let gutter_width = 6;
    let code_width = width.saturating_sub(gutter_width);

    // Language label header
    if let Some(lang) = language {
        let label = format!("  {} ", lang.to_uppercase());
        lines.push(Line::from(vec![
            Span::styled("╭", Style::default().fg(colors.ui.comment)),
            Span::styled("─".repeat(3), Style::default().fg(colors.ui.comment)),
            Span::styled(
                label,
                Style::default()
                    .fg(colors.status.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "─".repeat(code_width.saturating_sub(lang.len() + 6).min(20)),
                Style::default().fg(colors.ui.comment),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("╭", Style::default().fg(colors.ui.comment)),
            Span::styled(
                "─".repeat(code_width.min(20)),
                Style::default().fg(colors.ui.comment),
            ),
        ]));
    }

    // Code lines with line numbers
    for (i, code_line) in code.lines().enumerate() {
        let line_num = format!("{:4} │ ", i + 1);
        let line_width = code_line.width();

        if line_width <= code_width {
            lines.push(Line::from(vec![
                Span::styled(line_num, Style::default().fg(colors.ui.comment)),
                Span::styled(code_line.to_string(), Style::default().fg(colors.ui.symbol)),
            ]));
        } else {
            // Wrap long lines
            let chars: Vec<char> = code_line.chars().collect();
            let mut first = true;
            for chunk in chars.chunks(code_width) {
                if first {
                    lines.push(Line::from(vec![
                        Span::styled(line_num.clone(), Style::default().fg(colors.ui.comment)),
                        Span::styled(
                            chunk.iter().collect::<String>(),
                            Style::default().fg(colors.ui.symbol),
                        ),
                    ]));
                    first = false;
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("     │ ", Style::default().fg(colors.ui.comment)),
                        Span::styled(
                            chunk.iter().collect::<String>(),
                            Style::default().fg(colors.ui.symbol),
                        ),
                    ]));
                }
            }
        }
    }

    // Bottom border
    lines.push(Line::from(vec![
        Span::styled("╰", Style::default().fg(colors.ui.comment)),
        Span::styled(
            "─".repeat(code_width.min(20)),
            Style::default().fg(colors.ui.comment),
        ),
    ]));

    lines
}

fn render_inline_code_spans(
    segment: &str,
    colors: &crate::ui::semantic_colors::SemanticColors,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_inline_code = false;

    for c in segment.chars() {
        if c == '`' {
            if in_inline_code {
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(colors.ui.symbol),
                ));
                current.clear();
                in_inline_code = false;
            } else {
                if !current.is_empty() {
                    spans.extend(render_inline_bold_spans(&current, colors));
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
                Style::default().fg(colors.ui.symbol),
            ));
        } else {
            spans.extend(render_inline_bold_spans(&current, colors));
        }
    }

    spans
}

fn render_inline_bold_spans(
    text: &str,
    colors: &crate::ui::semantic_colors::SemanticColors,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_bold = false;

    for c in text.chars() {
        if c == '*' {
            if in_bold {
                if !current.is_empty() {
                    spans.push(Span::styled(
                        current.clone(),
                        Style::default()
                            .fg(colors.text.primary)
                            .add_modifier(Modifier::BOLD),
                    ));
                    current.clear();
                }
                in_bold = false;
            } else {
                if !current.is_empty() {
                    spans.push(Span::styled(
                        current.clone(),
                        Style::default().fg(colors.text.primary),
                    ));
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
                Style::default()
                    .fg(colors.text.primary)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                current.clone(),
                Style::default().fg(colors.text.primary),
            ));
        }
    }

    spans
}

fn render_text_line(
    line: &str,
    width: usize,
    colors: &crate::ui::semantic_colors::SemanticColors,
) -> Vec<Line<'static>> {
    let trimmed = line.trim_start();

    if trimmed.is_empty() {
        return vec![Line::from("")];
    }

    let header_line = if let Some(stripped) = trimmed.strip_prefix("### ") {
        Some(Line::from(vec![Span::styled(
            stripped.trim_end().to_string(),
            Style::default()
                .fg(colors.text.accent)
                .add_modifier(Modifier::BOLD),
        )]))
    } else if let Some(stripped) = trimmed.strip_prefix("## ") {
        Some(Line::from(vec![Span::styled(
            stripped.trim_end().to_string(),
            Style::default()
                .fg(colors.text.accent)
                .add_modifier(Modifier::BOLD),
        )]))
    } else if let Some(stripped) = trimmed.strip_prefix("# ") {
        Some(Line::from(vec![Span::styled(
            stripped.trim_end().to_string(),
            Style::default()
                .fg(colors.text.accent)
                .add_modifier(Modifier::BOLD),
        )]))
    } else {
        None
    };

    if let Some(line) = header_line {
        return vec![line];
    }

    if let Some(content) = trimmed.strip_prefix("- ") {
        let spans = render_inline_code_spans(content, colors);
        let mut final_spans = vec![Span::styled("• ", Style::default().fg(colors.text.accent))];
        final_spans.extend(spans);
        return wrap_lines(final_spans, width);
    }

    if let Some(content) = trimmed.strip_prefix("* ") {
        let spans = render_inline_code_spans(content, colors);
        let mut final_spans = vec![Span::styled("• ", Style::default().fg(colors.text.accent))];
        final_spans.extend(spans);
        return wrap_lines(final_spans, width);
    }

    let spans = render_inline_code_spans(trimmed, colors);
    let mut final_spans = Vec::new();
    let indent = &line[0..line.len() - trimmed.len()];
    if !indent.is_empty() {
        final_spans.push(Span::styled(
            indent.to_string(),
            Style::default().fg(colors.text.primary),
        ));
    }
    final_spans.extend(spans);
    wrap_lines(final_spans, width)
}

fn wrap_lines(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    if spans.is_empty() || width == 0 {
        return vec![Line::from(spans)];
    }

    let mut lines = Vec::new();
    let mut current_line_spans = Vec::new();
    let mut current_width = 0;

    for span in spans {
        let style = span.style;
        let content = span.content.as_ref();

        // If the span contains spaces, we can wrap at those spaces
        let words: Vec<&str> = if content.contains(' ') {
            // We want to keep the spaces to account for their width
            let mut parts = Vec::new();
            let mut start = 0;
            for (i, c) in content.char_indices() {
                if c == ' ' {
                    if i > start {
                        parts.push(&content[start..i]);
                    }
                    parts.push(" ");
                    start = i + 1;
                }
            }
            if start < content.len() {
                parts.push(&content[start..]);
            }
            parts
        } else {
            vec![content]
        };

        for word in words {
            let word_width = word.width();

            if word == " " {
                if current_width < width {
                    current_line_spans.push(Span::styled(" ", style));
                    current_width += 1;
                } else {
                    // Space at end of line, just drop it and start new line
                    if !current_line_spans.is_empty() {
                        lines.push(Line::from(current_line_spans));
                        current_line_spans = Vec::new();
                        current_width = 0;
                    }
                }
                continue;
            }

            if current_width + word_width > width {
                // Word doesn't fit on current line
                if !current_line_spans.is_empty() {
                    lines.push(Line::from(current_line_spans));
                    current_line_spans = Vec::new();
                    current_width = 0;
                }

                if word_width > width {
                    // Word is longer than the whole width, must break it
                    let mut remaining = word;
                    while !remaining.is_empty() {
                        let mut take = 0;
                        let mut w = 0;
                        for c in remaining.chars() {
                            let cw = c.width().unwrap_or(0);
                            if w + cw > width && take > 0 {
                                break;
                            }
                            w += cw;
                            take += c.len_utf8();
                        }

                        let chunk = &remaining[..take];
                        remaining = &remaining[take..];

                        if remaining.is_empty() {
                            current_line_spans.push(Span::styled(chunk.to_string(), style));
                            current_width = w;
                        } else {
                            lines.push(Line::from(vec![Span::styled(chunk.to_string(), style)]));
                        }
                    }
                } else {
                    current_line_spans.push(Span::styled(word.to_string(), style));
                    current_width = word_width;
                }
            } else {
                current_line_spans.push(Span::styled(word.to_string(), style));
                current_width += word_width;
            }
        }
    }

    if !current_line_spans.is_empty() {
        lines.push(Line::from(current_line_spans));
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

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 3
}

fn parse_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn is_table_separator(line: &str) -> bool {
    if !is_table_row(line) {
        return false;
    }

    let cells = parse_table_row(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let trimmed = cell.trim();
            !trimmed.is_empty() && trimmed.chars().all(|c| matches!(c, '-' | ':' | ' '))
        })
}

fn render_table_lines(
    header: &[String],
    rows: &[Vec<String>],
    colors: &crate::ui::semantic_colors::SemanticColors,
) -> Vec<Line<'static>> {
    let column_count = header
        .len()
        .max(rows.iter().map(|row| row.len()).max().unwrap_or(0));
    if column_count == 0 {
        return Vec::new();
    }

    let mut widths = vec![0usize; column_count];
    for (idx, cell) in header.iter().enumerate() {
        widths[idx] = widths[idx].max(cell.width());
    }
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.width());
        }
    }

    let render_row = |cells: &[String], is_header: bool| {
        let mut spans = Vec::new();
        spans.push(Span::styled("| ", Style::default().fg(colors.ui.comment)));

        for col in 0..column_count {
            let cell = cells.get(col).map(String::as_str).unwrap_or("");
            let padding = widths[col].saturating_sub(cell.width());
            let mut style = Style::default().fg(colors.text.primary);
            if is_header {
                style = style.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(cell.to_string(), style));
            if padding > 0 {
                spans.push(Span::styled(
                    " ".repeat(padding),
                    Style::default().fg(colors.text.primary),
                ));
            }
            spans.push(Span::styled(" |", Style::default().fg(colors.ui.comment)));
            if col + 1 < column_count {
                spans.push(Span::raw(" "));
            }
        }

        Line::from(spans)
    };

    let separator_cells: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
    let mut lines = vec![
        render_row(header, true),
        render_row(&separator_cells, false),
    ];
    for row in rows {
        lines.push(render_row(row, false));
    }
    lines
}

pub fn render_markdown(content: &str, width: usize) -> Vec<Line<'static>> {
    let segments = detect_code_blocks(content);
    let colors = get_colors();
    let mut lines = Vec::new();

    for segment in segments {
        match segment {
            Segment::Code { code, language } => {
                lines.extend(render_code_block_lines(
                    &code,
                    language.as_deref(),
                    width,
                    &colors,
                ));
            }
            Segment::Text(text) => {
                let input_lines: Vec<&str> = text.lines().collect();
                let mut i = 0;

                while i < input_lines.len() {
                    let line = input_lines[i];

                    if line.trim().is_empty() {
                        lines.push(Line::from(""));
                        i += 1;
                        continue;
                    }

                    if i + 1 < input_lines.len()
                        && is_table_row(line)
                        && is_table_separator(input_lines[i + 1])
                    {
                        let header = parse_table_row(line);
                        let mut rows = Vec::new();
                        i += 2;

                        while i < input_lines.len() && is_table_row(input_lines[i]) {
                            rows.push(parse_table_row(input_lines[i]));
                            i += 1;
                        }

                        lines.extend(render_table_lines(&header, &rows, &colors));
                        continue;
                    }

                    if is_header(line) || is_list_item(line) {
                        lines.extend(render_text_line(line, width, &colors));
                        i += 1;
                        continue;
                    }

                    lines.extend(render_text_line(line, width, &colors));
                    i += 1;
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
        // Should have: header, code line, footer (at least 3 lines)
        assert!(result.len() >= 3);
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
        assert!(!result.is_empty());
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
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_paragraph_with_blank_line() {
        let result = render_markdown("Line 1\nLine 2\n\nLine 3", 100);
        assert_eq!(result.len(), 4);
    }

    // New tests for language labels and line numbers

    #[test]
    fn test_code_block_with_language_label() {
        let result = render_markdown("```rust\nfn main() {}\n```", 100);
        // First line should contain "RUST" language label
        let first_line = &result[0];
        let content: String = first_line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(content.contains("RUST"));
    }

    #[test]
    fn test_code_block_with_python_label() {
        let result = render_markdown("```python\nprint('hello')\n```", 100);
        let first_line = &result[0];
        let content: String = first_line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(content.contains("PYTHON"));
    }

    #[test]
    fn test_code_block_with_no_language() {
        let result = render_markdown("```\ncode here\n```", 100);
        // Should still render without crashing
        assert!(!result.is_empty());
    }

    #[test]
    fn test_code_block_has_line_numbers() {
        let result = render_markdown("```rust\nline1\nline2\nline3\n```", 100);
        // Check that line numbers appear in the content
        let all_content: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_content.contains("1"));
        assert!(all_content.contains("2"));
        assert!(all_content.contains("3"));
    }

    #[test]
    fn test_code_block_has_borders() {
        let result = render_markdown("```rust\ncode\n```", 100);
        // First line should have top border character
        let first_line = &result[0];
        let content: String = first_line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(content.contains('╭') || content.contains('─'));
    }

    #[test]
    fn test_code_block_multiline() {
        let code = "```rust\nfn foo() {\n    bar()\n}\n```";
        let result = render_markdown(code, 100);
        // Should have: header + 3 code lines + footer = 5 lines minimum
        assert!(result.len() >= 5);
    }

    #[test]
    fn test_multiple_code_blocks() {
        let content = "```rust\ncode1\n```\n\nSome text\n\n```python\ncode2\n```";
        let result = render_markdown(content, 100);
        let all_content: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_content.contains("RUST"));
        assert!(all_content.contains("PYTHON"));
    }

    #[test]
    fn test_segment_detection() {
        // Test that segments are properly detected
        let segments = detect_code_blocks("Text\n```rust\ncode\n```\nMore text");
        assert_eq!(segments.len(), 3);
    }

    #[test]
    fn test_markdown_table_renders_aligned_columns() {
        let result = render_markdown(
            "| Name | Role |\n|------|------|\n| Alice | Engineer |\n| Bob | Designer |",
            100,
        );
        let rendered = result
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered
            .iter()
            .any(|line| line.contains("| Name  | Role     |")));
        assert!(rendered
            .iter()
            .any(|line| line.contains("| Alice | Engineer |")));
        assert!(rendered
            .iter()
            .any(|line| line.contains("| Bob   | Designer |")));
    }
}
