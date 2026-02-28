#![deny(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::dbg_macro,
    // Note: eprintln! will fail the build with this deny
)]

use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::execute;
use std::io::stdout;
use std::time::Duration;

use pact::app::App;
use pact::config::Config;
use pact::{event, ui, utils};

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
    let agents_context = config.load_agents_context();

    let server_info = utils::fetch_server_info(&config.api.endpoint);

    let debug = args.debug || config.debug;

    let mut app = App::new(
        debug,
        config.api.endpoint.clone(),
        config.api.max_tokens,
        temperature,
        config.ui.default_mode.clone(),
        modes_config,
        agents_context,
    );
    // Load history from SQLite database (for up/down arrow navigation)
    // Don't load previous messages - start with a fresh session
    app.load_history_from_db();
    app.context_window = server_info.context_window;
    app.model_name = server_info.model_name;

    loop {
        terminal.draw(|f| ui::draw_app(&mut app, f))?;

        // Process LLM events from background thread
        while let Ok(llm_event) = app.rx.try_recv() {
            event::handle_llm_event(&mut app, llm_event);
        }

        // Poll for terminal events (16ms timeout for smooth UI at 60fps)
        if crossterm::event::poll(Duration::from_millis(16))? {
            match crossterm::event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    // Returns false if user pressed Ctrl+C to exit
                    if !event::handle_key_event(&mut app, key) {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    event::handle_mouse_event(&mut app, mouse);
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
