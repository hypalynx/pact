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
        Err(_) => app.db = None,
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
    app.rendered_line_count = 50;

    app.scroll_up();
    assert!(!app.auto_scroll);

    let max_scroll = app
        .rendered_line_count
        .saturating_sub(app.messages_rect.height as usize);
    app.scroll_offset = max_scroll;
    app.scroll_down();

    assert!(app.auto_scroll);
}

#[test]
fn test_submit_message_clears_input() {
    let mut app = create_test_app();
    app.input = "Test message".to_string();
    app.cursor_pos = 12;

    app.submit_message();

    assert_eq!(app.input, "");
    assert_eq!(app.cursor_pos, 0);
}

#[test]
fn test_submit_message_enables_auto_scroll() {
    let mut app = create_test_app();
    app.auto_scroll = false;
    app.input = "Test".to_string();

    app.submit_message();
    assert!(app.auto_scroll);
}

#[test]
fn test_agents_context_stored_in_app() {
    let app = App::new(
        false,
        None,
        "build".to_string(),
        Default::default(),
        Some("Agent context".to_string()),
        "test_session".to_string(),
        ".".to_string(),
        Vec::new(),
    );
    assert_eq!(app.agents_context, Some("Agent context".to_string()));
}

#[test]
fn test_history_navigation_up() {
    let mut app = create_test_app();
    app.input = "draft".to_string();
    app.history.push("first".to_string());
    app.history.push("second".to_string());

    app.history_up();

    assert_eq!(app.input, "second");
    assert_eq!(app.history_index, Some(1));
}

#[test]
fn test_history_navigation_down() {
    let mut app = create_test_app();
    app.history.push("first".to_string());
    app.history.push("second".to_string());
    app.history_up();
    assert_eq!(app.input, "second");

    app.history_down();
    assert_eq!(app.input, "");
    assert_eq!(app.history_index, None);
}

#[test]
fn test_history_preserves_unsent_draft() {
    let mut app = create_test_app();
    app.input = "my draft".to_string();
    app.cursor_pos = 8;
    app.history.push("first".to_string());

    app.history_up();
    app.history_down();

    assert_eq!(app.input, "my draft");
    assert_eq!(app.cursor_pos, 8);
}

#[test]
fn test_set_status_message() {
    let mut app = create_test_app();
    app.set_status("Test".to_string(), pact::app::StatusLevel::Info);
    assert!(app.has_status());
    assert_eq!(app.get_status_level(), Some(pact::app::StatusLevel::Info));
}

#[test]
fn test_exit_confirmation() {
    let mut app = create_test_app();
    assert!(!app.is_exit_confirming());
    app.set_exit_confirmation();
    assert!(app.is_exit_confirming());
    app.reset_exit_confirmation();
    assert!(!app.is_exit_confirming());
}

#[test]
fn test_cancel_confirmation() {
    let mut app = create_test_app();
    assert!(!app.is_cancel_confirming());
    app.set_cancel_confirmation();
    assert!(app.is_cancel_confirming());
    app.reset_cancel_confirmation();
    assert!(!app.is_cancel_confirming());
}

#[test]
fn test_input_insert_char() {
    let mut app = create_test_app();
    app.input = "Hell".to_string();
    app.cursor_pos = 4;
    app.insert_char('o');
    assert_eq!(app.input, "Hello");
    assert_eq!(app.cursor_pos, 5);
}

#[test]
fn test_input_delete_char() {
    let mut app = create_test_app();
    // delete_char removes char BEFORE cursor
    app.input = "Hllo".to_string();
    app.cursor_pos = 1; // After 'H'
    app.delete_char();
    assert_eq!(app.input, "llo");
    assert_eq!(app.cursor_pos, 0);
}

#[test]
fn test_input_move_cursor() {
    let mut app = create_test_app();
    app.input = "Hello".to_string();

    app.cursor_pos = 5;
    app.move_cursor_to_start();
    assert_eq!(app.cursor_pos, 0);

    app.move_cursor_to_end();
    assert_eq!(app.cursor_pos, 5);

    app.move_cursor_backward();
    assert_eq!(app.cursor_pos, 4);

    app.move_cursor_forward();
    assert_eq!(app.cursor_pos, 5);
}

#[test]
fn test_input_kill_word_backward() {
    let mut app = create_test_app();
    app.input = "Hello world".to_string();
    app.cursor_pos = 11;
    app.kill_word_backward();
    assert_eq!(app.input, "Hello ");
    assert_eq!(app.cursor_pos, 6);
}

#[test]
fn test_input_kill_line() {
    let mut app = create_test_app();
    // kill_line removes from line start to cursor
    app.input = "Hello world".to_string();
    app.cursor_pos = 5;
    app.kill_line();
    assert_eq!(app.input, " world");
    assert_eq!(app.cursor_pos, 0);
}

#[test]
fn test_cycle_mode() {
    let mut app = create_test_app();
    let mut modes = IndexMap::new();
    modes.insert(
        "build".to_string(),
        Mode {
            system_prompt: None,
            temperature: None,
            color: Some("cyan".to_string()),
        },
    );
    modes.insert(
        "plan".to_string(),
        Mode {
            system_prompt: None,
            temperature: None,
            color: Some("magenta".to_string()),
        },
    );
    app.modes_config = modes;
    app.available_modes = vec!["build".to_string(), "plan".to_string()];
    app.mode_name = "build".to_string();

    app.cycle_mode();
    assert_eq!(app.mode_name, "plan");
    assert_eq!(app.mode_color, Some("magenta".to_string()));
}

#[test]
fn test_cycle_mode_wraps_around() {
    let mut app = create_test_app();
    let mut modes = IndexMap::new();
    modes.insert(
        "build".to_string(),
        Mode {
            system_prompt: None,
            temperature: None,
            color: Some("cyan".to_string()),
        },
    );
    modes.insert(
        "plan".to_string(),
        Mode {
            system_prompt: None,
            temperature: None,
            color: Some("magenta".to_string()),
        },
    );
    app.modes_config = modes;
    app.available_modes = vec!["build".to_string(), "plan".to_string()];
    app.mode_name = "plan".to_string();

    app.cycle_mode();
    assert_eq!(app.mode_name, "build");
}

#[test]
fn test_selection_handling() {
    let mut app = create_test_app();
    app.messages_rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };

    app.start_selection(10, 5);
    assert_eq!(app.selection_start, Some((10, 5)));

    app.extend_selection(20, 8);
    assert_eq!(app.selection_end, Some((20, 8)));

    // finish_selection clears the selection (and tries to copy)
    app.finish_selection();
    // After finish, selection is cleared
    assert!(app.selection_start.is_none());
    assert!(app.selection_end.is_none());
}

#[test]
fn test_is_copying() {
    let mut app = create_test_app();
    // Initially not copying (last_copy_frame is u32::MAX)
    assert!(!app.is_copying());
    // Simulate a copy by setting last_copy_frame to current frame
    app.last_copy_frame = app.frame_count;
    assert!(app.is_copying());
    // After 126 frames, should not be copying (indicator lasts 125 frames)
    app.last_copy_frame = 0;
    app.frame_count = 126;
    assert!(!app.is_copying());
}

#[test]
fn test_cancel_current_call() {
    let mut app = create_test_app();
    app.active_llm_calls = 1;
    app.active_call_id = Some(1);
    app.cancel_current_call();
    // active_llm_calls is decremented, active_call_id is NOT cleared by cancel_current_call
    assert_eq!(app.active_llm_calls, 0);
    // Should have added a cancellation message
    assert!(!app.messages.is_empty());
}

#[test]
fn test_was_just_cancelled() {
    let mut app = create_test_app();
    // Initially not just cancelled (last_cancel_frame is u32::MAX)
    assert!(!app.was_just_cancelled());
    // Need active_llm_calls > 0 for cancel to set last_cancel_frame
    app.active_llm_calls = 1;
    app.cancel_current_call();
    // After cancel, should be "just cancelled"
    assert!(app.was_just_cancelled());
    // After 126 frames, should not be "just cancelled" (lasts 125 frames)
    app.last_cancel_frame = 0;
    app.frame_count = 126;
    assert!(!app.was_just_cancelled());
}

#[test]
fn test_toggle_debug_row_expand() {
    let mut app = create_test_app();
    assert!(app.debug_expanded_row.is_none());
    app.toggle_debug_row_expand(0);
    assert_eq!(app.debug_expanded_row, Some(0));
    app.toggle_debug_row_expand(0);
    assert!(app.debug_expanded_row.is_none());
}

#[test]
fn test_start_slash_command_help() {
    let mut app = create_test_app();
    app.start_slash_command_help();
    assert!(app.slash_picker.is_some());
}

#[test]
fn test_api_key_input() {
    let mut app = create_test_app();
    app.api_key_input = Some(String::new());
    app.handle_api_key_input('a');
    assert_eq!(app.api_key_input, Some("a".to_string()));
}

#[test]
fn test_api_key_backspace() {
    let mut app = create_test_app();
    app.api_key_input = Some("ab".to_string());
    // First backspace removes 'b', returns true (continue)
    let result = app.handle_api_key_backspace();
    assert!(result);
    assert_eq!(app.api_key_input, Some("a".to_string()));
    // Second backspace removes 'a', returns true (key is now empty but not None)
    let result = app.handle_api_key_backspace();
    assert!(result);
    assert_eq!(app.api_key_input, Some("".to_string()));
    // Third backspace on empty key returns false (signal to exit)
    let result = app.handle_api_key_backspace();
    assert!(!result);
    assert!(app.api_key_input.is_none());
}
