use crate::app::{App, PanelState};
use crate::llm::{LlmEvent, Message};
use crate::tools;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

/// Handle incoming LLM events (tokens, done, errors, etc.)
pub fn handle_llm_event(app: &mut App, event: LlmEvent) {
    match event {
        LlmEvent::Token(t) => {
            app.pending_response.push_str(&t);
            if !app.user_scrolled {
                let new_line_count = app.calculate_total_lines();
                let height = app.messages_rect.height as usize;
                app.scroll_offset = new_line_count.saturating_sub(height);
            }
        }
        LlmEvent::Thinking(t) => {
            app.pending_thinking.push_str(&t);
            if !app.user_scrolled {
                let new_line_count = app.calculate_total_lines();
                let height = app.messages_rect.height as usize;
                app.scroll_offset = new_line_count.saturating_sub(height);
            }
        }
        LlmEvent::Done => {
            let text = std::mem::take(&mut app.pending_response);
            let thinking = if app.pending_thinking.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut app.pending_thinking))
            };
            let msg = Message {
                role: "assistant".to_string(),
                text,
                is_tool_result: false,
                thinking,
                tool_result_content: None,
            };
            app.messages.push(msg.clone());
            if let Some(db) = &app.db {
                let _ = db.save_message(&msg);
            }
            app.loading = false;
            app.progress = None;
            if !app.user_scrolled {
                let new_line_count = app.calculate_total_lines();
                let height = app.messages_rect.height as usize;
                app.scroll_offset = new_line_count.saturating_sub(height);
            }
        }
        LlmEvent::Error(e) => {
            let text = format!("Error: {}", e);
            app.messages.push(Message {
                role: "assistant".to_string(),
                text,
                is_tool_result: false,
                thinking: None,
                tool_result_content: None,
            });
            app.loading = false;
            app.progress = None;
            if !app.user_scrolled {
                let new_line_count = app.calculate_total_lines();
                let height = app.messages_rect.height as usize;
                app.scroll_offset = new_line_count.saturating_sub(height);
            }
        }
        LlmEvent::Usage {
            input_tokens,
            output_tokens,
        } => {
            app.total_input_tokens += input_tokens;
            app.total_output_tokens += output_tokens;
        }
        LlmEvent::ToolCall { name, args } => {
            let tool_call = tools::ToolCall { name, args };
            let (summary, content) = tools::execute_tool(&tool_call);
            app.messages.push(Message {
                role: "user".to_string(),
                text: summary,
                is_tool_result: true,
                thinking: None,
                tool_result_content: Some(content),
            });
            app.send_to_llm();
        }
        LlmEvent::Progress(p) => {
            app.progress = Some(p);
        }
        LlmEvent::ApiLog {
            request_body,
            response_body,
            full_response,
            duration_ms,
            error_message,
        } => {
            if let Some(db) = &app.db {
                let _ = db.save_api_log(
                    &request_body,
                    response_body.as_deref(),
                    full_response.as_deref(),
                    duration_ms,
                    error_message.as_deref(),
                );
            }
        }
    }
}

/// Handle keyboard events
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> bool {
    // Ctrl+G toggles control panel
    if let KeyCode::Char('g') = key.code
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        app.panel_state = match app.panel_state {
            PanelState::None => PanelState::ControlPanel,
            _ => PanelState::None,
        };
        return true;
    }

    // Handle panel-specific keys
    match app.panel_state {
        PanelState::ControlPanel => {
            handle_control_panel_key(app, key);
            return true;
        }
        PanelState::Debug => {
            handle_debug_panel_key(app, key);
            return true;
        }
        PanelState::None => {}
    }

    // Handle main input keys
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.input.is_empty() {
                return false; // Signal to exit
            } else {
                app.input.clear();
                app.cursor_pos = 0;
                app.history_index = None;
            }
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_to_start();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_to_end();
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.kill_word_backward();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.kill_line();
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_forward();
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_backward();
        }
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.cycle_mode();
        }
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_char('\n');
        }
        KeyCode::Enter => {
            if !app.input.trim().is_empty() {
                app.submit_message();
            }
        }
        KeyCode::Backspace => {
            app.delete_char();
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
        }
        KeyCode::Up => {
            app.history_up();
        }
        KeyCode::Down => {
            app.history_down();
        }
        KeyCode::PageUp => {
            app.scroll_up();
        }
        KeyCode::PageDown => {
            app.scroll_down();
        }
        KeyCode::Left => {
            app.move_cursor_backward();
        }
        KeyCode::Right => {
            app.move_cursor_forward();
        }
        KeyCode::Delete => {
            app.delete_char();
        }
        _ => {}
    }

    true
}

/// Handle control panel specific keys
fn handle_control_panel_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.panel_state = PanelState::None,
        KeyCode::Char('d') | KeyCode::Char('D') => {
            app.panel_state = PanelState::Debug;
            app.refresh_debug_logs();
            app.debug_scroll = 0;
        }
        _ => {}
    }
}

/// Handle debug panel specific keys
fn handle_debug_panel_key(app: &mut App, key: KeyEvent) {
    if app.debug_expanded_row.is_some() {
        // Expanded view key handling
        match key.code {
            KeyCode::Esc => {
                app.debug_expanded_row = None;
                app.debug_expand_scroll = 0;
                app.debug_expand_scroll_x = 0;
            }
            KeyCode::Up | KeyCode::PageUp => {
                app.debug_expand_scroll = app.debug_expand_scroll.saturating_sub(3);
            }
            KeyCode::Down | KeyCode::PageDown => {
                app.debug_expand_scroll += 3;
            }
            KeyCode::Left => {
                app.debug_expand_scroll_x = app.debug_expand_scroll_x.saturating_sub(5);
            }
            KeyCode::Right => {
                app.debug_expand_scroll_x += 5;
            }
            _ => {}
        }
    } else {
        // List view key handling
        match key.code {
            KeyCode::Esc => app.panel_state = PanelState::ControlPanel,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.panel_state = PanelState::ControlPanel
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                app.debug_filter_errors = !app.debug_filter_errors;
                app.debug_selected_row = 0;
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if let Some(db) = &app.db {
                    let _ = db.clear_api_logs();
                    app.refresh_debug_logs();
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(db) = &app.db {
                    let _ = db.clear_messages();
                    app.messages.clear();
                    app.history.clear();
                }
            }
            KeyCode::Up => {
                app.debug_selected_row = app.debug_selected_row.saturating_sub(1);
                if app.debug_selected_row < app.debug_scroll {
                    app.debug_scroll = app.debug_selected_row;
                }
            }
            KeyCode::Down => {
                let filtered_logs = app.debug_filtered_logs();
                if app.debug_selected_row < filtered_logs.len().saturating_sub(1) {
                    app.debug_selected_row += 1;
                    let visible_height = 10;
                    if app.debug_selected_row >= app.debug_scroll + visible_height {
                        app.debug_scroll = app.debug_selected_row - visible_height + 1;
                    }
                }
            }
            KeyCode::PageUp => {
                app.debug_selected_row = app.debug_selected_row.saturating_sub(5);
                if app.debug_selected_row < app.debug_scroll {
                    app.debug_scroll = app.debug_selected_row;
                }
            }
            KeyCode::PageDown => {
                let filtered_logs = app.debug_filtered_logs();
                if app.debug_selected_row < filtered_logs.len().saturating_sub(1) {
                    app.debug_selected_row =
                        (app.debug_selected_row + 5).min(filtered_logs.len().saturating_sub(1));
                    let visible_height = 10;
                    if app.debug_selected_row >= app.debug_scroll + visible_height {
                        app.debug_scroll = app.debug_selected_row - visible_height + 1;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.toggle_debug_row_expand(app.debug_selected_row);
            }
            _ => {}
        }
    }
}

/// Handle mouse events
pub fn handle_mouse_event(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.scroll_up();
            app.user_scrolled = true;
        }
        MouseEventKind::ScrollDown => {
            app.scroll_down();
            app.user_scrolled = true;
        }
        MouseEventKind::Down(_) => {
            let scrollbar_x = app.messages_rect.x + app.messages_rect.width.saturating_sub(1);
            if mouse.column == scrollbar_x
                && mouse.row >= app.messages_rect.y
                && mouse.row < app.messages_rect.y + app.messages_rect.height
            {
                app.handle_scrollbar_click(mouse.row);
                app.dragging_scrollbar = true;
            } else if mouse.row >= app.messages_rect.y
                && mouse.row < app.messages_rect.y + app.messages_rect.height
            {
                app.start_selection(mouse.column, mouse.row);
            }
        }
        MouseEventKind::Drag(_) if app.dragging_scrollbar => {
            app.handle_scrollbar_click(mouse.row);
        }
        MouseEventKind::Drag(_) => {
            if app.selection_start.is_some() {
                app.extend_selection(mouse.column, mouse.row);
            }
        }
        MouseEventKind::Up(_) => {
            app.dragging_scrollbar = false;
            app.finish_selection();
        }
        _ => {}
    }
}
