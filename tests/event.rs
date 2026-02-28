use pact::app::App;
use pact::db::Db;
use pact::event::handle_llm_event;
use pact::llm::LlmEvent;
use ratatui::layout::Rect;
use rusqlite::Connection;

/// Create a test app with in-memory database
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

    let mut app = app;
    match create_temp_db() {
        Ok(db) => app.db = Some(db),
        Err(_) => app.db = None,
    }
    app
}

/// Create an in-memory SQLite database for tests
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

#[test]
fn test_handle_token_event_at_bottom_auto_scrolls() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Set up: add a message and position at bottom
    app.pending_response = "Initial message\n".to_string();
    let initial_lines = app.calculate_total_lines();
    app.scroll_offset = initial_lines.saturating_sub(10);

    // Verify we're at bottom before token arrives
    let (was_at_bottom, _) = app.calculate_scroll_info();
    assert!(was_at_bottom);

    // Send a token event
    handle_llm_event(&mut app, LlmEvent::Token("new text ".to_string()));

    // Should have updated content
    assert!(app.pending_response.contains("new text"));

    // Scroll offset should have increased to keep us at bottom
    let (still_at_bottom, _) = app.calculate_scroll_info();
    assert!(still_at_bottom);
}

#[test]
fn test_handle_token_event_not_at_bottom_no_auto_scroll() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Add enough messages to have room to scroll
    for i in 0..10 {
        app.pending_response.push_str(&format!("Message {}\n", i));
    }

    // Position at bottom first
    let total_lines = app.calculate_total_lines();
    app.scroll_offset = total_lines.saturating_sub(10);

    // Verify we're at bottom
    let (at_bottom, _) = app.calculate_scroll_info();
    assert!(at_bottom);

    // Now scroll up (away from bottom)
    app.scroll_up();
    let saved_offset = app.scroll_offset;

    // Verify we're NO longer at bottom
    let (still_at_bottom, _) = app.calculate_scroll_info();
    assert!(!still_at_bottom);

    // Send a token event
    handle_llm_event(&mut app, LlmEvent::Token("new text ".to_string()));

    // When not at bottom, scroll position should not change (don't auto-scroll)
    assert_eq!(app.scroll_offset, saved_offset);
}

#[test]
fn test_handle_token_event_startup_unsized_viewport_auto_scrolls() {
    let mut app = create_test_app();

    // Viewport not sized yet (height = 0, simulates startup)
    assert_eq!(app.messages_rect.height, 0);
    assert_eq!(app.scroll_offset, 0);

    // Send first token
    handle_llm_event(&mut app, LlmEvent::Token("first ".to_string()));
    assert_eq!(app.pending_response, "first ");

    // Size the viewport
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Verify we're at bottom (scroll_offset adjusted for content)
    let (at_bottom, _) = app.calculate_scroll_info();
    assert!(at_bottom);

    // More tokens should keep us at bottom
    handle_llm_event(&mut app, LlmEvent::Token("second ".to_string()));
    let (still_at_bottom, _) = app.calculate_scroll_info();
    assert!(still_at_bottom);
}

#[test]
fn test_handle_thinking_event() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    handle_llm_event(&mut app, LlmEvent::Thinking("thinking...".to_string()));

    assert_eq!(app.pending_thinking, "thinking...");
}

#[test]
fn test_handle_done_event_creates_message() {
    let mut app = create_test_app();
    app.pending_response = "Hello world".to_string();
    app.pending_thinking = "thought about this".to_string();

    let initial_count = app.messages.len();

    handle_llm_event(&mut app, LlmEvent::Done);

    // Message should be added
    assert_eq!(app.messages.len(), initial_count + 1);
    // Pending response and thinking should be cleared
    assert!(app.pending_response.is_empty());
    assert!(app.pending_thinking.is_empty());
    // Loading should be reset
    assert!(!app.loading);

    // Verify message content
    let msg = &app.messages[initial_count];
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.text, "Hello world");
    assert_eq!(msg.thinking, Some("thought about this".to_string()));
}

#[test]
fn test_handle_done_event_not_at_bottom_no_auto_scroll() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Add enough messages to have room to scroll
    for i in 0..10 {
        app.messages.push(pact::llm::Message {
            role: "assistant".to_string(),
            text: format!("Message {}\nLine 2\nLine 3\n", i),
            is_tool_result: false,
            thinking: None,
            tool_result_content: None,
        });
    }

    // Position at bottom first
    let total_lines = app.calculate_total_lines();
    app.scroll_offset = total_lines.saturating_sub(10);

    // Verify we're at bottom
    let (at_bottom, _) = app.calculate_scroll_info();
    assert!(at_bottom);

    // Now scroll up (away from bottom)
    app.scroll_up();
    let saved_offset = app.scroll_offset;

    // Verify we're NO longer at bottom
    let (still_at_bottom, _) = app.calculate_scroll_info();
    assert!(!still_at_bottom);

    // Send a Done event with a multi-line message
    app.pending_response = "New assistant response\nLine 2\nLine 3".to_string();
    handle_llm_event(&mut app, LlmEvent::Done);

    // When not at bottom, scroll position should not change (don't auto-scroll to new bottom)
    assert_eq!(app.scroll_offset, saved_offset);
}

#[test]
fn test_handle_error_event() {
    let mut app = create_test_app();
    let initial_count = app.messages.len();

    handle_llm_event(&mut app, LlmEvent::Error("API error occurred".to_string()));

    // Error message should be added
    assert_eq!(app.messages.len(), initial_count + 1);
    assert!(!app.loading);

    let msg = &app.messages[initial_count];
    assert_eq!(msg.role, "assistant");
    assert!(msg.text.contains("Error"));
    assert!(msg.text.contains("API error occurred"));
}

#[test]
fn test_handle_usage_event_accumulates_tokens() {
    let mut app = create_test_app();
    let initial_input = app.total_input_tokens;
    let initial_output = app.total_output_tokens;

    handle_llm_event(
        &mut app,
        LlmEvent::Usage {
            input_tokens: 10,
            output_tokens: 20,
        },
    );

    assert_eq!(app.total_input_tokens, initial_input + 10);
    assert_eq!(app.total_output_tokens, initial_output + 20);
}

#[test]
fn test_handle_progress_event() {
    let mut app = create_test_app();

    assert!(app.progress.is_none());

    handle_llm_event(&mut app, LlmEvent::Progress(0.5));

    assert_eq!(app.progress, Some(0.5));
}
