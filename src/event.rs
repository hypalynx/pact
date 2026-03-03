use crate::app::{App, DEFAULT_MAX_TOKENS, PanelState, SlashCommand};
use crate::llm::{LlmEvent, Message, ToolCallInfo};
use crate::tools;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

/// Handle incoming LLM events (tokens, done, errors, etc.)
pub fn handle_llm_event(app: &mut App, event: LlmEvent) {
    match event {
        LlmEvent::Token(t, _call_id) => {
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
        }
        LlmEvent::Thinking(t, _call_id) => {
            app.pending_thinking.push_str(&t);
        }
        LlmEvent::Done(_call_id) => {
            let mut text = std::mem::take(&mut app.pending_response);
            let thinking = if app.pending_thinking.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut app.pending_thinking))
            };

            // Check if we hit the token limit and append warning
            if app.last_output_tokens >= DEFAULT_MAX_TOKENS {
                text.push_str("\n\n[Response truncated: reached max token limit (");
                text.push_str(&DEFAULT_MAX_TOKENS.to_string());
                text.push_str(" tokens)]");
            }
            app.last_output_tokens = 0;

            // Only create a message if there's actual content
            if !text.is_empty() || thinking.is_some() {
                let msg = Message {
                    role: "assistant".to_string(),
                    text,
                    is_tool_result: false,
                    thinking,
                    tool_result_content: None,
                    tool_call_id: None,
                    tool_name: None,
                    tool_calls: None,
                };
                app.messages.push(msg.clone());
                if let Some(db) = &app.db {
                    let _ = db.save_message(&msg, &app.session_id, &app.working_directory);
                }
            }
            app.active_llm_calls = app.active_llm_calls.saturating_sub(1);
            app.active_call_id = None;
        }
        LlmEvent::Error(e, _call_id) => {
            let text = format!("Error: {}", e);
            app.messages.push(Message {
                role: "assistant".to_string(),
                text,
                is_tool_result: false,
                thinking: None,
                tool_result_content: None,
                tool_call_id: None,
                tool_name: None,
                tool_calls: None,
            });
            app.active_llm_calls = app.active_llm_calls.saturating_sub(1);
            app.active_call_id = None;

            // Show error in status bar for 5 seconds (300 frames at 60fps)
            app.error_message = Some(format!("Error: {}", &e[..e.len().min(50)]));
            app.last_error_frame = app.frame_count;
        }
        LlmEvent::Usage {
            input_tokens,
            output_tokens,
            call_id: _,
        } => {
            app.total_input_tokens += input_tokens;
            app.total_output_tokens += output_tokens;
            app.last_output_tokens = output_tokens;
        }
        LlmEvent::ToolCall {
            id,
            name,
            args,
            call_id: _,
        } => {
            // Build tool_call metadata for the assistant message
            let tc_info = ToolCallInfo {
                id: id.clone(),
                name: name.clone(),
                arguments: serde_json::to_string(&args).unwrap_or_default(),
            };

            // Attach tool_calls to assistant message:
            // If last message is already an assistant with tool_calls (parallel calls), append
            // Otherwise save pending text as a new assistant message with this tool_call
            let appended = if let Some(last) = app.messages.last_mut() {
                if let Some(ref mut tcs) = last.tool_calls {
                    if last.role == "assistant" {
                        tcs.push(tc_info.clone());
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !appended {
                let text = std::mem::take(&mut app.pending_response)
                    .trim_end()
                    .to_string();
                let thinking = if app.pending_thinking.is_empty() {
                    None
                } else {
                    Some(
                        std::mem::take(&mut app.pending_thinking)
                            .trim_end()
                            .to_string(),
                    )
                };
                let msg = Message {
                    role: "assistant".to_string(),
                    text,
                    is_tool_result: false,
                    thinking,
                    tool_result_content: None,
                    tool_call_id: None,
                    tool_name: None,
                    tool_calls: Some(vec![tc_info]),
                };
                app.messages.push(msg.clone());
                if let Some(db) = &app.db {
                    let _ = db.save_message(&msg, &app.session_id, &app.working_directory);
                }
            }

            app.pending_tool_count += 1;

            // Check if this is a Bash tool call that might be dangerous
            if name == "Bash"
                && let Some(command) = args.get("command").and_then(|v| v.as_str())
            {
                match tools::validate_bash_command(command) {
                    tools::ValidationResult::HardBlocked(reason) => {
                        let error = format!("Command blocked for safety: {}", reason);
                        let _ = app.tx.send(LlmEvent::ToolResult {
                            tool_name: name,
                            tool_call_id: id,
                            summary: "Command blocked".to_string(),
                            content: error,
                            call_id: 0,
                        });
                        return;
                    }
                    tools::ValidationResult::SoftBlocked(reason) => {
                        // Pause and wait for user confirmation
                        app.pending_bash_confirm = Some(crate::app::PendingBashConfirm {
                            tool_id: id,
                            command: command.to_string(),
                            reason,
                            args,
                        });
                        return;
                    }
                    tools::ValidationResult::Safe => {}
                }
            }

            // Plan mode: Write/Edit restricted to .md files
            if app.mode_name == "plan" && (name == "Write" || name == "Edit") {
                let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                if !path.ends_with(".md") {
                    let error = format!(
                        "Blocked in plan mode: {} is only allowed for .md files. \
                         To modify source code, the user must press Tab to switch to build mode.",
                        name
                    );
                    let _ = app.tx.send(LlmEvent::ToolResult {
                        tool_name: name,
                        tool_call_id: id,
                        summary: "Tool blocked".to_string(),
                        content: error,
                        call_id: 0,
                    });
                    return;
                }
            }

            // Execute the tool in a background thread (don't block UI on I/O)
            let tool_call = tools::ToolCall {
                name: name.clone(),
                args,
            };
            let tx = app.tx.clone();
            let tool_name = name.clone();
            let tool_call_id = id.clone();
            std::thread::spawn(move || {
                let (summary, content) = tools::execute_tool(&tool_call);
                let _ = tx.send(LlmEvent::ToolResult {
                    tool_name,
                    tool_call_id,
                    summary,
                    content,
                    call_id: 0,
                });
            });
        }

        LlmEvent::ApiLog {
            request_body,
            response_body,
            full_response,
            duration_ms,
            error_message,
            model_name,
            provider,
            call_id: _,
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
            call_id: _,
        } => {
            app.model_name = model_name;
            app.context_window = context_window;
        }
        LlmEvent::ToolResult {
            tool_name,
            tool_call_id,
            summary,
            content,
            call_id: _,
        } => {
            let result_msg = Message {
                role: "user".to_string(),
                text: summary,
                is_tool_result: true,
                thinking: None,
                tool_result_content: Some(content),
                tool_call_id: Some(tool_call_id),
                tool_name: Some(tool_name),
                tool_calls: None,
            };
            app.messages.push(result_msg.clone());
            if let Some(db) = &app.db {
                let _ = db.save_message(&result_msg, &app.session_id, &app.working_directory);
            }
            app.pending_tool_count = app.pending_tool_count.saturating_sub(1);
        }
        LlmEvent::ModelsLoaded { models } => {
            // Update the model picker with fetched models
            if let Some(picker) = &mut app.slash_picker
                && picker.command == crate::app::SlashCommand::Model
            {
                picker.all_entries = models;
                app.slash_picker_update_filter();
            }
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

    // Handle bash confirmation mode
    if app.pending_bash_confirm.is_some() {
        return handle_bash_confirm_key(app, key);
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
        KeyCode::Tab => {
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

/// Handle bash confirmation specific keys
fn handle_bash_confirm_key(app: &mut App, key: KeyEvent) -> bool {
    let confirm = match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(true),
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(false),
        _ => None,
    };

    if let Some(approved) = confirm {
        if let Some(pending) = app.pending_bash_confirm.take() {
            if approved {
                // User approved - execute the command
                let (summary, content) = tools::execute_bash_unchecked(&pending.command, None);
                let result_msg = Message {
                    role: "user".to_string(),
                    text: summary,
                    is_tool_result: true,
                    thinking: None,
                    tool_result_content: Some(content),
                    tool_call_id: Some(pending.tool_id),
                    tool_name: Some("Bash".to_string()),
                    tool_calls: None,
                };
                app.messages.push(result_msg.clone());
                if let Some(db) = &app.db {
                    let _ = db.save_message(&result_msg, &app.session_id, &app.working_directory);
                }
            } else {
                // User denied - send denial message
                let result_msg = Message {
                    role: "user".to_string(),
                    text: "Command denied".to_string(),
                    is_tool_result: true,
                    thinking: None,
                    tool_result_content: Some("Command denied by user.".to_string()),
                    tool_call_id: Some(pending.tool_id),
                    tool_name: Some("Bash".to_string()),
                    tool_calls: None,
                };
                app.messages.push(result_msg.clone());
                if let Some(db) = &app.db {
                    let _ = db.save_message(&result_msg, &app.session_id, &app.working_directory);
                }
            }

            app.pending_tool_count = app.pending_tool_count.saturating_sub(1);
        }
        return true;
    }

    true
}

/// Handle mouse events
pub fn handle_mouse_event(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.scroll_up();
        }
        MouseEventKind::ScrollDown => {
            app.scroll_down();
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
