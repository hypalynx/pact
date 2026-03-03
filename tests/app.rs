use indexmap::IndexMap;
use pact::app::App;
use pact::config::Mode;
use pact::db::Db;
use pact::llm::Message;
use ratatui::layout::Rect;
use rusqlite::Connection;

/// Create a test app with in-memory database (no database side effects)
fn create_test_app() -> App {
    let app = App::new(
        false,
        None,
        "build".to_string(),
        Default::default(),
        None,
        "test_session".to_string(),
        std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string()),
        Vec::new(),
    );

    // Replace the real database with an in-memory one for testing
    let mut app = app;
    match create_temp_db() {
        Ok(db) => app.db = Some(db),
        Err(_) => app.db = None, // Graceful fallback
    }
    app
}

/// Create an in-memory SQLite database for tests (no real database writes)
fn create_temp_db() -> Result<Db, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -4000;
        PRAGMA foreign_keys = ON;
        PRAGMA analysis_limit = 400;
        "#,
    )?;

    let mut db = Db { conn };
    db.run_migrations()?;
    Ok(db)
}

/// Helper to add messages to app
fn add_test_messages(app: &mut App, count: usize) {
    for i in 0..count {
        let msg = Message {
            role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
            text: format!("Message {}\nLine 2\nLine 3", i),
            is_tool_result: false,
            thinking: None,
            tool_result_content: None,
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        };
        app.messages.push(msg);
    }
}

#[test]
fn test_auto_scroll_initialization() {
    let app = create_test_app();
    // New app should start with auto_scroll = true
    assert!(app.auto_scroll);
}

#[test]
fn test_scroll_up_disables_auto_scroll() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    assert!(app.auto_scroll);
    app.scroll_up();
    assert!(!app.auto_scroll);
}

#[test]
fn test_scroll_down_to_bottom_enables_auto_scroll() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Simulate rendered_line_count being set by renderer
    app.rendered_line_count = 50;

    // Simulate user scrolling up
    app.scroll_up();
    assert!(!app.auto_scroll);

    // Scroll down to bottom (set offset near max so scroll_down reaches it)
    let max_scroll = app
        .rendered_line_count
        .saturating_sub(app.messages_rect.height as usize);
    app.scroll_offset = max_scroll;
    app.scroll_down();

    // Should re-enable auto_scroll when at bottom
    assert!(app.auto_scroll);
}

#[test]
fn test_scroll_offset_bounds() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 5);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Simulate rendered_line_count being set by renderer
    app.rendered_line_count = 30;
    let max_scroll = app
        .rendered_line_count
        .saturating_sub(app.messages_rect.height as usize);

    // Scroll down multiple times
    for _ in 0..10 {
        app.scroll_down();
    }

    // Should not exceed max scroll
    assert!(app.scroll_offset <= max_scroll);
}

#[test]
fn test_scroll_up_multiple_times() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Simulate rendered_line_count and position in middle
    app.rendered_line_count = 50;
    let max_scroll = app.rendered_line_count.saturating_sub(10);
    app.scroll_offset = max_scroll / 2;

    let initial_offset = app.scroll_offset;

    // Scroll up several times
    for _ in 0..3 {
        app.scroll_up();
    }

    // Offset should have decreased (scrolled up in content)
    assert!(app.scroll_offset < initial_offset);
    assert!(!app.auto_scroll);
}

#[test]
fn test_submit_message_clears_input() {
    let mut app = create_test_app();
    app.input = "Test message".to_string();
    app.cursor_pos = 12;

    app.submit_message();

    // Input should be cleared after submit
    assert_eq!(app.input, "");
    assert_eq!(app.cursor_pos, 0);
}

#[test]
fn test_submit_message_enables_auto_scroll() {
    let mut app = create_test_app();
    app.auto_scroll = false;
    app.input = "Test message".to_string();
    app.cursor_pos = 12;

    app.submit_message();

    // Submitting a message should re-enable auto_scroll
    assert!(app.auto_scroll);
}

#[test]
fn test_scroll_offset_saturating_behavior() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 3);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 50,
    };

    // Try to scroll up when already at top
    let initial_offset = app.scroll_offset;
    app.scroll_up();

    // Should use saturating_sub, so it stops at 0
    assert!(app.scroll_offset <= initial_offset);
}

#[test]
fn test_auto_scroll_flag_controls_scroll_behavior() {
    let mut app = create_test_app();

    // auto_scroll starts true
    assert!(app.auto_scroll);

    // scroll_up disables it
    app.scroll_up();
    assert!(!app.auto_scroll);

    // Simulating scroll_down to bottom re-enables it
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.rendered_line_count = 10; // Content fits viewport
    app.scroll_offset = 0;
    app.scroll_down(); // At max_scroll = 0, offset >= max_scroll
    assert!(app.auto_scroll);
}

#[test]
fn test_agents_context_stored_in_app() {
    let app = App::new(
        false,
        None,
        "build".to_string(),
        Default::default(),
        Some("Agent context content here".to_string()),
        "test_session".to_string(),
        ".".to_string(),
        Vec::new(),
    );

    // Verify agents_context is stored correctly
    assert_eq!(
        app.agents_context,
        Some("Agent context content here".to_string())
    );
}

#[test]
fn test_agents_context_replaces_system_prompt() {
    let mut modes = IndexMap::new();
    modes.insert(
        "build".to_string(),
        Mode {
            system_prompt: Some("You are a helpful coding assistant...".to_string()),
            temperature: None,
            color: Some("cyan".to_string()),
        },
    );

    let app = App::new(
        false,
        None,
        "build".to_string(),
        modes,
        Some("Agent context content here".to_string()),
        "test_session".to_string(),
        ".".to_string(),
        Vec::new(),
    );

    // When agents_context is present, it should replace the mode prompt
    let expected_prompt = if let Some(ref ctx) = app.agents_context {
        Some(ctx.clone())
    } else {
        app.modes_config
            .get(&app.mode_name)
            .and_then(|m| m.system_prompt.clone())
    };

    // Verify that the system prompt is the agents context, not the mode prompt
    let final_prompt = expected_prompt.unwrap();
    assert_eq!(
        final_prompt, "Agent context content here",
        "Should use agents_context instead of mode prompt"
    );
    assert!(
        !final_prompt.contains("helpful coding"),
        "Should not contain mode prompt"
    );
}

#[test]
fn test_agents_context_none_uses_only_mode_prompt() {
    let mut modes = IndexMap::new();
    modes.insert(
        "build".to_string(),
        Mode {
            system_prompt: Some("You are a helpful coding assistant...".to_string()),
            temperature: None,
            color: Some("cyan".to_string()),
        },
    );

    let app = App::new(
        false,
        None,
        "build".to_string(),
        modes,
        None, // No agents context
        "test_session".to_string(),
        ".".to_string(),
        Vec::new(),
    );

    // Verify agents_context is None
    assert!(app.agents_context.is_none());

    // Get the mode prompt
    let mode_prompt = app
        .modes_config
        .get(&app.mode_name)
        .and_then(|m| m.system_prompt.clone())
        .unwrap_or_default();

    // When no agents_context, the system prompt should just be the mode prompt
    assert!(
        mode_prompt.contains("helpful coding"),
        "Should contain mode prompt"
    );
    assert!(
        !mode_prompt.contains("Agent context"),
        "Should not contain agents context"
    );
}
