use crate::app::App;
use crate::ui::layout::{
    DEBUG_MODAL_HEIGHT_PERCENT, DEBUG_MODAL_MIN_HEIGHT, DEBUG_MODAL_MIN_WIDTH,
    DEBUG_MODAL_WIDTH_PERCENT,
};
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use serde_json::Value;

/// Draw the debug modal showing API logs and request details.
/// Supports two views: list view (all logs) and expanded view (single log details).
pub fn draw_debug_modal(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();
    let modal_width =
        (frame_area.width * DEBUG_MODAL_WIDTH_PERCENT / 10).max(DEBUG_MODAL_MIN_WIDTH);
    let modal_height =
        (frame_area.height * DEBUG_MODAL_HEIGHT_PERCENT / 10).max(DEBUG_MODAL_MIN_HEIGHT);

    let modal_x = (frame_area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (frame_area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    let title = if app.debug_expanded_row.is_some() {
        " Debug: Request Details  [Esc]back "
    } else {
        " Debug: API Logs  [↑↓]select  [Enter]expand  [e]rrors  [c]lear logs  [m]clear msgs  [Esc]back "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::EMPTY)
        .title(title)
        .title_alignment(Alignment::Left)
        .style(Style::default().bg(Color::Black));

    let inner = modal_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if let Some(expanded_idx) = app.debug_expanded_row {
        draw_expanded_view(app, frame, modal_area, inner, block, expanded_idx);
    } else {
        draw_list_view(app, frame, modal_area, inner, block);
    }
}

/// Draw the expanded view showing full details of a single API log entry.
fn draw_expanded_view(
    app: &App,
    frame: &mut Frame,
    modal_area: Rect,
    inner: Rect,
    block: Block,
    expanded_idx: usize,
) {
    let filtered_logs = app.debug_filtered_logs();
    if let Some(log) = filtered_logs.get(expanded_idx) {
        let mut lines = Vec::new();

        // Header info
        lines.push(Line::from(vec![
            Span::styled("Time: ", Style::default().fg(Color::Cyan)),
            Span::raw(log.created_at.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Duration: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}ms", log.duration_ms.unwrap_or(0))),
        ]));

        if let Some(err) = &log.error_message {
            lines.push(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(Color::Red)),
                Span::raw(err.clone()),
            ]));
        }

        // Request body
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Request Body:",
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(""));

        if let Ok(json) = serde_json::from_str::<Value>(&log.request_body) {
            if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                for pretty_line in pretty.lines() {
                    lines.push(Line::from(Span::raw(pretty_line.to_string())));
                }
            }
        } else {
            lines.push(Line::from(Span::raw(log.request_body.clone())));
        }

        // Full response (accumulated content)
        if let Some(full_response) = &log.full_response {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Response Body:",
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::raw(full_response.clone())));
        }

        // SSE events (raw response body)
        if let Some(response) = &log.response_body {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "SSE Events:",
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(""));

            for line in response.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("data: ") {
                    let json_str = trimmed.strip_prefix("data: ").unwrap_or(trimmed);
                    if let Ok(json) = serde_json::from_str::<Value>(json_str)
                        && let Ok(pretty) = serde_json::to_string_pretty(&json)
                    {
                        for pretty_line in pretty.lines() {
                            lines.push(Line::from(Span::raw(pretty_line.to_string())));
                        }
                        continue;
                    }
                }
                lines.push(Line::from(Span::raw(line.to_string())));
            }
        }

        // Apply scroll within expanded view
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(app.debug_expand_scroll)
            .take(inner.height as usize)
            .collect();

        frame.render_widget(block, modal_area);
        frame.render_widget(Clear, inner);
        frame.render_widget(
            Paragraph::new(visible_lines)
                .style(Style::default().bg(Color::Black))
                .scroll((0, app.debug_expand_scroll_x as u16)),
            inner,
        );
    }
}

/// Draw the list view showing all API logs with selection highlighting.
fn draw_list_view(app: &App, frame: &mut Frame, modal_area: Rect, inner: Rect, block: Block) {
    let filtered_logs = app.debug_filtered_logs();
    let mut lines = Vec::new();

    for (idx, log) in filtered_logs.iter().enumerate() {
        let is_selected = idx == app.debug_selected_row;
        let status_icon = if log.error_message.is_some() {
            Span::styled("✗", Style::default().fg(Color::Red))
        } else {
            Span::styled("✓", Style::default().fg(Color::Green))
        };

        let time_span = format_time(&log.created_at);
        let duration_span = format_duration(log.duration_ms.unwrap_or(0));
        let description = extract_description(&log.request_body);
        let bg_style = get_bg_style(is_selected);

        if let Some(err) = &log.error_message {
            let error_text = format!("Error: {}", err);
            let mut spans = vec![
                get_status_icon(status_icon, is_selected, true),
                Span::styled("  ", bg_style),
                get_time_span(&time_span.content, is_selected),
                Span::styled("  ", bg_style),
                Span::styled(duration_span.content.clone(), bg_style),
                Span::styled(
                    error_text,
                    if is_selected {
                        Style::default().fg(Color::Red).bg(Color::Rgb(50, 50, 50))
                    } else {
                        Style::default().fg(Color::Red)
                    },
                ),
            ];
            if is_selected {
                spans.push(Span::styled(
                    " ".repeat(100),
                    Style::default().bg(Color::Rgb(50, 50, 50)),
                ));
            }
            lines.push(Line::from(spans));
        } else {
            let mut spans = vec![
                get_status_icon(status_icon, is_selected, false),
                Span::styled("  ", bg_style),
                get_time_span(&time_span.content, is_selected),
                Span::styled("  ", bg_style),
                Span::styled(duration_span.content.clone(), bg_style),
                Span::styled(description, bg_style),
            ];
            if is_selected {
                spans.push(Span::styled(
                    " ".repeat(100),
                    Style::default().bg(Color::Rgb(50, 50, 50)),
                ));
            }
            lines.push(Line::from(spans));
        }
    }

    // Apply scroll offset
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(app.debug_scroll)
        .take(inner.height as usize)
        .collect();

    frame.render_widget(block, modal_area);
    frame.render_widget(Clear, inner);
    frame.render_widget(
        Paragraph::new(visible_lines).style(Style::default().bg(Color::Black)),
        inner,
    );
}

/// Format a timestamp string for display (extract time portion with ms precision).
fn format_time(created_at: &str) -> Span<'static> {
    let time_str = created_at
        .split('T')
        .nth(1)
        .unwrap_or("")
        .split('+')
        .next()
        .unwrap_or("");

    // Truncate timestamp to 3 decimal places (milliseconds)
    let time_display = if let Some(dot_idx) = time_str.find('.') {
        let base = &time_str[..dot_idx];
        let decimals = &time_str[dot_idx + 1..];
        let truncated = if decimals.len() > 3 {
            &decimals[..3]
        } else {
            decimals
        };
        format!("{}.{}", base, truncated)
    } else {
        time_str.to_string()
    };

    Span::styled(time_display, Style::default().fg(Color::White))
}

/// Format duration in milliseconds for display.
fn format_duration(duration_ms: i64) -> Span<'static> {
    let duration_str = format!("{:>6}ms", duration_ms);
    Span::raw(format!("  {}  ", duration_str))
}

/// Extract a description from the request body JSON.
/// Returns the last user message content, truncated to 50 chars, or "tool call".
fn extract_description(request_body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<Value>(request_body) {
        if let Some(messages) = json.get("messages").and_then(|m| m.as_array()) {
            messages
                .iter()
                .rev()
                .find(|msg| {
                    msg.get("role")
                        .and_then(|r| r.as_str())
                        .map(|r| r == "user")
                        .unwrap_or(false)
                })
                .and_then(|msg| msg.get("content"))
                .and_then(|content| content.as_str())
                .map(|s| {
                    if s.len() > 50 {
                        format!("{}...", &s[..47])
                    } else {
                        s.to_string()
                    }
                })
                .unwrap_or_else(|| "tool call".to_string())
        } else {
            "tool call".to_string()
        }
    } else {
        "tool call".to_string()
    }
}

/// Get background style based on selection state.
fn get_bg_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().bg(Color::Rgb(50, 50, 50))
    } else {
        Style::default()
    }
}

/// Get status icon span with appropriate styling.
fn get_status_icon(icon: Span<'static>, is_selected: bool, is_error: bool) -> Span<'static> {
    if is_selected {
        if is_error {
            Span::styled(
                "✗",
                Style::default().fg(Color::Red).bg(Color::Rgb(50, 50, 50)),
            )
        } else {
            Span::styled(
                "✓",
                Style::default().fg(Color::Green).bg(Color::Rgb(50, 50, 50)),
            )
        }
    } else {
        icon
    }
}

/// Get time span with selection-aware styling.
fn get_time_span(time_content: &str, is_selected: bool) -> Span<'static> {
    Span::styled(
        time_content.to_string(),
        if is_selected {
            Style::default().fg(Color::White).bg(Color::Rgb(50, 50, 50))
        } else {
            Style::default().fg(Color::White)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time_with_milliseconds() {
        let span = format_time("2024-01-15T14:30:45.123456+00:00");
        assert_eq!(span.content, "14:30:45.123");
    }

    #[test]
    fn test_format_time_without_milliseconds() {
        let span = format_time("2024-01-15T14:30:45+00:00");
        assert_eq!(span.content, "14:30:45");
    }

    #[test]
    fn test_format_time_invalid() {
        let span = format_time("invalid");
        assert_eq!(span.content, "");
    }

    #[test]
    fn test_format_duration() {
        // format!("{:>6}ms", 123) = "   123ms", then wrapped in "  {}  " = "      123ms  "
        let span = format_duration(123);
        assert_eq!(span.content, "     123ms  ");

        let span = format_duration(12345);
        assert_eq!(span.content, "   12345ms  ");
    }

    #[test]
    fn test_extract_description_with_user_message() {
        let json = r#"{"messages": [{"role": "user", "content": "Hello world"}]}"#;
        assert_eq!(extract_description(json), "Hello world");
    }

    #[test]
    fn test_extract_description_long_message() {
        let json = r#"{"messages": [{"role": "user", "content": "This is a very long message that exceeds fifty characters in length"}]}"#;
        let result = extract_description(json);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn test_extract_description_multiple_messages() {
        let json = r#"{"messages": [
            {"role": "assistant", "content": "Hi"},
            {"role": "user", "content": "First user message"},
            {"role": "user", "content": "Last user message"}
        ]}"#;
        // Should find the last user message in array order (the last "user" entry)
        assert_eq!(extract_description(json), "Last user message");
    }

    #[test]
    fn test_extract_description_no_messages() {
        let json = r#"{}"#;
        assert_eq!(extract_description(json), "tool call");
    }

    #[test]
    fn test_extract_description_no_user_message() {
        let json = r#"{"messages": [{"role": "assistant", "content": "Hi"}]}"#;
        assert_eq!(extract_description(json), "tool call");
    }

    #[test]
    fn test_extract_description_invalid_json() {
        assert_eq!(extract_description("not json"), "tool call");
    }

    #[test]
    fn test_debug_modal_constants() {
        let _: u16 = DEBUG_MODAL_WIDTH_PERCENT;
        let _: u16 = DEBUG_MODAL_HEIGHT_PERCENT;
        let _: u16 = DEBUG_MODAL_MIN_WIDTH;
        let _: u16 = DEBUG_MODAL_MIN_HEIGHT;

        assert!(DEBUG_MODAL_WIDTH_PERCENT > 0 && DEBUG_MODAL_WIDTH_PERCENT <= 10);
        assert!(DEBUG_MODAL_HEIGHT_PERCENT > 0 && DEBUG_MODAL_HEIGHT_PERCENT <= 10);
        assert!(DEBUG_MODAL_MIN_WIDTH > 0);
        assert!(DEBUG_MODAL_MIN_HEIGHT > 0);
    }

    #[test]
    fn test_get_bg_style() {
        let selected = get_bg_style(true);
        let not_selected = get_bg_style(false);

        // Just verify they return different styles
        assert_ne!(format!("{:?}", selected), format!("{:?}", not_selected));
    }
}
