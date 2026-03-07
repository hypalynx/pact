use crate::app::App;
use crate::ui::colors::{DIM_COPYING, DIM_ERROR, DIM_MODE, DIM_STATUS, DIM_WARN, parse_color};
use crate::utils::{format_tokens, get_git_branch, get_pwd_display};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Draw the status bar showing pwd, git branch, mode, tokens, and status messages.
pub fn draw_status(app: &App, frame: &mut Frame, area: Rect, is_dimmed: bool) {
    use crate::app::StatusLevel;

    // Dimmed colors when modal is open
    let normal_fg = if is_dimmed {
        DIM_STATUS
    } else {
        Color::DarkGray
    };
    let mode_color = if is_dimmed {
        DIM_MODE
    } else {
        app.mode_color
            .as_ref()
            .map(|c| parse_color(c))
            .unwrap_or(Color::White)
    };
    let error_color = if is_dimmed { DIM_ERROR } else { Color::Red };
    let warn_color = if is_dimmed { DIM_WARN } else { Color::Yellow };
    let info_color = if is_dimmed { DIM_STATUS } else { Color::Cyan };
    let copying_color = if is_dimmed {
        DIM_COPYING
    } else {
        Color::Yellow
    };

    let mut left_spans = Vec::new();

    // Show status notification if recent, otherwise copy notification, otherwise normal status
    if app.has_status() {
        if let Some((ref msg, ref level)) = app.status_message {
            let color = match level {
                StatusLevel::Error => error_color,
                StatusLevel::Warn => warn_color,
                StatusLevel::Info => info_color,
            };
            left_spans.push(Span::styled(msg.to_string(), Style::default().fg(color)));
        }
    } else if app.is_exit_confirming() {
        left_spans.push(Span::styled(
            "Press Ctrl+C again to exit",
            Style::default().fg(normal_fg),
        ));
    } else if app.is_cancel_confirming() {
        left_spans.push(Span::styled(
            "Press ESC again to cancel current call",
            Style::default().fg(normal_fg),
        ));
    } else if app.is_copying() {
        left_spans.push(Span::styled(
            "Copied to clipboard!",
            Style::default().fg(copying_color),
        ));
    } else if app.was_just_cancelled() {
        left_spans.push(Span::styled(
            "Call cancelled",
            Style::default().fg(normal_fg),
        ));
    } else if app.has_pending_messages() {
        let count = app.pending_user_messages.len();
        let msg = if count == 1 {
            "1 message pending".to_string()
        } else {
            format!("{} messages pending", count)
        };
        left_spans.push(Span::styled(msg, Style::default().fg(info_color)));
    } else {
        let pwd = get_pwd_display();
        let git_branch = get_git_branch();

        left_spans.push(Span::styled(pwd, Style::default().fg(normal_fg)));

        if let Some(branch) = git_branch {
            left_spans.push(Span::raw(" "));
            left_spans.push(Span::styled(
                format!("[{}]", branch),
                Style::default().fg(normal_fg),
            ));
        }

        left_spans.push(Span::raw(" "));
        left_spans.push(Span::styled(
            app.mode_name.clone(),
            Style::default().fg(mode_color),
        ));

        if app.active_llm_calls > 0 {
            left_spans.push(Span::raw(" "));
            let braille_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let braille = braille_frames[((app.frame_count / 3) as usize) % braille_frames.len()];
            left_spans.push(Span::styled(
                braille.to_string(),
                Style::default().fg(mode_color),
            ));
        }
    }

    let tokens_used = app.total_input_tokens + app.total_output_tokens;
    let percentage = calculate_token_percentage(tokens_used, app.context_window);
    let provider_name = app
        .active_provider
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("local");
    // Use app.model_name (from server info) if available, otherwise fall back to provider's default_model
    let raw_model_id = if !app.model_name.is_empty() && app.model_name != "unknown" {
        app.model_name.clone()
    } else {
        app.active_provider
            .as_ref()
            .and_then(|p| p.default_model.clone())
            .unwrap_or_else(|| "local".to_string())
    };
    // Extract just the model name from paths like "accounts/fireworks/models/kimi-k2p5"
    let model_id = raw_model_id
        .rsplit('/')
        .next()
        .unwrap_or(&raw_model_id)
        .to_string();
    let right_text = format!(
        "[{}] {} | {}/{} ({}%)",
        provider_name,
        model_id,
        format_tokens(tokens_used),
        format_tokens(app.context_window),
        percentage,
    );

    let status_style = Style::default().fg(normal_fg);

    let total_width = area.width as usize;
    let left_width: usize = left_spans.iter().map(|s| s.content.len()).sum();
    let right_width = right_text.len();

    if left_width + right_width + 2 > total_width {
        frame.render_widget(Paragraph::new(Line::from(left_spans)), area);
    } else {
        let gap = total_width - left_width - right_width;
        left_spans.push(Span::raw(" ".repeat(gap)));
        left_spans.push(Span::styled(right_text, status_style));
        frame.render_widget(Paragraph::new(Line::from(left_spans)), area);
    }
}

/// Calculate token usage percentage.
/// Returns 0 if context_window is 0 to avoid division by zero.
pub fn calculate_token_percentage(tokens_used: usize, context_window: usize) -> usize {
    if context_window > 0 {
        (tokens_used * 100) / context_window
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_token_percentage_normal() {
        assert_eq!(calculate_token_percentage(50, 100), 50);
        assert_eq!(calculate_token_percentage(25, 100), 25);
        assert_eq!(calculate_token_percentage(100, 100), 100);
    }

    #[test]
    fn test_calculate_token_percentage_zero_window() {
        // Should not panic, should return 0
        assert_eq!(calculate_token_percentage(50, 0), 0);
        assert_eq!(calculate_token_percentage(0, 0), 0);
    }

    #[test]
    fn test_calculate_token_percentage_zero_used() {
        assert_eq!(calculate_token_percentage(0, 100), 0);
        assert_eq!(calculate_token_percentage(0, 1000), 0);
    }

    #[test]
    fn test_calculate_token_percentage_rounding() {
        // Integer division should floor the result
        assert_eq!(calculate_token_percentage(33, 100), 33);
        assert_eq!(calculate_token_percentage(1, 3), 33); // 33.33... -> 33
    }

    #[test]
    fn test_calculate_token_percentage_large_numbers() {
        // Test with realistic token counts
        assert_eq!(calculate_token_percentage(4096, 8192), 50);
        assert_eq!(calculate_token_percentage(8000, 8192), 97);
    }
}
