use crate::app::App;
use crate::text::{parse_markdown_line, wrap_text};
use crate::utils::{format_tokens, get_git_branch, get_pwd_display};
use ratatui::Frame;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

fn parse_color(color_str: &str) -> Color {
    match color_str.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "dark_gray" | "darkgray" | "dark-gray" => Color::DarkGray,
        _ => Color::White,
    }
}

pub fn draw_app(app: &mut App, frame: &mut Frame) {
    let margin = ratatui::layout::Margin::new(1, 1);
    let area = frame.area().inner(margin);

    let input_lines =
        (app.input.lines().count() + if app.input.ends_with('\n') { 1 } else { 0 }).max(1) as u16;
    let input_height = (input_lines + 2).min(10).max(3);

    let vertical = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(input_height),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);

    let [messages_area, _gap1, input_area, _gap2, status_area] = vertical.areas(area);

    app.messages_rect = messages_area;
    app.input_rect = input_area;

    let (at_bottom, _) = app.calculate_scroll_info();
    app.was_at_bottom = at_bottom;

    draw_messages(app, frame);
    draw_input(app, frame);
    draw_status(app, frame, status_area);

    if app.show_debug {
        draw_debug_modal(app, frame);
    }
}

fn draw_messages(app: &App, frame: &mut Frame) {
    let mut lines = Vec::new();
    let available_width = (app.messages_rect.width.saturating_sub(4)) as usize;

    for msg in &app.messages {
        if msg.role == "user" {
            let wrapped = wrap_text(&msg.text, available_width);
            for line_text in wrapped {
                let padded = format!("  {}  ", line_text);
                let style = if msg.is_tool_result {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().bg(Color::Black)
                };
                lines.push(Line::from(vec![Span::styled(padded, style)]));
            }
        } else {
            // Render thinking tokens first (if present)
            if let Some(thinking) = &msg.thinking {
                let wrapped = wrap_text(thinking, available_width);
                for line_text in wrapped {
                    let style = Style::default().fg(Color::DarkGray).italic();
                    let padded = format!("  {}  ", line_text);
                    lines.push(Line::from(vec![Span::styled(padded, style)]));
                }
                // Empty line between thinking and response
                lines.push(Line::from(""));
            }
            // Render main response text
            let wrapped = wrap_text(&msg.text, available_width);
            for line_text in wrapped {
                let spans = parse_markdown_line(&line_text);
                lines.push(Line::from(spans));
            }
        }
        lines.push(Line::from(""));
    }

    // Render pending thinking tokens (while streaming)
    if !app.pending_thinking.is_empty() {
        let wrapped = wrap_text(&app.pending_thinking, available_width);
        for line_text in wrapped {
            let style = Style::default().fg(Color::DarkGray).italic();
            let padded = format!("  {}  ", line_text);
            lines.push(Line::from(vec![Span::styled(padded, style)]));
        }
        // Empty line between thinking and response
        if !app.pending_response.is_empty() {
            lines.push(Line::from(""));
        }
    }

    // Render pending response text
    if !app.pending_response.is_empty() {
        let wrapped = wrap_text(&app.pending_response, available_width);
        for line_text in wrapped {
            let spans = parse_markdown_line(&line_text);
            lines.push(Line::from(spans));
        }
    }

    let line_count = lines.len() as u16;
    let max_scroll = line_count.saturating_sub(app.messages_rect.height);
    let start_line = (app.scroll_offset as u16).min(max_scroll);

    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(start_line as usize)
        .take(app.messages_rect.height as usize)
        .collect();

    frame.render_widget(Paragraph::new(visible_lines), app.messages_rect);

    let at_bottom = start_line >= max_scroll;
    if !at_bottom {
        let current_line = start_line.saturating_add(1);
        let position_text = format!("[{}/{}]", current_line, line_count);
        let pos_width = position_text.len() as u16;
        if pos_width < app.messages_rect.width {
            let pos_x = app.messages_rect.x + app.messages_rect.width.saturating_sub(pos_width + 1);
            let pos_area = Rect {
                x: pos_x,
                y: app.messages_rect.y,
                width: pos_width,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(position_text).style(Style::default().fg(Color::DarkGray)),
                pos_area,
            );
        }
    }

    if line_count > app.messages_rect.height {
        let scrollbar_height = (app.messages_rect.height as f64 * app.messages_rect.height as f64
            / line_count as f64)
            .max(1.0) as u16;
        let scrollable_height = app.messages_rect.height.saturating_sub(scrollbar_height);
        let scrollbar_pos = ((start_line as f64 / max_scroll.max(1) as f64).min(1.0)
            * scrollable_height as f64) as u16;

        let mut scrollbar_lines = Vec::new();
        for y_offset in 0..app.messages_rect.height {
            if y_offset >= scrollbar_pos && y_offset < scrollbar_pos + scrollbar_height {
                scrollbar_lines.push(Line::from(Span::styled(
                    "█",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                scrollbar_lines.push(Line::from(Span::raw(" ")));
            }
        }

        let scrollbar_area = Rect {
            x: app.messages_rect.x + app.messages_rect.width.saturating_sub(1),
            y: app.messages_rect.y,
            width: 1,
            height: app.messages_rect.height,
        };
        frame.render_widget(Paragraph::new(scrollbar_lines), scrollbar_area);
    }
}

fn draw_input(app: &App, frame: &mut Frame) {
    let margin = Paragraph::new("").style(Style::default().bg(Color::Black));
    frame.render_widget(margin, app.input_rect);

    let inner = app.input_rect.inner(ratatui::layout::Margin {
        horizontal: 3,
        vertical: 1,
    });

    let input =
        Paragraph::new(app.input.clone()).style(Style::default().fg(Color::White).bg(Color::Black));
    frame.render_widget(input, inner);

    if !app.loading {
        let (cursor_x, cursor_y) = cursor_position(&app.input, app.cursor_pos);
        let cursor_pos = ratatui::layout::Position {
            x: inner.x + cursor_x as u16,
            y: inner.y + cursor_y as u16,
        };
        frame.set_cursor_position(cursor_pos);
    }
}

fn cursor_position(input: &str, cursor_pos: usize) -> (usize, usize) {
    let mut x = 0;
    let mut y = 0;
    let mut byte_count = 0;

    for c in input.chars() {
        if byte_count >= cursor_pos {
            break;
        }
        if c == '\n' {
            y += 1;
            x = 0;
        } else {
            x += 1;
        }
        byte_count += c.len_utf8();
    }
    (x, y)
}

fn draw_status(app: &App, frame: &mut Frame, area: Rect) {
    let mut left_spans = Vec::new();

    // Show error notification if present, otherwise copy notification, otherwise normal status
    if let Some(ref error) = app.error_message {
        left_spans.push(Span::styled(
            format!("⚠ {}", error),
            Style::default().fg(Color::Red),
        ));
    } else if app.is_copying() {
        left_spans.push(Span::styled(
            "Copied to clipboard!",
            Style::default().fg(Color::Yellow),
        ));
    } else {
        let pwd = get_pwd_display();
        let git_branch = get_git_branch();

        left_spans.push(Span::styled(pwd, Style::default().fg(Color::DarkGray)));

        if let Some(branch) = git_branch {
            left_spans.push(Span::raw(" "));
            left_spans.push(Span::styled(
                format!("[{}]", branch),
                Style::default().fg(Color::DarkGray),
            ));
        }

        left_spans.push(Span::raw(" "));
        let mode_color = app
            .mode_color
            .as_ref()
            .map(|c| parse_color(c))
            .unwrap_or(Color::White);
        left_spans.push(Span::styled(
            app.mode_name.clone(),
            Style::default().fg(mode_color),
        ));

        if app.loading {
            left_spans.push(Span::raw(" "));
            let braille_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let braille = braille_frames[((app.frame_count / 3) as usize) % braille_frames.len()];
            left_spans.push(Span::styled(
                braille.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let tokens_used = app.total_input_tokens + app.total_output_tokens;
    let percentage = if app.context_window > 0 {
        (tokens_used * 100) / app.context_window
    } else {
        0
    };
    let right_text = format!(
        "{} | {}/{} ({}%)",
        app.model_name,
        format_tokens(tokens_used),
        format_tokens(app.context_window),
        percentage
    );

    let status_style = Style::default().fg(Color::DarkGray);

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

fn draw_debug_modal(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();
    let modal_width = (frame_area.width * 9 / 10).max(40);
    let modal_height = (frame_area.height * 8 / 10).max(10);

    let modal_x = (frame_area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (frame_area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Debug: API Logs  [e]rrors-only  [c]lear logs  [m]clear msgs  [Esc]close ")
        .title_alignment(ratatui::layout::Alignment::Left);

    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    let mut lines = Vec::new();

    // Filter logs based on error filter
    let filtered_logs: Vec<_> = if app.debug_filter_errors {
        app.debug_logs
            .iter()
            .filter(|log| log.error_message.is_some())
            .collect()
    } else {
        app.debug_logs.iter().collect()
    };

    // Apply scroll offset
    let visible_logs = filtered_logs
        .iter()
        .skip(app.debug_scroll)
        .take((inner.height as usize).saturating_sub(1));

    for log in visible_logs {
        let status_icon = if log.error_message.is_some() {
            Span::styled("✗", Style::default().fg(Color::Red))
        } else {
            Span::styled("✓", Style::default().fg(Color::Green))
        };

        let time_str = log
            .created_at
            .split('T')
            .nth(1)
            .unwrap_or("")
            .split('+')
            .next()
            .unwrap_or("");
        let time_span = Span::styled(time_str.to_string(), Style::default().fg(Color::DarkGray));

        let duration_str = format!("{}ms", log.duration_ms.unwrap_or(0));
        let duration_span = Span::raw(format!("  {}  ", duration_str));

        if let Some(ref err) = log.error_message {
            let error_text = format!("Error: {}", err);
            lines.push(Line::from(vec![
                time_span,
                Span::raw("  "),
                status_icon,
                duration_span,
                Span::styled(error_text, Style::default().fg(Color::Red)),
            ]));
        } else {
            let truncated_body = if log.request_body.len() > 60 {
                format!("{}...", &log.request_body[..57])
            } else {
                log.request_body.clone()
            };
            lines.push(Line::from(vec![
                time_span,
                Span::raw("  "),
                status_icon,
                duration_span,
                Span::raw(truncated_body),
            ]));
        }
    }

    frame.render_widget(block, modal_area);
    frame.render_widget(Paragraph::new(lines), inner);
}
