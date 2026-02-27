#![deny(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::dbg_macro,
    // Note: eprintln! will fail the build with this deny
)]

mod app;
mod config;
mod db;
mod llm;
mod text;
mod tools;
mod ui;
mod utils;

use app::App;
use clap::Parser;
use config::Config;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use crossterm::execute;
use std::io::stdout;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "pact")]
struct Args {
    #[arg(long)]
    debug: bool,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let config = Config::load();
    let mut terminal = ratatui::init();

    execute!(stdout(), EnableMouseCapture)?;

    let default_mode_config = config.ui.modes.get(&config.ui.default_mode).cloned();
    let temperature = default_mode_config.as_ref().and_then(|m| m.temperature);
    let modes_config = config.ui.modes.clone();

    let server_info = utils::fetch_server_info(&config.api.endpoint);

    let mut app = App::new(
        args.debug,
        config.api.endpoint.clone(),
        config.api.max_tokens,
        temperature,
        config.ui.default_mode.clone(),
        modes_config,
    );
    app.history = App::load_history().unwrap_or_default();
    app.context_window = server_info.context_window;
    app.model_name = server_info.model_name;

    loop {
        terminal.draw(|f| ui::draw_app(&mut app, f))?;

        while let Ok(event) = app.rx.try_recv() {
            match event {
                llm::LlmEvent::Token(t) => {
                    app.pending_response.push_str(&t);
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                llm::LlmEvent::Thinking(t) => {
                    // Accumulate thinking tokens separately
                    app.pending_thinking.push_str(&t);
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                llm::LlmEvent::Done => {
                    let text = std::mem::take(&mut app.pending_response);
                    let thinking = if app.pending_thinking.is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut app.pending_thinking))
                    };
                    let msg = llm::Message {
                        role: "assistant".to_string(),
                        text,
                        is_tool_result: false,
                        thinking,
                    };
                    app.messages.push(msg.clone());
                    // Save assistant message to database if available
                    if let Some(db) = &app.db {
                        let _ = db.save_message(&msg);
                    }
                    app.loading = false;
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                llm::LlmEvent::Error(e) => {
                    let text = format!("Error: {}", e);
                    app.messages.push(llm::Message {
                        role: "assistant".to_string(),
                        text,
                        is_tool_result: false,
                        thinking: None,
                    });
                    app.loading = false;
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                llm::LlmEvent::Usage {
                    input_tokens,
                    output_tokens,
                } => {
                    app.total_input_tokens += input_tokens;
                    app.total_output_tokens += output_tokens;
                }
                llm::LlmEvent::ToolCall { name, args } => {
                    let tool_call = tools::ToolCall { name, args };
                    let result = tools::execute_tool(&tool_call);
                    app.messages.push(llm::Message {
                        role: "user".to_string(),
                        text: result,
                        is_tool_result: true,
                        thinking: None,
                    });
                    app.send_to_llm();
                }
                llm::LlmEvent::ApiLog {
                    request_body,
                    duration_ms,
                    error_message,
                } => {
                    if let Some(db) = &app.db {
                        let _ =
                            db.save_api_log(&request_body, duration_ms, error_message.as_deref());
                    }
                }
            }
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('c')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            if app.input.is_empty() {
                                break;
                            } else {
                                app.input.clear();
                                app.cursor_pos = 0;
                                app.history_index = None;
                            }
                        }
                        KeyCode::Char('a')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.move_cursor_to_start();
                        }
                        KeyCode::Char('e')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.move_cursor_to_end();
                        }
                        KeyCode::Char('w')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.kill_word_backward();
                        }
                        KeyCode::Char('u')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.kill_line();
                        }
                        KeyCode::Char('f')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.move_cursor_forward();
                        }
                        KeyCode::Char('b')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.move_cursor_backward();
                        }
                        KeyCode::Char('j')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            app.insert_char('\n');
                        }
                        KeyCode::Char(c) => app.insert_char(c),
                        KeyCode::Backspace => {
                            app.delete_char();
                        }
                        KeyCode::Left => {
                            app.move_cursor_backward();
                        }
                        KeyCode::Right => {
                            app.move_cursor_forward();
                        }
                        KeyCode::Enter => {
                            app.submit_message();
                        }
                        KeyCode::Tab => {
                            app.cycle_mode();
                        }
                        KeyCode::Up => app.history_up(),
                        KeyCode::Down => app.history_down(),
                        KeyCode::PageUp => app.scroll_up(),
                        KeyCode::PageDown => app.scroll_down(),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => app.scroll_up(),
                        MouseEventKind::ScrollDown => app.scroll_down(),
                        MouseEventKind::Down(_) => {
                            let scrollbar_x =
                                app.messages_rect.x + app.messages_rect.width.saturating_sub(1);
                            if mouse.column == scrollbar_x
                                && mouse.row >= app.messages_rect.y
                                && mouse.row < app.messages_rect.y + app.messages_rect.height
                            {
                                app.handle_scrollbar_click(mouse.row);
                                app.dragging_scrollbar = true;
                            } else if mouse.row >= app.messages_rect.y
                                && mouse.row < app.messages_rect.y + app.messages_rect.height
                            {
                                // Start selection in message area
                                app.start_selection(mouse.column, mouse.row);
                            }
                        }
                        MouseEventKind::Drag(_) if app.dragging_scrollbar => {
                            app.handle_scrollbar_click(mouse.row);
                        }
                        MouseEventKind::Drag(_) => {
                            // Extend selection
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
                _ => {}
            }
        }

        app.frame_count = app.frame_count.wrapping_add(1);
        app.check_server_info();
    }

    execute!(stdout(), DisableMouseCapture)?;
    ratatui::restore();
    Ok(())
}
