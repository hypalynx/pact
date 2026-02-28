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
