use crate::app::App;
use crate::text::{cursor_position, wrap_text};
use crate::ui::colors::*;
use crate::ui::layout::*;
use ratatui::{
    Frame,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Draw the input box with text wrapping, cursor positioning, and @mention highlighting.
pub fn draw_input(app: &mut App, frame: &mut Frame, is_dimmed: bool) {
    // Dimmed input background when modal is open
    let input_bg = if is_dimmed {
        DIM_BG
    } else {
        Color::Black
    };
    let input_fg = if is_dimmed {
        DIM_TEXT
    } else {
        Color::White
    };

    let margin = Paragraph::new("").style(Style::default().bg(input_bg));
    frame.render_widget(margin, app.input_rect);

    let inner = app.input_rect.inner(ratatui::layout::Margin {
        horizontal: INPUT_HORIZONTAL_MARGIN,
        vertical: INPUT_VERTICAL_MARGIN,
    });

    let available_width = (inner.width.saturating_sub(1)) as usize;
    let inner_height = inner.height as usize;

    // Wrap the input text to available width
    let wrapped_lines = wrap_text(&app.input, available_width);
    let total_lines = wrapped_lines.len();

    // Calculate cursor position to determine scroll offset
    let (cursor_x, cursor_y) = cursor_position(&app.input, app.cursor_pos, available_width);

    // Adjust scroll offset to ensure cursor is visible
    if total_lines > inner_height {
        if cursor_y >= app.input_scroll_offset + inner_height {
            // Cursor is below visible area - scroll down
            app.input_scroll_offset = cursor_y.saturating_sub(inner_height - 1);
        } else if cursor_y < app.input_scroll_offset {
            // Cursor is above visible area - scroll up
            app.input_scroll_offset = cursor_y;
        }
        // Ensure scroll offset doesn't exceed max
        let max_scroll = total_lines.saturating_sub(inner_height);
        app.input_scroll_offset = app.input_scroll_offset.min(max_scroll);
    } else {
        app.input_scroll_offset = 0;
    }

    let mut lines: Vec<Line> = Vec::new();
    for line_text in wrapped_lines {
        let spans = colorize_input(&line_text, is_dimmed);
        lines.push(Line::from(spans));
    }

    // Apply scroll to show the relevant portion of input
    let input = Paragraph::new(lines)
        .style(Style::default().bg(input_bg).fg(input_fg))
        .scroll((app.input_scroll_offset as u16, 0));
    frame.render_widget(input, inner);

    // Always show cursor in input area
    let visible_cursor_y = cursor_y.saturating_sub(app.input_scroll_offset);
    let cursor_pos = ratatui::layout::Position {
        x: inner.x + cursor_x as u16,
        y: inner.y + visible_cursor_y as u16,
    };
    frame.set_cursor_position(cursor_pos);
}

/// Colorize input text, highlighting @mentions in yellow.
/// Returns spans with appropriate colors based on dimmed state.
///
/// File references (@file.txt) are highlighted, but email addresses (user@domain.com)
/// are not, since they have alphanumeric text immediately before the @.
pub fn colorize_input(input: &str, is_dimmed: bool) -> Vec<Span<'static>> {
    let dim_fg = if is_dimmed { DIM_TEXT } else { Color::White };
    let dim_at = if is_dimmed { DIM_AT } else { Color::Yellow };

    let mut spans = Vec::new();
    let mut chars = input.chars().peekable();
    let mut current = String::new();

    while let Some(ch) = chars.next() {
        if ch == '@' {
            // Check if @ is preceded by alphanumeric text (indicating email, not file reference)
            let preceded_by_alphanumeric = !current.is_empty()
                && current.chars().last().map_or(false, |c| c.is_alphanumeric());

            if preceded_by_alphanumeric {
                // This looks like an email address (user@domain), not a file reference
                current.push('@');
                continue;
            }

            // Look ahead to see if this starts a valid mention
            let mut word = String::from("@");
            let mut has_valid_chars = false;

            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_alphanumeric()
                    || next_ch == '_'
                    || next_ch == '.'
                    || next_ch == '/'
                    || next_ch == '-'
                {
                    word.push(next_ch);
                    has_valid_chars = true;
                    chars.next();
                } else {
                    break;
                }
            }

            // Only treat as mention if it has valid characters after @
            if has_valid_chars {
                // Push any accumulated text before the @
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default().fg(dim_fg)));
                    current.clear();
                }
                // Add the @word in yellow (dimmed when modal is open)
                spans.push(Span::styled(word, Style::default().fg(dim_at)));
            } else {
                // Isolated @ - treat as regular text
                current.push('@');
            }
        } else {
            current.push(ch);
        }
    }

    // Push any remaining text
    if !current.is_empty() {
        spans.push(Span::styled(current, Style::default().fg(dim_fg)));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colorize_input_empty() {
        let spans = colorize_input("", false);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "");
    }

    #[test]
    fn test_colorize_input_no_mentions() {
        let spans = colorize_input("hello world", false);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello world");
    }

    #[test]
    fn test_colorize_input_simple_mention() {
        let spans = colorize_input("look @file.txt", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "look ");
        assert_eq!(spans[1].content, "@file.txt");
    }

    #[test]
    fn test_colorize_input_multiple_mentions() {
        let spans = colorize_input("@src/main.rs and @Cargo.toml", false);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "@src/main.rs");
        assert_eq!(spans[1].content, " and ");
        assert_eq!(spans[2].content, "@Cargo.toml");
    }

    #[test]
    fn test_colorize_input_mention_at_start() {
        let spans = colorize_input("@readme please", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "@readme");
        assert_eq!(spans[1].content, " please");
    }

    #[test]
    fn test_colorize_input_mention_at_end() {
        let spans = colorize_input("check this @log", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "check this ");
        assert_eq!(spans[1].content, "@log");
    }

    #[test]
    fn test_colorize_input_special_chars_in_mention() {
        // Test that mention parsing handles special characters correctly
        let spans = colorize_input("see @path/to/file_name.txt", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "see ");
        assert_eq!(spans[1].content, "@path/to/file_name.txt");
    }

    #[test]
    fn test_colorize_input_mention_with_dash() {
        let spans = colorize_input("check @my-file", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "check ");
        assert_eq!(spans[1].content, "@my-file");
    }

    #[test]
    fn test_colorize_input_mention_stops_at_space() {
        let spans = colorize_input("see @file here", false);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "see ");
        assert_eq!(spans[1].content, "@file");
        assert_eq!(spans[2].content, " here");
    }

    #[test]
    fn test_colorize_input_mention_stops_at_special_char() {
        // @ should stop at characters like comma, semicolon
        let spans = colorize_input("see @file,txt", false);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "see ");
        assert_eq!(spans[1].content, "@file");
        assert_eq!(spans[2].content, ",txt");
    }

    #[test]
    fn test_colorize_input_only_mention() {
        let spans = colorize_input("@file", false);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "@file");
    }

    #[test]
    fn test_colorize_input_isolated_at_sign() {
        // Lone @ followed by space or punctuation - should be regular text
        let spans = colorize_input("see @ here", false);
        // Isolated @ is treated as regular text since it doesn't start a mention
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "see @ here");
    }

    #[test]
    fn test_colorize_input_newline_in_text() {
        let spans = colorize_input("line1\nline2", false);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "line1\nline2");
    }

    #[test]
    fn test_colorize_input_dimmed() {
        let spans = colorize_input("@file", true);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "@file");
    }

    #[test]
    fn test_colorize_input_mention_with_numbers() {
        let spans = colorize_input("check @file123", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].content, "@file123");
    }

    #[test]
    fn test_colorize_input_mention_with_underscore() {
        let spans = colorize_input("see @my_file", false);
        assert_eq!(spans[1].content, "@my_file");
    }

    #[test]
    fn test_colorize_input_consecutive_mentions() {
        let spans = colorize_input("@file1 @file2", false);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "@file1");
        assert_eq!(spans[1].content, " ");
        assert_eq!(spans[2].content, "@file2");
    }

    #[test]
    fn test_colorize_input_mention_at_end_no_space() {
        let spans = colorize_input("text@file", false);
        // @ preceded by alphanumeric looks like email, not a file reference
        // File references need whitespace/punctuation boundary before @
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "text@file");
    }

    #[test]
    fn test_colorize_input_multiple_special_chars() {
        let spans = colorize_input("see @path/to/file_name-test.txt", false);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].content, "@path/to/file_name-test.txt");
    }

    #[test]
    fn test_colorize_input_email_not_highlighted() {
        // Email addresses should NOT be highlighted as mentions
        let spans = colorize_input("me@example.com", false);
        // Should be all one span, not highlighting @example.com
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "me@example.com");
    }

    #[test]
    fn test_colorize_input_email_in_context() {
        // Email in a sentence should not be highlighted
        let spans = colorize_input("send to user@domain.org please", false);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "send to user@domain.org please");
    }

    #[test]
    fn test_colorize_input_file_reference_with_space() {
        // File reference preceded by space should work
        let spans = colorize_input("check @config.json here", false);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "check ");
        assert_eq!(spans[1].content, "@config.json");
        assert_eq!(spans[2].content, " here");
    }
}
