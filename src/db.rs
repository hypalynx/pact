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
    pub provider: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub is_active: bool,
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
                tool_call_id TEXT,
                tool_result_content TEXT,
                tool_name TEXT
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
                model_name TEXT,
                provider TEXT
            );

            CREATE TABLE IF NOT EXISTS providers (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                endpoint TEXT NOT NULL,
                api_key TEXT,
                default_model TEXT,
                is_active INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS provider_models (
                id INTEGER PRIMARY KEY,
                provider_name TEXT NOT NULL,
                model_id TEXT NOT NULL,
                UNIQUE(provider_name, model_id)
            );
            "#,
        )?;

        // Migrate existing tables if needed (add new columns if they don't exist)
        let _ = self
            .conn
            .execute("ALTER TABLE messages ADD COLUMN tool_call_id TEXT", []);
        let _ = self.conn.execute(
            "ALTER TABLE messages ADD COLUMN tool_result_content TEXT",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE messages ADD COLUMN tool_name TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE api_logs ADD COLUMN provider TEXT", []);

        // Create provider_models table if it doesn't exist (for backwards compatibility)
        let _ = self.conn.execute(
            "CREATE TABLE IF NOT EXISTS provider_models (
                id INTEGER PRIMARY KEY,
                provider_name TEXT NOT NULL,
                model_id TEXT NOT NULL,
                UNIQUE(provider_name, model_id)
            )",
            [],
        );

        // Run PRAGMA optimize to analyze tables if they have any data
        self.conn.execute_batch("PRAGMA optimize;")?;
        Ok(())
    }

    pub fn get_providers(&self) -> Result<Vec<Provider>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, endpoint, api_key, default_model, is_active FROM providers ORDER BY id"
        )?;

        let providers = stmt.query_map([], |row| {
            Ok(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                endpoint: row.get(2)?,
                api_key: row.get(3)?,
                default_model: row.get(4)?,
                is_active: row.get::<_, i64>(5)? != 0,
            })
        })?;

        let mut result = Vec::new();
        for provider in providers {
            result.push(provider?);
        }
        Ok(result)
    }

    pub fn get_active_provider(&self) -> Result<Option<Provider>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, endpoint, api_key, default_model, is_active FROM providers WHERE is_active = 1 LIMIT 1"
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                endpoint: row.get(2)?,
                api_key: row.get(3)?,
                default_model: row.get(4)?,
                is_active: true,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn set_active_provider(&self, name: &str) -> Result<()> {
        // Deactivate all providers first
        self.conn
            .execute("UPDATE providers SET is_active = 0", [])?;
        // Activate the specified provider
        self.conn
            .execute("UPDATE providers SET is_active = 1 WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn add_provider(
        &self,
        name: &str,
        endpoint: &str,
        api_key: Option<&str>,
        default_model: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO providers (name, endpoint, api_key, default_model, is_active) VALUES (?1, ?2, ?3, ?4, 0)",
            params![name, endpoint, api_key, default_model],
        )?;
        Ok(())
    }

    pub fn delete_provider(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM providers WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn provider_exists(&self, name: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM providers WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_provider_models(&self, provider_name: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT model_id FROM provider_models WHERE provider_name = ?1 ORDER BY id")?;

        let models = stmt.query_map([provider_name], |row| row.get::<_, String>(0))?;

        let mut result = Vec::new();
        for model in models {
            result.push(model?);
        }
        Ok(result)
    }

    pub fn set_provider_models(&self, provider_name: &str, models: &[String]) -> Result<()> {
        // Clear existing models for this provider
        self.conn.execute(
            "DELETE FROM provider_models WHERE provider_name = ?1",
            [provider_name],
        )?;

        // Insert new models
        for model in models {
            self.conn.execute(
                "INSERT INTO provider_models (provider_name, model_id) VALUES (?1, ?2)",
                params![provider_name, model],
            )?;
        }
        Ok(())
    }

    pub fn update_provider_model(&self, name: &str, model: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE providers SET default_model = ?1 WHERE name = ?2",
            [model, name],
        )?;
        Ok(())
    }

    pub fn update_provider_api_key(&self, name: &str, api_key: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE providers SET api_key = ?1 WHERE name = ?2",
            [api_key, name],
        )?;
        Ok(())
    }

    pub fn save_message(&self, msg: &crate::llm::Message) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO messages (created_at, role, text, is_tool_result, thinking, tool_result_content, tool_call_id, tool_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                now,
                msg.role,
                msg.text,
                if msg.is_tool_result { 1 } else { 0 },
                msg.thinking,
                msg.tool_result_content,
                msg.tool_call_id,
                msg.tool_name
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_api_log(
        &self,
        body: &str,
        response: Option<&str>,
        full_response: Option<&str>,
        duration_ms: u64,
        error: Option<&str>,
        model_name: Option<&str>,
        provider: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO api_logs (created_at, request_body, response_body, full_response, duration_ms, error_message, model_name, provider)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![now, body, response, full_response, duration_ms as i64, error, model_name, provider],
        )?;
        Ok(())
    }

    pub fn recent_api_logs(&self, limit: usize) -> Result<Vec<ApiLogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, request_body, response_body, full_response, tokens_prompt, tokens_completion, duration_ms, error_message, model_name, provider
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
                provider: row.get(10)?,
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
            .prepare("SELECT role, text, is_tool_result, thinking, tool_result_content, tool_call_id, tool_name FROM messages ORDER BY id ASC")?;

        let messages = stmt.query_map([], |row| {
            Ok(crate::llm::Message {
                role: row.get(0)?,
                text: row.get(1)?,
                is_tool_result: row.get::<_, i64>(2)? != 0,
                thinking: row.get(3)?,
                tool_result_content: row.get(4)?,
                tool_call_id: row.get(5)?,
                tool_name: row.get(6)?,
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
