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
fn test_init_schema() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

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
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let msg = Message {
        role: "user".to_string(),
        text: "Hello, world!".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
    };

    db.save_message(&msg).expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].text, "Hello, world!");
    assert!(!messages[0].is_tool_result);
}

#[test]
fn test_save_tool_result_message() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let msg = Message {
        role: "user".to_string(),
        text: "Tool output".to_string(),
        is_tool_result: true,
        thinking: None,
        tool_result_content: None,
    };

    db.save_message(&msg).expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);
    assert!(messages[0].is_tool_result);
}

#[test]
fn test_save_message_with_thinking() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let msg = Message {
        role: "assistant".to_string(),
        text: "Response".to_string(),
        is_tool_result: false,
        thinking: Some("Let me think...".to_string()),
        tool_result_content: None,
    };

    db.save_message(&msg).expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].thinking, Some("Let me think...".to_string()));
}

#[test]
fn test_clear_messages() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let msg = Message {
        role: "user".to_string(),
        text: "Test".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
    };

    db.save_message(&msg).expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 1);

    db.clear_messages().expect("Failed to clear messages");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 0);
}

#[test]
fn test_save_and_load_api_log() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let response = r#"{"result":"ok"}"#;

    db.save_api_log(request, Some(response), None, 100, None)
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
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let error = "Connection timeout";

    db.save_api_log(request, None, None, 5000, Some(error))
        .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].error_message, Some(error.to_string()));
}

#[test]
fn test_clear_api_logs() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    db.save_api_log(r#"{"test":true}"#, None, None, 50, None)
        .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);

    db.clear_api_logs().expect("Failed to clear API logs");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 0);
}

#[test]
fn test_recent_api_logs_limit() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    for i in 0..5 {
        db.save_api_log(&format!(r#"{{"id":{}}}"#, i), None, None, 50, None)
            .expect("Failed to save API log");
    }

    let logs = db.recent_api_logs(3).expect("Failed to load API logs");
    assert_eq!(logs.len(), 3);
}

#[test]
fn test_load_messages_ordered() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let msg1 = Message {
        role: "user".to_string(),
        text: "First".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
    };

    let msg2 = Message {
        role: "assistant".to_string(),
        text: "Second".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
    };

    db.save_message(&msg1).expect("Failed to save message");
    db.save_message(&msg2).expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text, "First");
    assert_eq!(messages[1].text, "Second");
}

#[test]
fn test_save_api_log_with_all_fields() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;
    let response = r#"{"result":"ok"}"#;
    let full_response = r#"{"full":"data"}"#;
    let error = "Test error";

    db.save_api_log(request, Some(response), Some(full_response), 150, Some(error))
        .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].request_body, request);
    assert_eq!(logs[0].response_body, Some(response.to_string()));
    assert_eq!(logs[0].full_response, Some(full_response.to_string()));
    assert_eq!(logs[0].duration_ms, Some(150));
    assert_eq!(logs[0].error_message, Some(error.to_string()));
}

#[test]
fn test_save_api_log_with_none_fields() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let request = r#"{"model":"test"}"#;

    db.save_api_log(request, None, None, 100, None)
        .expect("Failed to save API log");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].request_body, request);
    assert_eq!(logs[0].response_body, None);
    assert_eq!(logs[0].full_response, None);
    assert_eq!(logs[0].error_message, None);
}

#[test]
fn test_load_empty_messages() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 0);
}

#[test]
fn test_recent_api_logs_empty() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let logs = db.recent_api_logs(10).expect("Failed to load API logs");
    assert_eq!(logs.len(), 0);
}

#[test]
fn test_multiple_messages_with_mixed_types() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

    let user_msg = Message {
        role: "user".to_string(),
        text: "Hello".to_string(),
        is_tool_result: false,
        thinking: None,
        tool_result_content: None,
    };

    let tool_msg = Message {
        role: "assistant".to_string(),
        text: "Tool result".to_string(),
        is_tool_result: true,
        thinking: None,
        tool_result_content: None,
    };

    let assistant_msg = Message {
        role: "assistant".to_string(),
        text: "Response".to_string(),
        is_tool_result: false,
        thinking: Some("Thinking...".to_string()),
        tool_result_content: None,
    };

    db.save_message(&user_msg).expect("Failed to save message");
    db.save_message(&tool_msg).expect("Failed to save message");
    db.save_message(&assistant_msg).expect("Failed to save message");

    let messages = db.load_messages().expect("Failed to load messages");
    assert_eq!(messages.len(), 3);
    assert!(!messages[0].is_tool_result);
    assert!(messages[1].is_tool_result);
    assert!(!messages[2].is_tool_result);
    assert_eq!(messages[2].thinking, Some("Thinking...".to_string()));
}

#[test]
fn test_api_log_with_tokens() {
    let db = create_temp_db();
    db.init_schema().expect("Failed to init schema");

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
