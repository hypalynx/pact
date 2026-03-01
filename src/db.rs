use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};

pub struct Db {
    pub conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLogEntry {
    pub id: i64,
    pub created_at: String,
    pub request_body: String,
    pub response_body: Option<String>,
    pub full_response: Option<String>,
    pub tokens_prompt: Option<i64>,
    pub tokens_completion: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub model_name: Option<String>,
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = crate::utils::db_path();
        let conn = Connection::open(path)?;

        // Enable WAL mode and optimize for CLI usage
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -4000;
            PRAGMA foreign_keys = ON;
            PRAGMA analysis_limit = 400;
            "#,
        )?;

        Ok(Db { conn })
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY,
                created_at TEXT NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                is_tool_result INTEGER NOT NULL DEFAULT 0,
                thinking TEXT,
                tool_call_id TEXT
            );

            CREATE TABLE IF NOT EXISTS api_logs (
                id INTEGER PRIMARY KEY,
                created_at TEXT NOT NULL,
                request_body TEXT NOT NULL,
                response_body TEXT,
                full_response TEXT,
                tokens_prompt INTEGER,
                tokens_completion INTEGER,
                duration_ms INTEGER,
                error_message TEXT,
                model_name TEXT
            );
            "#,
        )?;

        // Migrate existing tables if needed (add tool_call_id column if it doesn't exist)
        let _ = self
            .conn
            .execute("ALTER TABLE messages ADD COLUMN tool_call_id TEXT", []);

        // Run PRAGMA optimize to analyze tables if they have any data
        self.conn.execute_batch("PRAGMA optimize;")?;
        Ok(())
    }

    pub fn save_message(&self, msg: &crate::llm::Message) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO messages (created_at, role, text, is_tool_result, thinking)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                now,
                msg.role,
                msg.text,
                if msg.is_tool_result { 1 } else { 0 },
                msg.thinking
            ],
        )?;
        Ok(())
    }

    pub fn save_api_log(
        &self,
        body: &str,
        response: Option<&str>,
        full_response: Option<&str>,
        duration_ms: u64,
        error: Option<&str>,
        model_name: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO api_logs (created_at, request_body, response_body, full_response, duration_ms, error_message, model_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![now, body, response, full_response, duration_ms as i64, error, model_name],
        )?;
        Ok(())
    }

    pub fn recent_api_logs(&self, limit: usize) -> Result<Vec<ApiLogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, request_body, response_body, full_response, tokens_prompt, tokens_completion, duration_ms, error_message, model_name
             FROM api_logs ORDER BY id DESC LIMIT ?1",
        )?;

        let entries = stmt.query_map(params![limit as i64], |row| {
            Ok(ApiLogEntry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                request_body: row.get(2)?,
                response_body: row.get(3)?,
                full_response: row.get(4)?,
                tokens_prompt: row.get(5)?,
                tokens_completion: row.get(6)?,
                duration_ms: row.get(7)?,
                error_message: row.get(8)?,
                model_name: row.get(9)?,
            })
        })?;

        let mut result = Vec::new();
        for entry in entries {
            result.push(entry?);
        }
        Ok(result)
    }

    pub fn load_messages(&self) -> Result<Vec<crate::llm::Message>> {
        let mut stmt = self
            .conn
            .prepare("SELECT role, text, is_tool_result, thinking FROM messages ORDER BY id ASC")?;

        let messages = stmt.query_map([], |row| {
            Ok(crate::llm::Message {
                role: row.get(0)?,
                text: row.get(1)?,
                is_tool_result: row.get::<_, i64>(2)? != 0,
                thinking: row.get(3)?,
                tool_result_content: None,
                tool_call_id: None,
            })
        })?;

        let mut result = Vec::new();
        for msg in messages {
            result.push(msg?);
        }
        Ok(result)
    }

    pub fn clear_messages(&self) -> Result<()> {
        self.conn.execute("DELETE FROM messages", [])?;
        Ok(())
    }

    pub fn clear_api_logs(&self) -> Result<()> {
        self.conn.execute("DELETE FROM api_logs", [])?;
        Ok(())
    }
}
