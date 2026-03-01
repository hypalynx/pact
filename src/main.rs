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
use pact::db::Db;
use pact::{event, ui, utils};

#[derive(Parser)]
#[command(name = "pact")]
struct Args {
    #[arg(long)]
    debug: bool,
}

const DEFAULT_LOCAL_ENDPOINT: &str = "http://127.0.0.1:7777";

fn init_providers_from_config(config: &Config) {
    if let Ok(db) = Db::open()
        && let Ok(existing) = db.get_providers()
    {
        // If no providers in DB, create default local provider
        if existing.is_empty() {
            let _ = db.add_provider("local", DEFAULT_LOCAL_ENDPOINT, None, Some("local"));
            let _ = db.set_active_provider("local");
        }

        // Add any providers from config that don't exist
        for provider in &config.providers {
            if !db.provider_exists(&provider.name).unwrap_or(false) {
                let _ = db.add_provider(
                    &provider.name,
                    &provider.endpoint,
                    None,
                    provider.default_model.as_deref(),
                );
            }
            
            // Update models list from config (whether provider is new or existing)
            if !provider.models.is_empty() {
                let _ = db.set_provider_models(&provider.name, &provider.models);
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let config = Config::load();

    // Initialize providers from config
    init_providers_from_config(&config);

    let mut terminal = ratatui::init();

    execute!(stdout(), EnableMouseCapture)?;

    let default_mode_config = config.ui.modes.get(&config.ui.default_mode).cloned();
    let temperature = default_mode_config.as_ref().and_then(|m| m.temperature);
    let modes_config = config.ui.modes.clone();
    let agents_context = config.load_agents_context();

    let debug = args.debug || config.debug;

    let mut app = App::new(
        debug,
        temperature,
        config.ui.default_mode.clone(),
        modes_config,
        agents_context,
    );
    // Load history from SQLite database (for up/down arrow navigation)
    // Don't load previous messages - start with a fresh session
    app.load_history_from_db();
    app.load_providers_from_db();

    // Fetch server info from active provider endpoint
    if let Some(ref provider) = app.active_provider {
        let server_info = utils::fetch_server_info(&provider.endpoint);
        app.context_window = server_info.context_window;
        app.model_name = server_info.model_name;
    } else {
        // Fallback to default endpoint if no provider
        let server_info = utils::fetch_server_info(DEFAULT_LOCAL_ENDPOINT);
        app.context_window = server_info.context_window;
        app.model_name = server_info.model_name;
    }

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
