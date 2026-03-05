use pact::db::Db;
use pact::llm::Message;
use rusqlite::Connection;

fn create_temp_db() -> Db {
    let conn = Connection::open_in_memory().expect("Failed to create in-memory DB");

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -4000;
        PRAGMA foreign_keys = ON;
        PRAGMA analysis_limit = 400;
        "#,
    )
    .expect("Failed to set pragmas");

    Db { conn }
}

#[test]
fn test_run_migrations() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to run migrations");

    let mut stmt = db
        .conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('messages', 'api_logs')",
        )
        .expect("Failed to query schema");

    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("Failed to iterate tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to collect tables");

    assert_eq!(tables.len(), 2);
    assert!(tables.contains(&"messages".to_string()));
    assert!(tables.contains(&"api_logs".to_string()));
}

#[test]
fn test_save_and_load_message() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let msg = Message {
        role: "user".to_string(),
        text: "Hello, world!".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    db.save_message(&msg, "test-session", "/tmp")
        .expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].text, "Hello, world!");
    assert!(!messages[0].is_tool_result);
}

#[test]
fn test_save_tool_result_message() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let msg = Message {
        role: "user".to_string(),
        text: "Tool output".to_string(),
        is_tool_result: true,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    db.save_message(&msg, "test-session", "/tmp")
        .expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);
    assert!(messages[0].is_tool_result);
}

#[test]
fn test_save_message_with_thinking() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let msg = Message {
        role: "assistant".to_string(),
        text: "Response".to_string(),
        is_tool_result: false,
        thinking: Some("Let me think...".to_string()),
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    db.save_message(&msg, "test-session", "/tmp")
        .expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].thinking, Some("Let me think...".to_string()));
}

#[test]
fn test_clear_messages() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let msg = Message {
        role: "user".to_string(),
        text: "Test".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    db.save_message(&msg, "test-session", "/tmp")
        .expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);

    db.clear_messages().expect("Failed to clear messages");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 0);
}

#[test]
fn test_save_and_load_api_log() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let response = r#"{"result":"ok"}"#;

    db.save_api_log(
        request,
        Some(response),
        None,
        100,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].request_body, request);
    assert_eq!(logs[0].response_body, Some(response.to_string()));
    assert_eq!(logs[0].duration_ms, Some(100));
    assert!(logs[0].error_message.is_none());
}

#[test]
fn test_save_api_log_with_error() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let error = "Connection timeout";

    db.save_api_log(
        request,
        None,
        None,
        5000,
        Some(error),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].error_message, Some(error.to_string()));
}

#[test]
fn test_clear_api_logs() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    db.save_api_log(
        r#"{"test":true}"#,
        None,
        None,
        50,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);

    db.clear_api_logs().expect("Failed to clear API logs");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 0);
}

#[test]
fn test_recent_api_logs_limit() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    for i in 0..5 {
        db.save_api_log(
            &format!(r#"{{"id":{}}}"#, i),
            None,
            None,
            50,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to save API log");
    }

    let logs = db.recent_api_logs(3).expect("Failed to load API logs");
    assert_eq!(logs.len(), 3);
}

#[test]
fn test_load_messages_ordered() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let msg1 = Message {
        role: "user".to_string(),
        text: "First".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    let msg2 = Message {
        role: "assistant".to_string(),
        text: "Second".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    db.save_message(&msg1, "test-session", "/tmp")
        .expect("Failed to save message");
    db.save_message(&msg2, "test-session", "/tmp")
        .expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text, "First");
    assert_eq!(messages[1].text, "Second");
}

#[test]
fn test_save_api_log_with_all_fields() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let response = r#"{"result":"ok"}"#;
    let full_response = r#"{"full":"data"}"#;
    let error = "Test error";
    let model_name = "qwen3-coder-30b";

    db.save_api_log(
        request,
        Some(response),
        Some(full_response),
        150,
        Some(error),
        Some(model_name),
        Some("test-provider"),
        None,
        None,
        None,
    )
    .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].request_body, request);
    assert_eq!(logs[0].response_body, Some(response.to_string()));
    assert_eq!(logs[0].full_response, Some(full_response.to_string()));
    assert_eq!(logs[0].duration_ms, Some(150));
    assert_eq!(logs[0].error_message, Some(error.to_string()));
    assert_eq!(logs[0].model_name, Some(model_name.to_string()));
}

#[test]
fn test_save_api_log_with_model_name() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let model_name = "moonshot-v1-8k";

    db.save_api_log(
        request,
        None,
        None,
        50,
        None,
        Some(model_name),
        None,
        None,
        None,
        None,
    )
    .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model_name, Some(model_name.to_string()));
}

#[test]
fn test_save_api_log_with_none_fields() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;

    db.save_api_log(request, None, None, 100, None, None, None, None, None, None)
        .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].request_body, request);
    assert_eq!(logs[0].response_body, None);
    assert_eq!(logs[0].model_name, None);
    assert_eq!(logs[0].full_response, None);
    assert_eq!(logs[0].error_message, None);
}

#[test]
fn test_load_empty_messages() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 0);
}

#[test]
fn test_recent_api_logs_empty() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 0);
}

#[test]
fn test_multiple_messages_with_mixed_types() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    let user_msg = Message {
        role: "user".to_string(),
        text: "Hello".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    let tool_msg = Message {
        role: "assistant".to_string(),
        text: "Tool result".to_string(),
        is_tool_result: true,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    let assistant_msg = Message {
        role: "assistant".to_string(),
        text: "Response".to_string(),
        is_tool_result: false,
        thinking: Some("Thinking...".to_string()),
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };

    db.save_message(&user_msg, "test-session", "/tmp")
        .expect("Failed to save message");
    db.save_message(&tool_msg, "test-session", "/tmp")
        .expect("Failed to save message");
    db.save_message(&assistant_msg, "test-session", "/tmp")
        .expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 3);
    assert!(!messages[0].is_tool_result);
    assert!(messages[1].is_tool_result);
    assert!(!messages[2].is_tool_result);
    assert_eq!(messages[2].thinking, Some("Thinking...".to_string()));
}

#[test]
fn test_api_log_with_tokens() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    // Manually insert an API log with token counts using direct SQL
    let request = r#"{"model":"test"}"#;
    db.conn.execute(
        "INSERT INTO api_logs (created_at, request_body, response_body, full_response, tokens_prompt, tokens_completion, duration_ms, error_message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            chrono::Local::now().to_rfc3339(),
            request,
            Some(r#"{"result":"ok"}"#),
            None::<String>,
            Some(100i64),
            Some(50i64),
            75i64,
            None::<String>,
        ],
    ).expect("Failed to insert API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].tokens_prompt, Some(100));
    assert_eq!(logs[0].tokens_completion, Some(50));
}

#[test]
fn test_db_open() {
    // Test that db open works (uses temp in-memory db already)
    let db = create_temp_db();
    // Just verify it doesn't fail
    assert!(db.conn.execute_batch("SELECT 1;").is_ok());
}

#[test]
fn test_get_sessions_for_directory_returns_sessions_with_messages() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    // Create two sessions in the same working directory
    db.create_session("session-1", "/tmp", Some("first prompt"))
        .expect("Failed to create session 1");
    db.create_session("session-2", "/tmp", Some("second prompt"))
        .expect("Failed to create session 2");

    // Initially, no sessions should be returned (no messages yet)
    let sessions = db
        .get_sessions_for_directory("/tmp", 10)
        .expect("Failed to get sessions");
    assert_eq!(sessions.len(), 0);

    // Save a message to session-1
    let msg = Message {
        role: "user".to_string(),
        text: "Hello".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };
    db.save_message(&msg, "session-1", "/tmp")
        .expect("Failed to save message");

    // Now session-1 should appear in the list with 1 message
    let sessions = db
        .get_sessions_for_directory("/tmp", 10)
        .expect("Failed to get sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "session-1");
    assert_eq!(sessions[0].message_count, 1);
    assert_eq!(sessions[0].first_prompt, Some("first prompt".to_string()));

    // Save another message to session-1
    let msg2 = Message {
        role: "assistant".to_string(),
        text: "Hi there".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };
    db.save_message(&msg2, "session-1", "/tmp")
        .expect("Failed to save message 2");

    // Save a message to session-2
    let msg3 = Message {
        role: "user".to_string(),
        text: "Another session".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };
    db.save_message(&msg3, "session-2", "/tmp")
        .expect("Failed to save message 3");

    // Both sessions should now appear, ordered by id DESC (most recent first)
    let sessions = db
        .get_sessions_for_directory("/tmp", 10)
        .expect("Failed to get sessions");
    assert_eq!(sessions.len(), 2);
    // session-2 was created second, so it should be first (higher id)
    assert_eq!(sessions[0].session_id, "session-2");
    assert_eq!(sessions[0].message_count, 1);
    assert_eq!(sessions[1].session_id, "session-1");
    assert_eq!(sessions[1].message_count, 2);
}

#[test]
fn test_get_sessions_for_directory_limit() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    // Create 3 sessions with messages
    for i in 0..3 {
        let session_id = format!("session-{}", i);
        db.create_session(&session_id, "/tmp", None)
            .expect("Failed to create session");
        let msg = Message {
            role: "user".to_string(),
            text: format!("Message {}", i),
            is_tool_result: false,
            thinking: None,
            tool_result_content: None,
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        };
        db.save_message(&msg, &session_id, "/tmp")
            .expect("Failed to save message");
    }

    // Get only 2 sessions
    let sessions = db
        .get_sessions_for_directory("/tmp", 2)
        .expect("Failed to get sessions");
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_get_sessions_for_directory_different_working_dirs() {
    let mut db = create_temp_db();
    db.run_migrations().expect("Failed to init schema");

    // Create sessions in different directories
    db.create_session("session-a", "/home/user/project1", None)
        .expect("Failed to create session");
    db.create_session("session-b", "/home/user/project2", None)
        .expect("Failed to create session");

    // Add messages to both
    let msg = Message {
        role: "user".to_string(),
        text: "Hello".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
        tool_call_id: None,
        tool_name: None,
        tool_calls: None,
    };
    db.save_message(&msg, "session-a", "/home/user/project1")
        .expect("Failed to save message");
    db.save_message(&msg, "session-b", "/home/user/project2")
        .expect("Failed to save message");

    // Query each directory separately
    let sessions1 = db
        .get_sessions_for_directory("/home/user/project1", 10)
        .expect("Failed to get sessions");
    assert_eq!(sessions1.len(), 1);
    assert_eq!(sessions1[0].session_id, "session-a");

    let sessions2 = db
        .get_sessions_for_directory("/home/user/project2", 10)
        .expect("Failed to get sessions");
    assert_eq!(sessions2.len(), 1);
    assert_eq!(sessions2[0].session_id, "session-b");
}
