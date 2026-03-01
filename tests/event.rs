use pact::app::App;
use pact::db::Db;
use pact::event::handle_llm_event;
use pact::llm::LlmEvent;
use ratatui::layout::Rect;
use rusqlite::Connection;

/// Create a test app with in-memory database
fn create_test_app() -> App {
    let app = App::new(false, None, "build".to_string(), Default::default(), None);

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
    assert_eq!(app.active_llm_calls, 0);

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
            tool_call_id: None,
            tool_name: None,
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
        },
    );

    // Tool result message added, and send_to_llm() called → counter now 2
    assert_eq!(app.active_llm_calls, 2);
    assert!(
        app.messages.last().unwrap().is_tool_result,
        "Tool result should be added"
    );

    // First API call finishes (sends Done)
    handle_llm_event(&mut app, LlmEvent::Done);

    // Counter should decrement but NOT reach 0 (second call still in progress)
    assert_eq!(
        app.active_llm_calls, 1,
        "Loading should persist while second call is active"
    );

    // Second API call finishes
    app.pending_response = "Here's the file content...".to_string();
    handle_llm_event(&mut app, LlmEvent::Done);

    // Now counter reaches 0
    assert_eq!(
        app.active_llm_calls, 0,
        "Loading should stop when all calls complete"
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
    handle_llm_event(&mut app, LlmEvent::Error("Network timeout".to_string()));

    // Counter decrements but still loading
    assert_eq!(app.active_llm_calls, 1);

    // Second call errors
    handle_llm_event(&mut app, LlmEvent::Error("Connection refused".to_string()));

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
        },
    );

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

    // A new LLM call should have started
    assert_eq!(app.active_llm_calls, 2);
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
        },
    );

    // Should only add ONE message (just the tool result, no empty assistant message)
    assert_eq!(app.messages.len(), initial_count + 1);

    // The only new message should be the tool result
    let tool_msg = &app.messages[initial_count];
    assert_eq!(tool_msg.role, "user");
    assert!(tool_msg.is_tool_result);
}
