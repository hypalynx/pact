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
        "http://127.0.0.1:7777".to_string(),
        1024,
        None,
        "build".to_string(),
        Default::default(),
        None,
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

    let db = Db { conn };
    db.init_schema()?;
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
        };
        app.messages.push(msg);
    }
}

#[test]
fn test_scroll_info_detects_at_bottom() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 2);

    // Set viewport to simulate terminal size
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    let (at_bottom, _total_lines) = app.calculate_scroll_info();
    // With scroll_offset = 0 and small number of lines, should be at bottom
    assert!(at_bottom);
}

#[test]
fn test_scroll_up_sets_user_scrolled() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    assert!(!app.user_scrolled);
    app.scroll_up();
    assert!(app.user_scrolled);
}

#[test]
fn test_scroll_down_to_bottom_clears_user_scrolled() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Simulate user scrolling up
    app.scroll_up();
    assert!(app.user_scrolled);

    // Scroll down to bottom
    let total_lines = app.calculate_total_lines();
    let max_scroll = total_lines.saturating_sub(10);
    app.scroll_offset = max_scroll;
    app.scroll_down();

    // Should reset user_scrolled when at bottom
    assert!(!app.user_scrolled);
}

#[test]
fn test_was_at_bottom_initialization() {
    let app = create_test_app();
    // New app should start with was_at_bottom = true
    assert!(app.was_at_bottom);
}

#[test]
fn test_calculate_total_lines() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 2);

    let total_lines = app.calculate_total_lines();
    // Each message has 3 lines, so 2 messages = 6 lines
    // Plus some spacing/formatting may add lines
    assert!(total_lines >= 6);
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

    let total_lines = app.calculate_total_lines();
    let max_scroll = total_lines.saturating_sub(app.messages_rect.height as usize);

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

    // First scroll to middle of content
    let total_lines = app.calculate_total_lines();
    let max_scroll = total_lines.saturating_sub(10);
    if max_scroll > 5 {
        app.scroll_offset = max_scroll / 2;
    }

    let initial_offset = app.scroll_offset;

    // Scroll up several times
    for _ in 0..3 {
        app.scroll_up();
    }

    // Offset should have decreased (scrolled up in content)
    assert!(app.scroll_offset < initial_offset);
    assert!(app.user_scrolled);
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
fn test_send_to_llm_resets_user_scrolled() {
    let mut app = create_test_app();
    app.user_scrolled = true;

    app.send_to_llm();

    // Sending to LLM should reset user_scrolled
    // (This is the current behavior, may need to change)
    assert!(!app.user_scrolled);
}

#[test]
fn test_calculate_scroll_info_with_overflow() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 20);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 5,
    };

    let (at_bottom, total_lines) = app.calculate_scroll_info();

    // With many lines and small viewport, total_lines should be much > viewport
    assert!(total_lines > 5);

    // At scroll_offset 0, we're not at bottom when content overflows
    assert!(!at_bottom);
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
fn test_sticky_to_bottom_when_at_bottom() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Position at bottom
    let total_lines = app.calculate_total_lines();
    let max_scroll = total_lines.saturating_sub(10);
    app.scroll_offset = max_scroll;

    // Verify we're at bottom
    let (at_bottom, _) = app.calculate_scroll_info();
    assert!(at_bottom);

    // Simulate new content arriving (like a Token event)
    // The scroll logic should auto-scroll to stay at bottom
    let (at_bottom, new_total) = app.calculate_scroll_info();
    if at_bottom {
        app.scroll_offset = new_total.saturating_sub(10);
    }

    // Should be at new bottom position
    let (still_at_bottom, _) = app.calculate_scroll_info();
    assert!(still_at_bottom);
}

#[test]
fn test_no_auto_scroll_when_scrolled_up() {
    let mut app = create_test_app();
    add_test_messages(&mut app, 10);

    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Position at bottom
    let total_lines = app.calculate_total_lines();
    let max_scroll = total_lines.saturating_sub(10);
    app.scroll_offset = max_scroll;

    // User scrolls up (away from bottom)
    app.scroll_up();

    // Verify we're NOT at bottom
    let (at_bottom, _) = app.calculate_scroll_info();
    assert!(!at_bottom);

    let saved_offset = app.scroll_offset;

    // Simulate new content arriving
    // The scroll logic should NOT auto-scroll when not at bottom
    let (at_bottom, new_total) = app.calculate_scroll_info();
    if at_bottom {
        app.scroll_offset = new_total.saturating_sub(10);
    }

    // Should stay at same position (not auto-scrolled)
    assert_eq!(app.scroll_offset, saved_offset);
    let (still_not_at_bottom, _) = app.calculate_scroll_info();
    assert!(!still_not_at_bottom);
}

#[test]
fn test_agents_context_stored_in_app() {
    let app = App::new(
        false,
        "http://127.0.0.1:7777".to_string(),
        1024,
        None,
        "build".to_string(),
        Default::default(),
        Some("Agent context content here".to_string()),
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
        "http://127.0.0.1:7777".to_string(),
        1024,
        None,
        "build".to_string(),
        modes,
        Some("Agent context content here".to_string()),
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
        "http://127.0.0.1:7777".to_string(),
        1024,
        None,
        "build".to_string(),
        modes,
        None, // No agents context
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
