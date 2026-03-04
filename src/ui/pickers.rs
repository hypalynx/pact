use crate::app::App;
use crate::ui::layout::DEBUG_FILE_PICKER_MAX_VISIBLE;
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Draw the file picker popup above the input area.
/// Shows filtered file entries with selection highlighting and scrolling.
pub fn draw_file_picker(app: &App, frame: &mut Frame) {
    if let Some(picker) = &app.file_picker {
        let max_visible = DEBUG_FILE_PICKER_MAX_VISIBLE;
        let height = (picker.filtered.len().min(max_visible) + 2).max(3) as u16;

        // Position: top edge of input_rect, same horizontal position
        let area = Rect {
            x: app.input_rect.x,
            y: app.input_rect.y.saturating_sub(height),
            width: app.input_rect.width,
            height,
        };

        // Only draw if there's space above the input
        if app.input_rect.y >= height {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::EMPTY)
                .style(Style::default().bg(Color::Black));

            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });

            // Draw background
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Render visible entries
            let start_idx =
                calculate_scroll_offset(picker.selected, max_visible, picker.filtered.len());
            let end_idx = (start_idx + max_visible).min(picker.filtered.len());

            let mut lines = Vec::new();
            for (i, entry) in picker.filtered[start_idx..end_idx].iter().enumerate() {
                let idx = start_idx + i;
                let is_selected = idx == picker.selected;
                let style = if is_selected {
                    Style::default().bg(Color::Rgb(50, 50, 50))
                } else {
                    Style::default()
                };

                let truncated = truncate_entry(entry, inner.width as usize);
                lines.push(Line::from(Span::styled(truncated, style)));
            }

            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

/// Draw the slash command picker popup above the input area.
/// Similar to file picker but with command-specific rendering and hints.
pub fn draw_slash_picker(app: &App, frame: &mut Frame) {
    if let Some(picker) = &app.slash_picker {
        let max_visible = DEBUG_FILE_PICKER_MAX_VISIBLE;
        let height = (picker.filtered.len().min(max_visible) + 2).max(3) as u16;

        // Position: top edge of input_rect, same horizontal position
        let area = Rect {
            x: app.input_rect.x,
            y: app.input_rect.y.saturating_sub(height),
            width: app.input_rect.width,
            height,
        };

        // Only draw if there's space above the input
        if app.input_rect.y >= height {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::EMPTY)
                .style(Style::default().bg(Color::Black));

            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });

            // Draw background
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Render visible entries
            let start_idx =
                calculate_scroll_offset(picker.selected, max_visible, picker.filtered.len());
            let end_idx = (start_idx + max_visible).min(picker.filtered.len());

            let mut lines = Vec::new();

            // Show entries if there are any
            if !picker.filtered.is_empty() {
                for (i, entry) in picker.filtered[start_idx..end_idx].iter().enumerate() {
                    let idx = start_idx + i;
                    let is_selected = idx == picker.selected;
                    let style = if is_selected {
                        Style::default().bg(Color::Rgb(50, 50, 50))
                    } else {
                        Style::default()
                    };

                    let truncated = truncate_entry(entry, inner.width as usize);
                    lines.push(Line::from(Span::styled(truncated, style)));
                }
            } else if matches!(picker.command, crate::app::SlashCommand::Model) {
                // No models available - show hint for manual entry
                lines.push(Line::from(vec![
                    Span::styled("No models found. ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Type model ID and press Enter",
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
                if !picker.query.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Will use: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&picker.query, Style::default().fg(Color::Cyan)),
                    ]));
                }
            }

            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

/// Draw the API key input modal above the input area.
/// Shows a masked input field for entering sensitive API keys.
pub fn draw_api_key_input(app: &App, frame: &mut Frame) {
    if let Some(key) = &app.api_key_input {
        let height = 3_u16;
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
                .title("Enter API Key")
                .style(Style::default().bg(Color::Black));

            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });

            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Mask the API key with asterisks
            let masked = "*".repeat(key.len());
            let lines = vec![Line::from(Span::styled(masked, Style::default()))];
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

/// Calculate the scroll offset for a picker based on selection and visible count.
/// Ensures the selected item is always visible within the picker window.
fn calculate_scroll_offset(selected: usize, max_visible: usize, total_items: usize) -> usize {
    if selected > max_visible - 1 && max_visible > 0 {
        (selected - max_visible + 1).min(total_items.saturating_sub(max_visible))
    } else {
        0
    }
}

/// Truncate an entry string if it exceeds the available width.
/// Adds "..." suffix when truncating.
fn truncate_entry(entry: &str, max_width: usize) -> String {
    if entry.len() > max_width {
        let truncate_at = max_width.saturating_sub(3);
        format!("{}...", &entry[..truncate_at])
    } else {
        entry.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_scroll_offset_start() {
        // At the beginning, no scrolling needed
        assert_eq!(calculate_scroll_offset(0, 8, 100), 0);
        assert_eq!(calculate_scroll_offset(3, 8, 100), 0);
        assert_eq!(calculate_scroll_offset(7, 8, 100), 0);
    }

    #[test]
    fn test_calculate_scroll_offset_middle() {
        // In the middle, scroll to keep selection visible
        assert_eq!(calculate_scroll_offset(8, 8, 100), 1);
        assert_eq!(calculate_scroll_offset(15, 8, 100), 8);
    }

    #[test]
    fn test_calculate_scroll_offset_end() {
        // At the end, scroll to keep selection visible without going past last item
        // Selected 99 with 8 visible in 100 items: scroll to 92 (showing 92-99)
        assert_eq!(calculate_scroll_offset(99, 8, 100), 92);
        // Selected 95 with 8 visible: scroll to 88 (showing 88-95)
        assert_eq!(calculate_scroll_offset(95, 8, 100), 88);
    }

    #[test]
    fn test_calculate_scroll_offset_small_list() {
        // Small list - no scrolling needed
        assert_eq!(calculate_scroll_offset(5, 8, 6), 0);
        assert_eq!(calculate_scroll_offset(0, 8, 5), 0);
    }

    #[test]
    fn test_truncate_entry_no_truncate() {
        assert_eq!(truncate_entry("hello", 10), "hello");
        assert_eq!(truncate_entry("", 5), "");
        assert_eq!(truncate_entry("exact", 5), "exact");
    }

    #[test]
    fn test_truncate_entry_with_ellipsis() {
        assert_eq!(truncate_entry("hello world", 8), "hello...");
        assert_eq!(truncate_entry("very long string", 6), "ver...");
        assert_eq!(truncate_entry("test", 2), "...");
    }

    #[test]
    fn test_truncate_entry_edge_cases() {
        // Exactly at boundary
        assert_eq!(truncate_entry("12345", 5), "12345");
        // One over
        assert_eq!(truncate_entry("123456", 5), "12...");
        // Very small width
        assert_eq!(truncate_entry("test", 3), "...");
        assert_eq!(truncate_entry("test", 0), "...");
    }
}
