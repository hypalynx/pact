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
    /// Resume a session (with optional session ID, or list available sessions)
    #[arg(long, value_name = "SESSION_ID")]
    resume: Option<Option<String>>,
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

fn get_working_dir() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

#[allow(clippy::print_stdout, clippy::print_stderr)]
fn list_available_sessions(db: &Db, working_dir: &str) -> std::io::Result<()> {
    match db.get_sessions_for_directory(working_dir) {
        Ok(sessions) if sessions.is_empty() => {
            println!("No sessions found in {}", working_dir);
            println!("Run `pact` to start a new session.");
        }
        Ok(sessions) => {
            println!("Available sessions in {}:", working_dir);
            for session in sessions {
                let preview = session
                    .first_prompt
                    .as_ref()
                    .map(|s| s.chars().take(60).collect::<String>())
                    .unwrap_or_else(|| "(no messages)".to_string());
                let count = db
                    .get_session_message_count(&session.session_id)
                    .unwrap_or(0);
                let time = session
                    .created_at
                    .split('T')
                    .next()
                    .unwrap_or(&session.created_at);
                println!(
                    "  {}: \"{}\" ({} msgs, {})",
                    session.session_id, preview, count, time
                );
            }
            println!("\nRun `pact --resume <SESSION_ID>` to resume a specific session.");
        }
        Err(e) => {
            eprintln!("Failed to list sessions: {}", e);
        }
    }
    Ok(())
}

#[allow(clippy::print_stdout, clippy::print_stderr)]
fn handle_session_init(
    resume_arg: &Option<Option<String>>,
    working_dir: &str,
) -> (String, Vec<pact::llm::Message>) {
    if let Some(resume_arg) = resume_arg {
        if let Ok(mut db) = Db::open() {
            let _ = db.run_migrations();
            match resume_arg {
                Some(specific_id) => {
                    // Resume specific session
                    match db.load_session_messages(specific_id) {
                        Ok(msgs) => {
                            println!(
                                "Resuming session {} with {} messages",
                                specific_id,
                                msgs.len()
                            );
                            (specific_id.clone(), msgs)
                        }
                        Err(e) => {
                            eprintln!("Failed to load session {}: {}", specific_id, e);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    // List available sessions and exit
                    list_available_sessions(&db, working_dir).ok();
                    std::process::exit(0);
                }
            }
        } else {
            eprintln!("Failed to open database for session management");
            std::process::exit(1);
        }
    } else {
        // Create new session
        let new_session_id = utils::generate_session_id();
        if let Ok(mut db) = Db::open() {
            let _ = db.run_migrations();
            let _ = db.create_session(&new_session_id, working_dir, None);
        }
        (new_session_id, Vec::new())
    }
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let config = Config::load();

    // Initialize providers from config
    init_providers_from_config(&config);

    let working_dir = get_working_dir();

    // Handle session initialization/resuming before starting UI
    let (session_id, messages_to_load) = handle_session_init(&args.resume, &working_dir);

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
        session_id,
        working_dir,
        messages_to_load,
    );
    // Load history from SQLite database (for up/down arrow navigation)
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
        // Limit events per frame to keep UI responsive during heavy streaming
        const MAX_EVENTS_PER_FRAME: usize = 20;
        for _ in 0..MAX_EVENTS_PER_FRAME {
            if let Ok(llm_event) = app.rx.try_recv() {
                event::handle_llm_event(&mut app, llm_event);
            } else {
                break;
            }
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
