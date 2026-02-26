mod app;
mod config;
mod llm;
mod text;
mod ui;
mod utils;

use app::App;
use clap::Parser;
use config::Config;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind, EnableMouseCapture, DisableMouseCapture};
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
                llm::LlmEvent::Done => {
                    let text = std::mem::take(&mut app.pending_response);
                    app.messages.push(llm::Message {
                        role: "assistant".to_string(),
                        text,
                    });
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
                    });
                    app.loading = false;
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                llm::LlmEvent::Usage { input_tokens, output_tokens } => {
                    app.total_input_tokens += input_tokens;
                    app.total_output_tokens += output_tokens;
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
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            if app.input.is_empty() {
                                break;
                            } else {
                                app.input.clear();
                                app.cursor_pos = 0;
                                app.history_index = None;
                            }
                        }
                        KeyCode::Char('a') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.move_cursor_to_start();
                        }
                        KeyCode::Char('e') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.move_cursor_to_end();
                        }
                        KeyCode::Char('w') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.kill_word_backward();
                        }
                        KeyCode::Char('u') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.kill_line();
                        }
                        KeyCode::Char('j') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.insert_char('\n');
                        }
                        KeyCode::Char(c) => app.insert_char(c),
                        KeyCode::Backspace => {
                            app.delete_char();
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
                            let scrollbar_x = app.messages_rect.x + app.messages_rect.width.saturating_sub(1);
                            if mouse.column == scrollbar_x && mouse.row >= app.messages_rect.y && mouse.row < app.messages_rect.y + app.messages_rect.height {
                                app.handle_scrollbar_click(mouse.row);
                                app.dragging_scrollbar = true;
                            }
                        }
                        MouseEventKind::Drag(_) if app.dragging_scrollbar => {
                            app.handle_scrollbar_click(mouse.row);
                        }
                        MouseEventKind::Up(_) => {
                            app.dragging_scrollbar = false;
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
