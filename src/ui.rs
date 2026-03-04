use crate::app::App;
use crate::text::wrap_text;
use crate::ui::confirmations::{draw_ask_question, draw_bash_confirm};
use crate::ui::input::draw_input;
use crate::ui::layout::*;
use crate::ui::messages::draw_messages;
use crate::ui::pickers::{draw_api_key_input, draw_file_picker, draw_slash_picker};
use crate::ui::status::draw_status;
use ratatui::Frame;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub mod colors;
pub mod confirmations;
pub mod input;
pub mod layout;
pub mod messages;
pub mod pickers;
pub mod status;

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
    draw_input(
        app,
        frame,
        is_modal_open,
        app.pending_ask_question.is_some(),
    );
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

    // Draw ask question prompt if active
    if app.pending_ask_question.is_some() {
        draw_ask_question(app, frame);
    }
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
