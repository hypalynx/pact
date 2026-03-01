use crate::app::{App, PanelState, SlashCommand};
use crate::llm::{LlmEvent, Message};
use crate::tools;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

/// Handle incoming LLM events (tokens, done, errors, etc.)
pub fn handle_llm_event(app: &mut App, event: LlmEvent) {
    match event {
        LlmEvent::Token(t) => {
            // Check if at bottom BEFORE adding content
            let was_at_bottom = if app.messages_rect.height > 0 {
                let (at_bottom, _) = app.calculate_scroll_info();
                at_bottom
            } else {
                // Startup: viewport not sized yet, assume at bottom
                true
            };

            // Add content - trim leading whitespace from first token of new response
            // to avoid gaps after tool calls (e.g., "\n\nHere's the answer...")
            if app.pending_response.is_empty() {
                let trimmed = t.trim_start();
                if !trimmed.is_empty() {
                    app.pending_response.push_str(trimmed);
                }
            } else {
                app.pending_response.push_str(&t);
            }

            // Auto-scroll to bottom if we were at bottom before content arrived
            if was_at_bottom {
                let total_lines = app.calculate_total_lines();
                app.scroll_offset = total_lines.saturating_sub(app.messages_rect.height as usize);
            }
        }
        LlmEvent::Thinking(t) => {
            // Check if at bottom BEFORE adding content
            let was_at_bottom = if app.messages_rect.height > 0 {
                let (at_bottom, _) = app.calculate_scroll_info();
                at_bottom
            } else {
                // Startup: viewport not sized yet, assume at bottom
                true
            };

            // Add content
            app.pending_thinking.push_str(&t);

            // Auto-scroll to bottom if we were at bottom before content arrived
            if was_at_bottom {
                let total_lines = app.calculate_total_lines();
                app.scroll_offset = total_lines.saturating_sub(app.messages_rect.height as usize);
            }
        }
        LlmEvent::Done => {
            // Check if at bottom BEFORE adding message
            let was_at_bottom = if app.messages_rect.height > 0 {
                let (at_bottom, _) = app.calculate_scroll_info();
                at_bottom
            } else {
                // Startup: viewport not sized yet, assume at bottom
                true
            };

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
                tool_call_id: None,
                tool_name: None,
            };
            app.messages.push(msg.clone());
            if let Some(db) = &app.db {
                let _ = db.save_message(&msg);
            }
            app.active_llm_calls = app.active_llm_calls.saturating_sub(1);

            // Auto-scroll to bottom if we were at bottom before message added
            if was_at_bottom {
                let total_lines = app.calculate_total_lines();
                app.scroll_offset = total_lines.saturating_sub(app.messages_rect.height as usize);
            }
        }
        LlmEvent::Error(e) => {
            // Check if at bottom BEFORE adding message
            let was_at_bottom = if app.messages_rect.height > 0 {
                let (at_bottom, _) = app.calculate_scroll_info();
                at_bottom
            } else {
                // Startup: viewport not sized yet, assume at bottom
                true
            };

            let text = format!("Error: {}", e);
            app.messages.push(Message {
                role: "assistant".to_string(),
                text,
                is_tool_result: false,
                thinking: None,
                tool_result_content: None,
                tool_call_id: None,
                tool_name: None,
            });
            app.active_llm_calls = app.active_llm_calls.saturating_sub(1);

            // Show error in status bar for 5 seconds (300 frames at 60fps)
            app.error_message = Some(format!("Error: {}", &e[..e.len().min(50)]));
            app.last_error_frame = app.frame_count;

            // Auto-scroll to bottom if we were at bottom before message added
            if was_at_bottom {
                let total_lines = app.calculate_total_lines();
                app.scroll_offset = total_lines.saturating_sub(app.messages_rect.height as usize);
            }
        }
        LlmEvent::Usage {
            input_tokens,
            output_tokens,
        } => {
            app.total_input_tokens += input_tokens;
            app.total_output_tokens += output_tokens;
        }
        LlmEvent::ToolCall { id, name, args } => {
            // Check if at bottom BEFORE adding content
            let was_at_bottom = if app.messages_rect.height > 0 {
                let (at_bottom, _) = app.calculate_scroll_info();
                at_bottom
            } else {
                // Startup: viewport not sized yet, assume at bottom
                true
            };

            // First, save any pending thinking/response as an assistant message
            // This preserves the LLM's thought process before the tool call
            // Trim trailing newlines to avoid gaps where the <tool_call> was stripped
            let text = std::mem::take(&mut app.pending_response);
            let text = text.trim_end().to_string();
            let thinking = if app.pending_thinking.is_empty() {
                None
            } else {
                Some(
                    std::mem::take(&mut app.pending_thinking)
                        .trim_end()
                        .to_string(),
                )
            };
            if !text.is_empty() || thinking.is_some() {
                let msg = Message {
                    role: "assistant".to_string(),
                    text,
                    is_tool_result: false,
                    thinking,
                    tool_result_content: None,
                    tool_call_id: None,
                    tool_name: None,
                };
                app.messages.push(msg.clone());
                if let Some(db) = &app.db {
                    let _ = db.save_message(&msg);
                }
            }

            // Execute the tool and add result
            let tool_call = tools::ToolCall {
                name: name.clone(),
                args,
            };
            let (summary, content) = tools::execute_tool(&tool_call);
            let result_msg = Message {
                role: "user".to_string(),
                text: summary,
                is_tool_result: true,
                thinking: None,
                tool_result_content: Some(content),
                tool_call_id: Some(id),
                tool_name: Some(name),
            };
            app.messages.push(result_msg.clone());
            if let Some(db) = &app.db {
                let _ = db.save_message(&result_msg);
            }

            // Auto-scroll to bottom if we were at bottom before tool result was added
            if was_at_bottom {
                let total_lines = app.calculate_total_lines();
                app.scroll_offset = total_lines.saturating_sub(app.messages_rect.height as usize);
            }

            app.send_to_llm();
        }

        LlmEvent::ApiLog {
            request_body,
            response_body,
            full_response,
            duration_ms,
            error_message,
            model_name,
            provider,
        } =>
        {
            #[allow(clippy::collapsible_if)]
            if let Some(db) = &app.db {
                if let Err(e) = db.save_api_log(
                    &request_body,
                    response_body.as_deref(),
                    full_response.as_deref(),
                    duration_ms,
                    error_message.as_deref(),
                    model_name.as_deref(),
                    provider.as_deref(),
                ) {
                    app.set_error(format!("DB error: {}", e));
                }
            }
        }
        LlmEvent::ServerInfo {
            model_name,
            context_window,
        } => {
            app.model_name = model_name;
            app.context_window = context_window;
        }
    }
}

/// Handle keyboard events
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> bool {
    // Reset exit confirmation on any keypress other than Ctrl+C
    if !(matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL)) {
        app.reset_exit_confirmation();
    }

    // Reset cancel confirmation on any keypress other than Esc
    if !matches!(key.code, KeyCode::Esc) {
        app.reset_cancel_confirmation();
    }

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

    // Handle file picker keys if picker is open
    if app.file_picker.is_some() {
        return handle_file_picker_key(app, key);
    }

    // Handle slash picker keys if picker is open
    if app.slash_picker.is_some() {
        return handle_slash_picker_key(app, key);
    }

    // Handle API key input mode
    if app.api_key_input.is_some() {
        return handle_api_key_input_key(app, key);
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
        KeyCode::Esc => {
            if app.input.is_empty() && app.active_llm_calls > 0 {
                if app.is_cancel_confirming() {
                    // Second ESC press - actually cancel
                    app.cancel_current_call();
                    app.reset_cancel_confirmation();
                } else {
                    // First ESC press - show confirmation in status bar
                    app.set_cancel_confirmation();
                }
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.input.is_empty() {
                if app.is_exit_confirming() {
                    return false; // Second Ctrl+C - actually exit
                } else {
                    app.set_exit_confirmation(); // First Ctrl+C - show confirmation
                }
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
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.history_down();
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.history_up();
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
            // Handle special input modes first
            if app.slash_picker.is_some() {
                // Don't insert into main input - just update picker query
                app.slash_picker_type(c);
            } else if app.api_key_input.is_some() {
                app.handle_api_key_input(c);
            } else if c == '@' {
                app.insert_char(c);
                app.start_file_picker();
            } else if c == '/' {
                app.insert_char(c);
                // Check if this is the start of a slash command
                let is_at_start = app.cursor_pos == 1;
                let is_after_space = app.cursor_pos > 1
                    && app.input.chars().nth(app.cursor_pos.saturating_sub(2)) == Some(' ');
                if is_at_start || is_after_space {
                    // Show command help immediately
                    app.start_slash_command_help();
                }
            } else {
                app.insert_char(c);
                // Check if we're building a slash command
                if let Some(picker) = &app.slash_picker {
                    let slash_start = picker.slash_start;
                    let current_text = &app.input[slash_start..app.cursor_pos];

                    // Detect which command based on what was typed
                    if current_text == "/model" || current_text.starts_with("/model ") {
                        // User typed "/model" - show model picker
                        if picker.all_entries.is_empty() {
                            app.slash_picker = None;
                            let query = current_text
                                .strip_prefix("/model")
                                .unwrap_or("")
                                .trim_start()
                                .to_string();
                            app.start_slash_picker(SlashCommand::Model, &query);
                        }
                    } else if current_text == "/connect" || current_text.starts_with("/connect ") {
                        // User typed "/connect" - switch to connect mode
                        if picker.all_entries.is_empty() {
                            app.slash_picker = None;
                            app.start_slash_picker(SlashCommand::Connect, "");
                        }
                    } else if !current_text.starts_with("/model")
                        && !current_text.starts_with("/connect")
                    {
                        // Not a recognized slash command, cancel the picker
                        app.slash_picker = None;
                    }
                }
            }
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
        KeyCode::Char('p') | KeyCode::Char('P') => {
            app.cycle_provider();
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

/// Handle file picker specific keys
fn handle_file_picker_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Up => {
            app.file_picker_up();
        }
        KeyCode::Down => {
            app.file_picker_down();
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.file_picker_up();
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.file_picker_down();
        }
        KeyCode::Enter | KeyCode::Tab => {
            app.file_picker_select();
        }
        KeyCode::Esc | KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Remove @ from input and close picker
            if let Some(picker) = &app.file_picker {
                let at_start = picker.at_start;
                app.file_picker = None;
                if at_start < app.input.len() {
                    app.input.drain(at_start..app.cursor_pos);
                    app.cursor_pos = at_start;
                }
            }
        }
        KeyCode::Backspace => {
            if !app.file_picker_backspace() {
                // Backspaced past @, close picker and remove @
                if let Some(picker) = &app.file_picker {
                    let at_start = picker.at_start;
                    app.file_picker = None;
                    if at_start < app.input.len() {
                        app.input.drain(at_start..app.cursor_pos);
                        app.cursor_pos = at_start;
                    }
                }
            }
        }
        KeyCode::Char(c) => {
            app.file_picker_type(c);
        }
        _ => {}
    }
    true
}

/// Handle slash picker specific keys
fn handle_slash_picker_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Up => {
            app.slash_picker_up();
        }
        KeyCode::Down => {
            app.slash_picker_down();
        }
        KeyCode::Enter | KeyCode::Tab => {
            app.slash_picker_select();
        }
        KeyCode::Esc | KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Remove slash command from input and close picker
            if let Some(picker) = &app.slash_picker {
                let slash_start = picker.slash_start;
                app.slash_picker = None;
                if slash_start < app.input.len() {
                    app.input.drain(slash_start..app.cursor_pos);
                    app.cursor_pos = slash_start;
                }
            }
        }
        KeyCode::Backspace => {
            if !app.slash_picker_backspace() {
                // Backspaced past start, close picker
                if let Some(picker) = &app.slash_picker {
                    let slash_start = picker.slash_start;
                    app.slash_picker = None;
                    if slash_start < app.input.len() {
                        app.input.drain(slash_start..app.cursor_pos);
                        app.cursor_pos = slash_start;
                    }
                }
            }
        }
        KeyCode::Char(c) => {
            app.slash_picker_type(c);
        }
        _ => {}
    }
    true
}

/// Handle API key input specific keys
fn handle_api_key_input_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Enter => {
            app.submit_api_key();
        }
        KeyCode::Esc => {
            app.api_key_input = None;
        }
        KeyCode::Backspace => {
            if !app.handle_api_key_backspace() {
                app.api_key_input = None;
            }
        }
        KeyCode::Char(c) => {
            app.handle_api_key_input(c);
        }
        _ => {}
    }
    true
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
