use crate::app::App;
use crate::text::{render_message, wrap_text};
use crate::ui::colors::*;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Highlight a character range within a line of styled spans
/// Used for selection highlighting in the messages area
pub fn highlight_line_range(
    line: Line<'static>,
    char_start: usize,
    char_end: usize,
) -> Line<'static> {
    let mut new_spans = Vec::new();
    let mut pos = 0;
    let highlight_style = Style::default().fg(Color::Blue);

    for span in line.spans {
        let span_len = span.content.chars().count();
        let span_end = pos + span_len;

        if span_end <= char_start || pos >= char_end {
            // Span is completely before or after the selection
            new_spans.push(span);
        } else {
            // Span overlaps with selection - split it into individual chars
            let span_chars: Vec<char> = span.content.chars().collect();

            for (current_pos, ch) in span_chars.iter().enumerate() {
                let char_pos = pos + current_pos;

                if char_pos >= char_start && char_pos < char_end {
                    // Char is in selection range - apply blue highlight
                    let highlighted_style = span.style.patch(highlight_style);
                    new_spans.push(Span::styled(ch.to_string(), highlighted_style));
                } else {
                    // Char is not in selection range - keep original style
                    new_spans.push(Span::styled(ch.to_string(), span.style));
                }
            }
        }

        pos = span_end;
    }

    Line::from(new_spans)
}

/// Draw the messages area - renders all conversation history
/// Handles user messages, assistant responses, tool results, thinking tokens,
/// selection highlighting, and scroll indicators
pub fn draw_messages(app: &mut App, frame: &mut ratatui::Frame, is_dimmed: bool) {
    let mut lines: Vec<Line> = Vec::new();
    let mut text_lines = Vec::new();
    let available_width = (app.messages_rect.width.saturating_sub(4)) as usize;

    // Dimming colors - use darker/grayed versions when modal is open
    let dim_text_color = if is_dimmed { DIM_TEXT } else { Color::White };
    let dim_bg_color = if is_dimmed { DIM_BG } else { Color::Black };

    for msg in &app.messages {
        // Skip messages with no content (empty text and no thinking)
        if msg.text.is_empty() && msg.thinking.is_none() {
            continue;
        }

        if msg.role == "user" {
            // If this is a tool result, remove the blank line separator from the previous message
            if msg.is_tool_result
                && !lines.is_empty()
                && lines.last().map(|l| l.spans.is_empty()).unwrap_or(false)
            {
                lines.pop();
                text_lines.pop();
            }

            // Add top padding with dimmed background (but not for tool results)
            if !msg.is_tool_result {
                let padding_line = Span::styled("".to_string(), Style::default().bg(dim_bg_color));
                lines.push(Line::from(vec![padding_line]));
                text_lines.push(String::new());
            }

            if msg.is_tool_result {
                // Tool results: show summary line first
                let padded = format!("  {}  ", msg.text);
                let tool_fg = if is_dimmed {
                    DIM_THINKING
                } else {
                    Color::DarkGray
                };
                let style = Style::default().fg(tool_fg).italic().bg(dim_bg_color);
                lines.push(Line::from(vec![Span::styled(padded, style)]));
                text_lines.push(msg.text.clone());

                // Only show full content for tools that should display their output
                let should_display_content = msg
                    .tool_name
                    .as_ref()
                    .map(|name| {
                        matches!(
                            name.as_str(),
                            "Write"
                                | "Edit"
                                | "Bash"
                                | "write"
                                | "edit"
                                | "bash"
                                | "TaskCreate"
                                | "TaskList"
                                | "TaskGet"
                                | "TaskUpdate"
                        )
                    })
                    .unwrap_or(false);

                if should_display_content {
                    // Use render_message for syntax highlighting (e.g., diffs, code blocks)
                    if let Some(content) = &msg.tool_result_content {
                        // Determine color for task tools (amber/yellow instead of gray)
                        let is_task_tool = msg
                            .tool_name
                            .as_ref()
                            .map(|name| {
                                matches!(
                                    name.as_str(),
                                    "TaskCreate" | "TaskList" | "TaskGet" | "TaskUpdate"
                                )
                            })
                            .unwrap_or(false);

                        let content_fg = if is_task_tool {
                            if is_dimmed {
                                Color::Rgb(180, 140, 0) // Muted amber
                            } else {
                                Color::Yellow
                            }
                        } else {
                            tool_fg
                        };

                        // Truncate bash output for display (full content still sent to LLM)
                        let display_content: std::borrow::Cow<str> = if matches!(
                            msg.tool_name.as_deref(),
                            Some("Bash") | Some("bash")
                        ) {
                            const MAX_BASH_DISPLAY_LINES: usize = 20;
                            let lines: Vec<&str> = content.lines().collect();
                            if lines.len() > MAX_BASH_DISPLAY_LINES {
                                let truncated = lines[..MAX_BASH_DISPLAY_LINES].join("\n");
                                std::borrow::Cow::Owned(format!(
                                    "{}\n[... {} more lines ...]",
                                    truncated,
                                    lines.len() - MAX_BASH_DISPLAY_LINES
                                ))
                            } else {
                                std::borrow::Cow::Borrowed(content)
                            }
                        } else {
                            std::borrow::Cow::Borrowed(content)
                        };

                        for (line_text, spans) in render_message(&display_content, available_width) {
                            let mut padded_spans = vec![Span::raw("  ")];
                            // Apply tool result styling: tint all spans with tool_fg color
                            for span in spans {
                                let mut style = Style::default().fg(content_fg).bg(dim_bg_color);
                                // Preserve text styling but override foreground color
                                if let Some(color) = span.style.fg {
                                    // Use a blend of the span's color and tool_fg
                                    // For now, prefer the original highlight color (e.g., green for +, red for -)
                                    style = Style::default().fg(color).bg(dim_bg_color);
                                }
                                padded_spans.push(Span::styled(span.content.to_string(), style));
                            }
                            padded_spans.push(Span::raw("  "));
                            lines.push(Line::from(padded_spans));
                            text_lines.push(line_text);
                        }
                    }
                }
            } else {
                // Regular user message
                let wrapped = wrap_text(&msg.text, available_width);
                for line_text in wrapped {
                    let padded = format!("  {}  ", line_text);
                    let style = Style::default().bg(dim_bg_color).fg(dim_text_color);
                    lines.push(Line::from(vec![Span::styled(padded, style)]));
                    text_lines.push(line_text);
                }
            }
        } else {
            // Render thinking tokens first (if present)
            if let Some(thinking) = &msg.thinking {
                let thinking_color = if is_dimmed {
                    DIM_THINKING
                } else {
                    Color::DarkGray
                };
                for (line_text, spans) in render_message(thinking, available_width) {
                    let mut padded_spans = vec![Span::raw("  ")];
                    for span in spans {
                        let new_style = span.style.add_modifier(Modifier::ITALIC);
                        let new_style = if new_style.fg.is_none() {
                            new_style.fg(thinking_color)
                        } else {
                            new_style
                        };
                        padded_spans.push(Span::styled(span.content.to_string(), new_style));
                    }
                    padded_spans.push(Span::raw("  "));
                    lines.push(Line::from(padded_spans));
                    text_lines.push(line_text);
                }
                // Empty line between thinking and response (only if there's response text)
                if !msg.text.is_empty() {
                    lines.push(Line::from(""));
                    text_lines.push(String::new());
                }
            }
            // Render main response text
            for (line_text, spans) in render_message(&msg.text, available_width) {
                // Add padding spans around the parsed spans
                let mut padded_spans = vec![Span::raw("  ")];
                // When dimmed, force all spans to use dark gray color
                if is_dimmed {
                    for span in spans {
                        padded_spans.push(Span::styled(
                            span.content.to_string(),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                } else {
                    padded_spans.extend(spans);
                }
                padded_spans.push(Span::raw("  "));
                lines.push(Line::from(padded_spans));
                text_lines.push(line_text);
            }
        }
        // Add bottom padding for user messages with dimmed background (but not for tool results)
        if msg.role == "user" && !msg.is_tool_result {
            let padding_line = Span::styled("".to_string(), Style::default().bg(dim_bg_color));
            lines.push(Line::from(vec![padding_line]));
        } else {
            lines.push(Line::from(""));
        }
        text_lines.push(String::new());
    }

    // Render pending thinking tokens (while streaming)
    if !app.pending_thinking.is_empty() {
        let thinking_color = if is_dimmed {
            DIM_THINKING
        } else {
            Color::DarkGray
        };
        for (line_text, spans) in render_message(&app.pending_thinking, available_width) {
            let mut padded_spans = vec![Span::raw("  ")];
            for span in spans {
                let new_style = span.style.add_modifier(Modifier::ITALIC);
                let new_style = if new_style.fg.is_none() {
                    new_style.fg(thinking_color)
                } else {
                    new_style
                };
                padded_spans.push(Span::styled(span.content.to_string(), new_style));
            }
            padded_spans.push(Span::raw("  "));
            lines.push(Line::from(padded_spans));
            text_lines.push(line_text);
        }
        // Empty line between thinking and response
        if !app.pending_response.is_empty() {
            lines.push(Line::from(""));
            text_lines.push(String::new());
        }
    }

    // Render pending response text
    if !app.pending_response.is_empty() {
        for (line_text, spans) in render_message(&app.pending_response, available_width) {
            // Add padding spans around the parsed spans
            let mut padded_spans = vec![Span::raw("  ")];
            // When dimmed, force all spans to use dark gray color
            if is_dimmed {
                for span in spans {
                    padded_spans.push(Span::styled(
                        span.content.to_string(),
                        Style::default().fg(DIM_TEXT),
                    ));
                }
            } else {
                padded_spans.extend(spans);
            }
            padded_spans.push(Span::raw("  "));
            lines.push(Line::from(padded_spans));
            text_lines.push(line_text);
        }
    }

    // Store the text lines for selection extraction
    app.all_line_texts = text_lines;

    // Renderer-driven scroll: single source of truth for scroll position
    let line_count = lines.len();
    let viewport = app.messages_rect.height as usize;
    let max_scroll = line_count.saturating_sub(viewport);

    if app.auto_scroll {
        app.scroll_offset = max_scroll;
    }
    app.scroll_offset = app.scroll_offset.min(max_scroll);
    app.rendered_line_count = line_count;

    let line_count_u16 = line_count as u16;
    let max_scroll_u16 = max_scroll as u16;
    let start_line = app.scroll_offset as u16;

    // Apply selection highlighting if selection is active
    let visible_lines: Vec<Line> = if let (Some(sel_start), Some(sel_end)) =
        (app.selection_start, app.selection_end)
    {
        let min_row = sel_start.1.min(sel_end.1);
        let max_row = sel_start.1.max(sel_end.1);
        let (col_start, col_end) =
            if sel_start.1 < sel_end.1 || (sel_start.1 == sel_end.1 && sel_start.0 <= sel_end.0) {
                (sel_start.0, sel_end.0)
            } else {
                (sel_end.0, sel_start.0)
            };

        // For highlighting, use column position within the rendered line (includes padding)
        let text_col_start = (col_start as usize).saturating_sub(app.messages_rect.x as usize);
        let text_col_end = (col_end as usize).saturating_sub(app.messages_rect.x as usize);

        lines
            .into_iter()
            .enumerate()
            .skip(start_line as usize)
            .take(app.messages_rect.height as usize)
            .map(|(full_idx, line)| {
                let vis_idx = full_idx - (start_line as usize);
                let screen_row = app.messages_rect.y + vis_idx as u16;

                if screen_row < min_row || screen_row > max_row {
                    line
                } else if min_row == max_row {
                    // Single line selection
                    highlight_line_range(line, text_col_start, text_col_end)
                } else if screen_row == min_row {
                    // First line of multi-line selection
                    highlight_line_range(line, text_col_start, usize::MAX)
                } else if screen_row == max_row {
                    // Last line of multi-line selection
                    highlight_line_range(line, 0, text_col_end)
                } else {
                    // Middle line - highlight entire line
                    let highlight_style = Style::default().fg(Color::Blue);
                    line.style(highlight_style)
                }
            })
            .collect()
    } else {
        lines
            .into_iter()
            .skip(start_line as usize)
            .take(app.messages_rect.height as usize)
            .collect()
    };

    frame.render_widget(Paragraph::new(visible_lines), app.messages_rect);

    // Draw position indicator when not at bottom
    let at_bottom = start_line >= max_scroll_u16;
    if !at_bottom {
        let current_line = start_line.saturating_add(1);
        let position_text = format!("[{}/{}]", current_line, line_count_u16);
        let pos_width = position_text.len() as u16;
        if pos_width < app.messages_rect.width {
            let pos_x = app.messages_rect.x + app.messages_rect.width.saturating_sub(pos_width + 1);
            let pos_area = Rect {
                x: pos_x,
                y: app.messages_rect.y,
                width: pos_width,
                height: 1,
            };
            let pos_color = if is_dimmed {
                DIM_SCROLLBAR
            } else {
                Color::DarkGray
            };
            frame.render_widget(
                Paragraph::new(position_text).style(Style::default().fg(pos_color)),
                pos_area,
            );
        }
    }

    // Draw scrollbar if content exceeds viewport
    if line_count > app.messages_rect.height as usize {
        let scrollbar_height = (app.messages_rect.height as f64 * app.messages_rect.height as f64
            / line_count as f64)
            .ceil() as u16;
        let scrollbar_height = scrollbar_height.clamp(1, app.messages_rect.height);

        let thumb_pos = if max_scroll_u16 == 0 {
            0
        } else {
            (start_line as f64 / max_scroll_u16 as f64
                * (app.messages_rect.height - scrollbar_height) as f64) as u16
        };

        let thumb_y = app.messages_rect.y + thumb_pos;
        let thumb_area = Rect {
            x: app.messages_rect.x + app.messages_rect.width - 1,
            y: thumb_y,
            width: 1,
            height: scrollbar_height,
        };

        let scrollbar_color = if is_dimmed {
            DIM_SCROLLBAR
        } else {
            Color::White
        };
        frame.render_widget(
            ratatui::widgets::Block::default().style(Style::default().bg(scrollbar_color)),
            thumb_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use ratatui::text::{Line, Span};

    #[test]
    fn test_highlight_line_range_no_overlap() {
        // Line: "Hello World" (spans: "Hello" + " " + "World")
        // Positions: 0-4 ("Hello"), 5 (" "), 6-10 ("World")
        let line = Line::from(vec![
            Span::styled("Hello", Style::default().fg(Color::White)),
            Span::styled(" ", Style::default()),
            Span::styled("World", Style::default().fg(Color::White)),
        ]);

        // Selection is at position 12-15, which is AFTER all content (positions 0-10)
        // So the selection doesn't overlap with any spans
        let result = highlight_line_range(line.clone(), 12, 15);
        assert_eq!(result.spans.len(), 3); // Unchanged - all spans pass through as-is
    }

    #[test]
    fn test_highlight_line_range_full_overlap() {
        // Create a line with single span of 5 chars
        let line = Line::from(vec![Span::styled(
            "Hello",
            Style::default().fg(Color::White),
        )]);

        // Select the entire span (positions 0-5)
        let result = highlight_line_range(line, 0, 5);

        // Should be split into 5 individual character spans, all highlighted blue
        assert_eq!(result.spans.len(), 5);
        for span in &result.spans {
            assert_eq!(span.style.fg, Some(Color::Blue));
        }
    }

    #[test]
    fn test_highlight_line_range_partial_overlap_start() {
        // Line with single span "Hello World" (11 chars)
        let line = Line::from(vec![Span::styled(
            "Hello World",
            Style::default().fg(Color::Red),
        )]);

        // Select positions 0-5 ("Hello")
        let result = highlight_line_range(line, 0, 5);

        // Should have 11 spans: first 5 blue, rest red
        assert_eq!(result.spans.len(), 11);
        for (i, span) in result.spans.iter().enumerate() {
            if i < 5 {
                assert_eq!(
                    span.style.fg,
                    Some(Color::Blue),
                    "char {} should be blue",
                    i
                );
            } else {
                assert_eq!(span.style.fg, Some(Color::Red), "char {} should be red", i);
            }
        }
    }

    #[test]
    fn test_highlight_line_range_partial_overlap_end() {
        // Line with single span "Hello World" (11 chars)
        let line = Line::from(vec![Span::styled(
            "Hello World",
            Style::default().fg(Color::Red),
        )]);

        // Select positions 6-11 ("World")
        let result = highlight_line_range(line, 6, 11);

        // Should have 11 spans: first 6 red, last 5 blue
        assert_eq!(result.spans.len(), 11);
        for (i, span) in result.spans.iter().enumerate() {
            if i < 6 {
                assert_eq!(span.style.fg, Some(Color::Red), "char {} should be red", i);
            } else {
                assert_eq!(
                    span.style.fg,
                    Some(Color::Blue),
                    "char {} should be blue",
                    i
                );
            }
        }
    }

    #[test]
    fn test_highlight_line_range_multiple_spans() {
        // Line with multiple spans: "Hello" + " " + "World"
        let line = Line::from(vec![
            Span::styled("Hello", Style::default().fg(Color::Red)),
            Span::styled(" ", Style::default().fg(Color::Green)),
            Span::styled("World", Style::default().fg(Color::Blue)),
        ]);

        // Select positions 3-8 (crosses all 3 spans: "lo" + " " + "Wo")
        let result = highlight_line_range(line, 3, 8);

        // Should have 11 spans (each char becomes its own span when highlighted)
        assert_eq!(result.spans.len(), 11);

        // First 3 chars keep original colors (Red)
        for i in 0..3 {
            assert_eq!(
                result.spans[i].style.fg,
                Some(Color::Red),
                "char {} should be red",
                i
            );
        }

        // Chars 3-8 should be blue (highlighted)
        for i in 3..8 {
            assert_eq!(
                result.spans[i].style.fg,
                Some(Color::Blue),
                "char {} should be blue",
                i
            );
        }

        // Remaining chars keep original colors (Blue)
        for i in 8..11 {
            assert_eq!(
                result.spans[i].style.fg,
                Some(Color::Blue),
                "char {} should be blue",
                i
            );
        }
    }

    #[test]
    fn test_highlight_line_range_empty_selection() {
        // Line with single span
        let line = Line::from(vec![Span::styled(
            "Hello",
            Style::default().fg(Color::White),
        )]);

        // Empty selection (start == end)
        let result = highlight_line_range(line, 3, 3);

        // No characters should be highlighted
        assert_eq!(result.spans.len(), 5);
        for span in &result.spans {
            assert_eq!(span.style.fg, Some(Color::White));
        }
    }
}
