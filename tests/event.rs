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
        None,
        "build".to_string(),
        Default::default(),
        None,
        "test_session".to_string(),
        ".".to_string(),
        Vec::new(),
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
    let mut db = Db { conn };
    db.run_migrations()?;
    Ok(db)
}

/// Process pending channel events (for tests with async tool execution)
fn process_pending_events(app: &mut App) {
    // Give background threads a moment to execute
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Process all pending events from the channel
    while let Ok(event) = app.rx.try_recv() {
        handle_llm_event(app, event);
    }
}

#[test]
fn test_handle_token_event_adds_content() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Set up: add initial content
    app.pending_response = "Initial message\n".to_string();

    // Send a token event
    handle_llm_event(&mut app, LlmEvent::Token("new text ".to_string(), 1));

    // Should have updated content
    assert!(app.pending_response.contains("new text"));
    // auto_scroll should not be modified by token events
    assert!(app.auto_scroll);
}

#[test]
fn test_handle_token_event_preserves_auto_scroll_state() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // User scrolled up, disabling auto_scroll
    app.scroll_up();
    assert!(!app.auto_scroll);
    let saved_offset = app.scroll_offset;

    // Send a token event
    handle_llm_event(&mut app, LlmEvent::Token("new text ".to_string(), 1));

    // auto_scroll should remain false (event handlers don't touch scroll)
    assert!(!app.auto_scroll);
    // scroll_offset should not change
    assert_eq!(app.scroll_offset, saved_offset);
}

#[test]
fn test_handle_token_event_startup_unsized_viewport() {
    let mut app = create_test_app();

    // Viewport not sized yet (height = 0, simulates startup)
    assert_eq!(app.messages_rect.height, 0);
    assert_eq!(app.scroll_offset, 0);

    // Send first token
    handle_llm_event(&mut app, LlmEvent::Token("first ".to_string(), 1));
    assert_eq!(app.pending_response, "first ");

    // auto_scroll should still be true (renderer will handle positioning)
    assert!(app.auto_scroll);
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

    handle_llm_event(&mut app, LlmEvent::Thinking("thinking...".to_string(), 1));

    assert_eq!(app.pending_thinking, "thinking...");
}

#[test]
fn test_handle_done_event_creates_message() {
    let mut app = create_test_app();
    app.pending_response = "Hello world".to_string();
    app.pending_thinking = "thought about this".to_string();

    let initial_count = app.messages.len();

    handle_llm_event(&mut app, LlmEvent::Done(1));

    // Message should be added
    assert_eq!(app.messages.len(), initial_count + 1);
    // Pending response and thinking should be cleared
    assert!(app.pending_response.is_empty());
    assert!(app.pending_thinking.is_empty());
    // Loading should be reset
    assert_eq!(app.active_llm_calls, 0);

    // Verify message content
    let msg = &app.messages[initial_count];
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.text, "Hello world");
    assert_eq!(msg.thinking, Some("thought about this".to_string()));
}

#[test]
fn test_handle_done_event_does_not_modify_scroll() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // User scrolled up, disabling auto_scroll
    app.scroll_up();
    assert!(!app.auto_scroll);
    let saved_offset = app.scroll_offset;

    // Send a Done event with a multi-line message
    app.pending_response = "New assistant response\nLine 2\nLine 3".to_string();
    handle_llm_event(&mut app, LlmEvent::Done(1));

    // Event handler should not modify scroll state — renderer handles that
    assert!(!app.auto_scroll);
    assert_eq!(app.scroll_offset, saved_offset);
}

#[test]
fn test_handle_error_event() {
    let mut app = create_test_app();
    let initial_count = app.messages.len();

    handle_llm_event(
        &mut app,
        LlmEvent::Error("API error occurred".to_string(), 1),
    );

    // Error message should be added
    assert_eq!(app.messages.len(), initial_count + 1);
    assert_eq!(app.active_llm_calls, 0);

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
            call_id: 1,
        },
    );

    assert_eq!(app.total_input_tokens, initial_input + 10);
    assert_eq!(app.total_output_tokens, initial_output + 20);
}

#[test]
fn test_active_llm_calls_counter_with_tool_calls() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Simulate: user sends message, triggers first API call
    app.active_llm_calls = 1;
    app.pending_response = "I'll read that file for you".to_string();

    // LLM responds with tool call
    handle_llm_event(
        &mut app,
        LlmEvent::ToolCall {
            id: "call_789".to_string(),
            name: "Read".to_string(),
            args: serde_json::from_str(r#"{"filePath": "test.txt"}"#).unwrap(),
            call_id: 1,
        },
    );

    // Process pending events from background thread
    process_pending_events(&mut app);

    // Tool result message added; pending_tool_count should be 0 (tool completed)
    assert_eq!(app.active_llm_calls, 1);
    assert_eq!(
        app.pending_tool_count, 0,
        "Tool count should be 0 after result arrives"
    );
    assert!(
        app.messages.last().unwrap().is_tool_result,
        "Tool result should be added"
    );

    // First API call finishes (sends Done)
    handle_llm_event(&mut app, LlmEvent::Done(1));

    // Counter should reach 0 (no more active calls)
    assert_eq!(
        app.active_llm_calls, 0,
        "Loading should stop when call completes"
    );
}

#[test]
fn test_active_llm_calls_counter_error_resets_properly() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Two concurrent calls
    app.active_llm_calls = 2;

    // First call errors out
    handle_llm_event(&mut app, LlmEvent::Error("Network timeout".to_string(), 1));

    // Counter decrements but still loading
    assert_eq!(app.active_llm_calls, 1);

    // Second call errors
    handle_llm_event(
        &mut app,
        LlmEvent::Error("Connection refused".to_string(), 1),
    );

    // Now counter reaches 0
    assert_eq!(app.active_llm_calls, 0);
}

#[test]
fn test_tool_call_preserves_thinking_content() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Simulate: assistant generates thinking then makes a tool call
    app.active_llm_calls = 1;
    app.pending_thinking = "I need to read the file to answer this.".to_string();
    app.pending_response = "Let me check that for you.".to_string();

    let initial_count = app.messages.len();

    // LLM decides to use a tool (use an existing file - Cargo.toml)
    handle_llm_event(
        &mut app,
        LlmEvent::ToolCall {
            id: "call_123".to_string(),
            name: "Read".to_string(),
            args: serde_json::from_str(r#"{"filePath": "Cargo.toml"}"#).unwrap(),
            call_id: 1,
        },
    );

    // Process pending events from background thread
    process_pending_events(&mut app);

    // Should have added TWO messages: assistant's thinking/response + tool result
    assert_eq!(app.messages.len(), initial_count + 2);

    // First message should be the assistant's response with thinking
    let assistant_msg = &app.messages[initial_count];
    assert_eq!(assistant_msg.role, "assistant");
    assert_eq!(assistant_msg.text, "Let me check that for you.");
    assert_eq!(
        assistant_msg.thinking,
        Some("I need to read the file to answer this.".to_string())
    );
    assert!(!assistant_msg.is_tool_result);

    // Second message should be the tool result
    let tool_msg = &app.messages[initial_count + 1];
    assert_eq!(tool_msg.role, "user");
    assert!(tool_msg.is_tool_result);
    assert!(tool_msg.text.contains("Cargo.toml"));

    // Pending content should be cleared
    assert!(app.pending_response.is_empty());
    assert!(app.pending_thinking.is_empty());

    // Tool completed, pending_tool_count back to 0
    assert_eq!(app.active_llm_calls, 1);
    assert_eq!(
        app.pending_tool_count, 0,
        "Tool count should be 0 after result arrives"
    );
}

#[test]
fn test_tool_call_without_pending_content() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // No pending content (e.g., tool call at start of response)
    app.active_llm_calls = 1;
    app.pending_thinking.clear();
    app.pending_response.clear();

    let initial_count = app.messages.len();

    handle_llm_event(
        &mut app,
        LlmEvent::ToolCall {
            id: "call_456".to_string(),
            name: "Read".to_string(),
            args: serde_json::from_str(r#"{"filePath": "Cargo.toml"}"#).unwrap(),
            call_id: 1,
        },
    );

    // Process pending events from background thread
    process_pending_events(&mut app);

    // Should only add ONE message (just the tool result, no empty assistant message)
    assert_eq!(app.messages.len(), initial_count + 1);

    // The only new message should be the tool result
    let tool_msg = &app.messages[initial_count];
    assert_eq!(tool_msg.role, "user");
    assert!(tool_msg.is_tool_result);
}

#[test]
fn test_handle_done_event_empty_response_no_message() {
    let mut app = create_test_app();
    app.active_llm_calls = 1;
    // No pending content
    app.pending_response.clear();
    app.pending_thinking.clear();

    let initial_count = app.messages.len();

    handle_llm_event(&mut app, LlmEvent::Done(1));

    // Should NOT create a message when there's no content
    assert_eq!(app.messages.len(), initial_count);
    assert_eq!(app.active_llm_calls, 0);
}
