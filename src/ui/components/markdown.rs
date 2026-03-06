use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::ui::colors::THEME;

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

fn render_code_block_lines(code: &str, language: Option<&str>, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Reserve space for line numbers (4 chars: "  1 |")
    let gutter_width = 6;
    let code_width = width.saturating_sub(gutter_width);

    // Language label header
    if let Some(lang) = language {
        let label = format!("  {} ", lang.to_uppercase());
        lines.push(Line::from(vec![
            Span::styled("╭", Style::default().fg(THEME.comment)),
            Span::styled("─".repeat(3), Style::default().fg(THEME.comment)),
            Span::styled(
                label,
                Style::default()
                    .fg(THEME.orange)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "─".repeat(code_width.saturating_sub(lang.len() + 6).min(20)),
                Style::default().fg(THEME.comment),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("╭", Style::default().fg(THEME.comment)),
            Span::styled(
                "─".repeat(code_width.min(20)),
                Style::default().fg(THEME.comment),
            ),
        ]));
    }

    // Code lines with line numbers
    for (i, code_line) in code.lines().enumerate() {
        let line_num = format!("{:4} │ ", i + 1);
        let line_width = code_line.width();

        if line_width <= code_width {
            lines.push(Line::from(vec![
                Span::styled(line_num, Style::default().fg(THEME.comment)),
                Span::styled(
                    code_line.to_string(),
                    Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
                ),
            ]));
        } else {
            // Wrap long lines
            let chars: Vec<char> = code_line.chars().collect();
            let mut first = true;
            for chunk in chars.chunks(code_width) {
                if first {
                    lines.push(Line::from(vec![
                        Span::styled(line_num.clone(), Style::default().fg(THEME.comment)),
                        Span::styled(
                            chunk.iter().collect::<String>(),
                            Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
                        ),
                    ]));
                    first = false;
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("     │ ", Style::default().fg(THEME.comment)),
                        Span::styled(
                            chunk.iter().collect::<String>(),
                            Style::default().fg(THEME.cyan).bg(THEME.tool_bg),
                        ),
                    ]));
                }
            }
        }
    }

    // Bottom border
    lines.push(Line::from(vec![
        Span::styled("╰", Style::default().fg(THEME.comment)),
        Span::styled(
            "─".repeat(code_width.min(20)),
            Style::default().fg(THEME.comment),
        ),
    ]));

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
                if current_width + 1 <= width {
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

/// Returns true if the character is a CJK ideograph or punctuation.
fn is_cjk(c: char) -> bool {
    // Basic CJK Unified Ideographs block
    ('\u{4E00}'..='\u{9FFF}').contains(&c) ||
    // CJK Symbols and Punctuation (full-width comma, period, etc.)
    ('\u{3000}'..='\u{303F}').contains(&c) ||
    // Halfwidth and Fullwidth Forms
    ('\u{FF00}'..='\u{FFEF}').contains(&c)
}

pub fn render_markdown(content: &str, width: usize) -> Vec<Line<'static>> {
    let segments = detect_code_blocks(content);
    let mut lines = Vec::new();

    for segment in segments {
        match segment {
            Segment::Code { code, language } => {
                lines.extend(render_code_block_lines(&code, language.as_deref(), width));
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
                            // CJK Heuristic: Only insert a space when joining lines if
                            // neither the preceding nor the current character is CJK.
                            let last_char = paragraph.chars().last();
                            let first_char = current_line.chars().next();

                            let needs_space = match (last_char, first_char) {
                                (Some(l), Some(f)) => !is_cjk(l) && !is_cjk(f),
                                _ => true,
                            };

                            if needs_space {
                                paragraph.push(' ');
                            }
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
}
