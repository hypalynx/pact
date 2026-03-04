use crate::app::App;
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Draw the bash command confirmation modal.
/// Shows the dangerous command, reason for concern, and allow/deny options.
pub fn draw_bash_confirm(app: &App, frame: &mut Frame) {
    if let Some(confirm) = &app.pending_bash_confirm {
        let height = 5_u16;
        let area = Rect {
            x: app.input_rect.x,
            y: app.input_rect.y.saturating_sub(height),
            width: app.input_rect.width,
            height,
        };

        if app.input_rect.y >= height {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::EMPTY)
                .title("Dangerous Command")
                .style(Style::default().bg(Color::Black));

            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });

            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Build content lines
            let mut lines = Vec::new();

            // Command line (truncate if too long)
            let cmd_display = truncate_text(&confirm.command, inner.width as usize, 2);
            lines.push(Line::from(Span::styled(
                format!("  {}", cmd_display),
                Style::default().fg(Color::Yellow),
            )));

            // Reason line (truncate if too long)
            let reason_display = truncate_text(&confirm.reason, inner.width as usize, 2);
            lines.push(Line::from(Span::styled(
                format!("  Reason: {}", reason_display),
                Style::default().fg(Color::DarkGray),
            )));

            // Instructions line
            lines.push(Line::from(Span::styled(
                "  [y] Allow   [n] Deny",
                Style::default().fg(Color::Cyan),
            )));

            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

/// Draw the ask question modal.
/// Shows a question with multiple choice options or freeform input.
pub fn draw_ask_question(app: &App, frame: &mut Frame) {
    if let Some(modal) = &app.pending_ask_question {
        let height = (5 + modal.options.len() as u16).min(15);
        let area = Rect {
            x: app.input_rect.x,
            y: app.input_rect.y.saturating_sub(height),
            width: app.input_rect.width,
            height,
        };

        if app.input_rect.y >= height {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::EMPTY)
                .title("Question")
                .style(Style::default().bg(Color::Black));

            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });

            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            let mut lines = Vec::new();

            // Question text
            lines.push(Line::from(Span::styled(
                modal.question.clone(),
                Style::default().fg(Color::Cyan),
            )));

            // Options
            if !modal.options.is_empty() {
                lines.push(Line::from("")); // Blank line
                for (idx, option) in modal.options.iter().enumerate() {
                    let is_selected = Some(idx) == get_selected_index(&modal.input, idx);
                    let style = if is_selected {
                        Style::default().bg(Color::Rgb(50, 50, 50)).fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let option_text = if option.len() > (inner.width as usize).saturating_sub(4) {
                        format!("{}...", &option[..inner.width.saturating_sub(7) as usize])
                    } else {
                        option.clone()
                    };

                    let prefix = if is_selected { "▸ " } else { "  " };
                    lines.push(Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled(option_text, style),
                    ]));
                }
            } else {
                // Freeform input mode
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("> {}", truncate_text(&modal.input, inner.width as usize, 3)),
                    Style::default().fg(Color::White),
                )));
            }

            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

/// Truncate text with ellipsis if it exceeds available width.
/// The padding parameter accounts for prefix characters (like "  " or "> ").
fn truncate_text(text: &str, max_width: usize, padding: usize) -> String {
    let available = max_width.saturating_sub(padding);
    if text.len() > available {
        format!("{}...", &text[..available.saturating_sub(3)])
    } else {
        text.to_string()
    }
}

/// Determine if an option index should be selected based on current input.
/// Returns the index if the input is a valid number matching that index.
fn get_selected_index(input: &str, default_idx: usize) -> Option<usize> {
    if input.is_empty() {
        return None;
    }
    input.trim().parse::<usize>().ok().or(Some(default_idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_text_no_truncate() {
        assert_eq!(truncate_text("hello", 10, 0), "hello");
        assert_eq!(truncate_text("test", 5, 0), "test");
    }

    #[test]
    fn test_truncate_text_with_padding() {
        // 10 width - 2 padding = 8 available, "hello world" (11 chars) > 8, so truncated
        assert_eq!(truncate_text("hello world", 10, 2), "hello...");
        // 10 width - 5 padding = 5 available, "hello" (5 chars) fits exactly
        assert_eq!(truncate_text("hello", 10, 5), "hello");
    }

    #[test]
    fn test_truncate_text_edge_cases() {
        // Exact fit
        assert_eq!(truncate_text("12345", 5, 0), "12345");
        // Just over
        assert_eq!(truncate_text("123456", 5, 0), "12...");
        // Very small available space
        assert_eq!(truncate_text("test", 3, 0), "...");
    }

    #[test]
    fn test_get_selected_index_valid() {
        assert_eq!(get_selected_index("0", 5), Some(0));
        assert_eq!(get_selected_index("3", 5), Some(3));
        assert_eq!(get_selected_index(" 2 ", 5), Some(2)); // whitespace trimmed
    }

    #[test]
    fn test_get_selected_index_invalid() {
        // Invalid input uses default
        assert_eq!(get_selected_index("abc", 5), Some(5));
        assert_eq!(get_selected_index("", 3), None);
    }

    #[test]
    fn test_get_selected_index_out_of_range() {
        // Out of range is still returned (caller should validate)
        assert_eq!(get_selected_index("99", 5), Some(99));
    }
}
