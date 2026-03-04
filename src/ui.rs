use crate::app::App;
use crate::text::{cursor_position, render_message, wrap_text};
use crate::utils::{format_tokens, get_git_branch, get_pwd_display};
use ratatui::Frame;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

// Input box layout constants
const INPUT_MIN_HEIGHT: u16 = 3;
const INPUT_MAX_HEIGHT: u16 = 20;
const INPUT_HORIZONTAL_MARGIN: u16 = 3;
const INPUT_VERTICAL_MARGIN: u16 = 1;

// Control panel constants
const CONTROL_PANEL_WIDTH: u16 = 40;

// Debug modal constants
const DEBUG_MODAL_WIDTH_PERCENT: u16 = 9; // out of 10
const DEBUG_MODAL_HEIGHT_PERCENT: u16 = 8; // out of 10
const DEBUG_MODAL_MIN_WIDTH: u16 = 40;
const DEBUG_MODAL_MIN_HEIGHT: u16 = 10;
const DEBUG_FILE_PICKER_MAX_VISIBLE: usize = 8;

// Dimmed color palette (used when modal is open)
const DIM_BG: Color = Color::Rgb(20, 20, 20); // Darker background for input/message boxes
const DIM_TEXT: Color = Color::DarkGray; // Muted text color
const DIM_THINKING: Color = Color::Rgb(60, 60, 60); // Even darker for thinking/tool text
const DIM_STATUS: Color = Color::Rgb(60, 60, 60); // Dark gray for status bar elements
const DIM_MODE: Color = Color::Rgb(100, 100, 100); // Grayed out mode color
const DIM_ERROR: Color = Color::Rgb(100, 0, 0); // Muted red for errors
const DIM_COPYING: Color = Color::Rgb(100, 100, 0); // Muted yellow for copy notification
const DIM_WARN: Color = Color::Rgb(100, 100, 0); // Muted yellow for warnings
const DIM_AT: Color = Color::Rgb(100, 100, 0); // Muted @mention highlight
const DIM_SCROLLBAR: Color = Color::Rgb(40, 40, 40); // Darker scrollbar when dimmed

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

fn highlight_line_range(line: Line<'static>, char_start: usize, char_end: usize) -> Line<'static> {
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
            // Span overlaps with selection - split it
            let span_chars: Vec<char> = span.content.chars().collect();

            for (current_pos, ch) in span_chars.iter().enumerate() {
                let char_pos = pos + current_pos;

                if char_pos >= char_start && char_pos < char_end {
                    // Char is in selection range
                    let highlighted_style = span.style.patch(highlight_style);
                    new_spans.push(Span::styled(ch.to_string(), highlighted_style));
                } else {
                    // Char is not in selection range
                    new_spans.push(Span::styled(ch.to_string(), span.style));
                }
            }
        }

        pos = span_end;
    }

    Line::from(new_spans)
}

pub fn draw_app(app: &mut App, frame: &mut Frame) {
    let margin = ratatui::layout::Margin::new(1, 1);
    let area = frame.area().inner(margin);

    // Calculate input height based on wrapped lines, not just actual newlines
    let available_input_width = (area.width.saturating_sub(INPUT_HORIZONTAL_MARGIN * 2)) as usize;
    let wrapped_lines = wrap_text(&app.input, available_input_width);
    let input_height = ((wrapped_lines.len() + 2) as u16).clamp(INPUT_MIN_HEIGHT, INPUT_MAX_HEIGHT);

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

    // Check if any modal is open (excluding small pickers which don't need dimming)
    let is_modal_open = matches!(
        app.panel_state,
        crate::app::PanelState::ControlPanel | crate::app::PanelState::Debug
    ) || app.api_key_input.is_some()
        || app.pending_bash_confirm.is_some();

    draw_messages(app, frame, is_modal_open);
    draw_input(app, frame, is_modal_open);
    draw_status(app, frame, status_area, is_modal_open);

    // Note: We intentionally don't draw a solid overlay here.
    // The modals (control panel, debug, etc.) have their own backgrounds
    // that provide sufficient contrast with the main UI.
    // A solid color overlay would hide the UI behind it completely.

    // Draw panels
    match app.panel_state {
        crate::app::PanelState::None => {}
        crate::app::PanelState::ControlPanel => {
            draw_control_panel(app, frame);
        }
        crate::app::PanelState::Debug => {
            draw_debug_modal(app, frame);
        }
    }

    // Draw file picker if open
    if app.file_picker.is_some() {
        draw_file_picker(app, frame);
    }

    // Draw slash picker if open
    if app.slash_picker.is_some() {
        draw_slash_picker(app, frame);
    }

    // Draw API key input prompt if active
    if app.api_key_input.is_some() {
        draw_api_key_input(app, frame);
    }

    // Draw bash confirmation prompt if active
    if app.pending_bash_confirm.is_some() {
        draw_bash_confirm(app, frame);
    }
}

fn draw_messages(app: &mut App, frame: &mut Frame, is_dimmed: bool) {
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
                // Write, Edit, Bash show full content
                // Read, Glob, Grep, Webfetch only show summary
                let should_display_content = msg
                    .tool_name
                    .as_ref()
                    .map(|name| {
                        matches!(
                            name.as_str(),
                            "Write" | "Edit" | "Bash" | "write" | "edit" | "bash"
                        )
                    })
                    .unwrap_or(false);

                if should_display_content {
                    // Don't use wrap_text for diffs - preserve formatting
                    if let Some(content) = &msg.tool_result_content {
                        for line_text in content.lines() {
                            let style = Style::default().fg(tool_fg).bg(dim_bg_color);
                            let padded = format!("  {}  ", line_text);
                            lines.push(Line::from(vec![Span::styled(padded, style)]));
                            text_lines.push(line_text.to_string());
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
                let wrapped = wrap_text(thinking, available_width);
                let thinking_color = if is_dimmed {
                    DIM_THINKING
                } else {
                    Color::DarkGray
                };
                for line_text in wrapped {
                    let style = Style::default().fg(thinking_color).italic();
                    let padded = format!("  {}  ", line_text);
                    lines.push(Line::from(vec![Span::styled(padded, style)]));
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
        let wrapped = wrap_text(&app.pending_thinking, available_width);
        let thinking_color = if is_dimmed {
            DIM_THINKING
        } else {
            Color::DarkGray
        };
        for line_text in wrapped {
            let style = Style::default().fg(thinking_color).italic();
            let padded = format!("  {}  ", line_text);
            lines.push(Line::from(vec![Span::styled(padded, style)]));
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

    let line_count = line_count as u16;
    let max_scroll = max_scroll as u16;
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

    if line_count > app.messages_rect.height {
        let scrollbar_height = (app.messages_rect.height as f64 * app.messages_rect.height as f64
            / line_count as f64)
            .max(1.0) as u16;
        let scrollable_height = app.messages_rect.height.saturating_sub(scrollbar_height);
        let scrollbar_pos = ((start_line as f64 / max_scroll.max(1) as f64).min(1.0)
            * scrollable_height as f64) as u16;

        let scrollbar_color = if is_dimmed {
            DIM_SCROLLBAR
        } else {
            Color::DarkGray
        };

        let mut scrollbar_lines = Vec::new();
        for y_offset in 0..app.messages_rect.height {
            if y_offset >= scrollbar_pos && y_offset < scrollbar_pos + scrollbar_height {
                scrollbar_lines.push(Line::from(Span::styled(
                    "█",
                    Style::default().fg(scrollbar_color),
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

fn draw_input(app: &mut App, frame: &mut Frame, is_dimmed: bool) {
    // Dimmed input background when modal is open
    let input_bg = if is_dimmed { DIM_BG } else { Color::Black };
    let input_fg = if is_dimmed { DIM_TEXT } else { Color::White };

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
    let (cursor_x, cursor_y) = if app.active_llm_calls == 0 {
        cursor_position(&app.input, app.cursor_pos, available_width)
    } else {
        (0, 0)
    };

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

    if app.active_llm_calls == 0 {
        // Adjust cursor Y position based on scroll
        let visible_cursor_y = cursor_y.saturating_sub(app.input_scroll_offset);
        let cursor_pos = ratatui::layout::Position {
            x: inner.x + cursor_x as u16,
            y: inner.y + visible_cursor_y as u16,
        };
        frame.set_cursor_position(cursor_pos);
    }
}

fn colorize_input(input: &str, is_dimmed: bool) -> Vec<Span<'static>> {
    let dim_fg = if is_dimmed { DIM_TEXT } else { Color::White };
    let dim_at = if is_dimmed { DIM_AT } else { Color::Yellow };

    let mut spans = Vec::new();
    let mut chars = input.chars().peekable();
    let mut current = String::new();

    while let Some(ch) = chars.next() {
        if ch == '@' {
            // Push any accumulated text before the @
            if !current.is_empty() {
                spans.push(Span::styled(current.clone(), Style::default().fg(dim_fg)));
                current.clear();
            }

            // Collect the word after @
            let mut word = String::from("@");
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_alphanumeric()
                    || next_ch == '_'
                    || next_ch == '.'
                    || next_ch == '/'
                    || next_ch == '-'
                {
                    word.push(next_ch);
                    chars.next();
                } else {
                    break;
                }
            }

            // Add the @word in yellow (dimmed when modal is open)
            spans.push(Span::styled(word, Style::default().fg(dim_at)));
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

fn draw_control_panel(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();
    // Height varies based on content (base 8 + provider info + model info)
    let panel_height = 11_u16;
    let modal_x = (frame_area.width.saturating_sub(CONTROL_PANEL_WIDTH)) / 2;
    let modal_y = (frame_area.height.saturating_sub(panel_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: CONTROL_PANEL_WIDTH,
        height: panel_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::EMPTY)
        .title(" Control Panel ")
        .style(Style::default().bg(Color::Black));

    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    let mut lines = vec![
        Line::from(vec![Span::raw("Available Panels:")]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "[D] Debug - API Logs & Performance",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    // Show provider info and switch option
    lines.push(Line::from(""));
    let current_provider = app
        .active_provider
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("local");
    let current_model = app
        .active_provider
        .as_ref()
        .and_then(|p| p.default_model.as_ref())
        .map(|m| m.as_str())
        .unwrap_or("local");

    if app.providers.len() > 1 {
        lines.push(Line::from(vec![Span::styled(
            format!("[P] Switch Provider ({})", current_provider),
            Style::default().fg(Color::Yellow),
        )]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            format!("Provider: {}", current_provider),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    // Show current model
    lines.push(Line::from(vec![Span::styled(
        format!("Model: {}", current_model),
        Style::default().fg(Color::DarkGray),
    )]));

    let text = Paragraph::new(lines).style(Style::default().bg(Color::Black));
    frame.render_widget(block, modal_area);
    frame.render_widget(Clear, inner);
    frame.render_widget(text, inner);
}

fn draw_status(app: &App, frame: &mut Frame, area: Rect, is_dimmed: bool) {
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
    let percentage = if app.context_window > 0 {
        (tokens_used * 100) / app.context_window
    } else {
        0
    };
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

fn draw_file_picker(app: &App, frame: &mut Frame) {
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

            let inner = area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 1,
            });

            // Draw background
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Render visible entries
            let start_idx = if picker.selected > max_visible - 1 {
                picker.selected - max_visible + 1
            } else {
                0
            };
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

                let truncated = if entry.len() > inner.width as usize {
                    format!("{}...", &entry[..inner.width.saturating_sub(3) as usize])
                } else {
                    entry.clone()
                };
                lines.push(Line::from(Span::styled(truncated, style)));
            }

            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

fn draw_slash_picker(app: &App, frame: &mut Frame) {
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

            let inner = area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 1,
            });

            // Draw background
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Render visible entries
            let start_idx = if picker.selected > max_visible - 1 {
                picker.selected - max_visible + 1
            } else {
                0
            };
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

                    let truncated = if entry.len() > inner.width as usize {
                        format!("{}...", &entry[..inner.width.saturating_sub(3) as usize])
                    } else {
                        entry.clone()
                    };
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

fn draw_api_key_input(app: &App, frame: &mut Frame) {
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

            let inner = area.inner(ratatui::layout::Margin {
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

fn draw_bash_confirm(app: &App, frame: &mut Frame) {
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

            let inner = area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 1,
            });

            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            // Build content lines
            let mut lines = Vec::new();

            // Command line (truncate if too long)
            let cmd_display = if confirm.command.len() > (inner.width as usize).saturating_sub(2) {
                format!(
                    "{}...",
                    &confirm.command[..inner.width.saturating_sub(5) as usize]
                )
            } else {
                confirm.command.clone()
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", cmd_display),
                Style::default().fg(Color::Yellow),
            )));

            // Reason line (truncate if too long)
            let reason_display = if confirm.reason.len() > (inner.width as usize).saturating_sub(2)
            {
                format!(
                    "{}...",
                    &confirm.reason[..inner.width.saturating_sub(5) as usize]
                )
            } else {
                confirm.reason.clone()
            };
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

fn draw_debug_modal(app: &App, frame: &mut Frame) {
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
        .title_alignment(ratatui::layout::Alignment::Left)
        .style(Style::default().bg(Color::Black));

    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    if let Some(expanded_idx) = app.debug_expanded_row {
        // Expanded view: show full request details
        let filtered_logs = app.debug_filtered_logs();
        if let Some(log) = filtered_logs.get(expanded_idx) {
            let mut lines = Vec::new();

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

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Request Body:",
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(""));

            // Pretty-print the JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&log.request_body) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    for pretty_line in pretty.lines() {
                        lines.push(Line::from(Span::raw(pretty_line.to_string())));
                    }
                }
            } else {
                lines.push(Line::from(Span::raw(log.request_body.clone())));
            }

            // Show full response (accumulated content from LLM)
            if let Some(full_response) = &log.full_response {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Response Body:",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::raw(full_response.clone())));
            }

            // Show SSE events if available
            if let Some(response) = &log.response_body {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "SSE Events:",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(""));

                // Pretty-print response JSON - handle SSE format (multiple JSON blocks)
                for line in response.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("data: ") {
                        let json_str = trimmed.strip_prefix("data: ").unwrap_or(trimmed);
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
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
    } else {
        // List view: show all logs with selection highlight
        let filtered_logs = app.debug_filtered_logs();
        let mut lines = Vec::new();

        for (idx, log) in filtered_logs.iter().enumerate() {
            let is_selected = idx == app.debug_selected_row;
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
            let time_span = Span::styled(time_display, Style::default().fg(Color::White));

            let duration_ms = log.duration_ms.unwrap_or(0);
            let duration_str = format!("{:>6}ms", duration_ms);
            let duration_span = Span::raw(format!("  {}  ", duration_str));

            // Extract user message text from request body JSON
            let description =
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&log.request_body) {
                    if let Some(messages) = json.get("messages").and_then(|m| m.as_array()) {
                        // Find the last user message
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
                };

            let bg_style = if is_selected {
                Style::default().bg(Color::Rgb(50, 50, 50))
            } else {
                Style::default()
            };

            if let Some(ref err) = log.error_message {
                let error_text = format!("Error: {}", err);
                let mut spans = vec![
                    if is_selected {
                        Span::styled(
                            "✗",
                            Style::default().fg(Color::Red).bg(Color::Rgb(50, 50, 50)),
                        )
                    } else {
                        status_icon.clone()
                    },
                    Span::styled("  ", bg_style),
                    Span::styled(
                        time_span.content.clone(),
                        if is_selected {
                            Style::default().fg(Color::White).bg(Color::Rgb(50, 50, 50))
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
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
                    if is_selected {
                        Span::styled(
                            "✓",
                            Style::default().fg(Color::Green).bg(Color::Rgb(50, 50, 50)),
                        )
                    } else {
                        status_icon.clone()
                    },
                    Span::styled("  ", bg_style),
                    Span::styled(
                        time_span.content.clone(),
                        if is_selected {
                            Style::default().fg(Color::White).bg(Color::Rgb(50, 50, 50))
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
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
}
