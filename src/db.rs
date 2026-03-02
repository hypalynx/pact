use refinery::embed_migrations;
use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};

embed_migrations!("./migrations");

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

#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub session_id: String,
    pub created_at: String,
    pub working_directory: String,
    pub first_prompt: Option<String>,
    pub message_count: i64,
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

    pub fn run_migrations(&mut self) -> Result<()> {
        migrations::runner()
            .run(&mut self.conn)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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

    pub fn create_session(
        &self,
        session_id: &str,
        working_directory: &str,
        first_prompt: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO sessions (session_id, created_at, working_directory, first_prompt, message_count)
             VALUES (?1, ?2, ?3, ?4, 0)
             ON CONFLICT(session_id) DO UPDATE SET 
                working_directory = excluded.working_directory,
                first_prompt = COALESCE(excluded.first_prompt, first_prompt)",
            params![session_id, now, working_directory, first_prompt],
        )?;
        Ok(())
    }

    pub fn get_sessions_for_directory(&self, working_directory: &str) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, created_at, working_directory, first_prompt, message_count
             FROM sessions WHERE working_directory = ?1 ORDER BY id DESC",
        )?;

        let sessions = stmt.query_map([working_directory], |row| {
            Ok(Session {
                id: row.get(0)?,
                session_id: row.get(1)?,
                created_at: row.get(2)?,
                working_directory: row.get(3)?,
                first_prompt: row.get(4)?,
                message_count: row.get(5)?,
            })
        })?;

        let mut result = Vec::new();
        for session in sessions {
            result.push(session?);
        }
        Ok(result)
    }

    pub fn load_session_messages(&self, session_id: &str) -> Result<Vec<crate::llm::Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, text, is_tool_result, thinking, tool_result_content, tool_call_id, tool_name 
             FROM messages WHERE session_id = ?1 ORDER BY id ASC"
        )?;

        let messages = stmt.query_map([session_id], |row| {
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

    pub fn update_session_first_prompt(&self, session_id: &str, first_prompt: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET first_prompt = ?1 WHERE session_id = ?2",
            params![first_prompt, session_id],
        )?;
        Ok(())
    }

    pub fn update_session_message_count(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET message_count = (SELECT COUNT(*) FROM messages WHERE session_id = ?1)
             WHERE session_id = ?1",
            [session_id],
        )?;
        Ok(())
    }

    pub fn get_session_message_count(&self, session_id: &str) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn clear_session_messages(&self, session_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM messages WHERE session_id = ?1", [session_id])?;
        Ok(())
    }

    pub fn save_message_with_session(
        &self,
        msg: &crate::llm::Message,
        session_id: &str,
        working_directory: &str,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO messages (created_at, role, text, is_tool_result, thinking, tool_result_content, tool_call_id, tool_name, session_id, working_directory)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                now,
                msg.role,
                msg.text,
                if msg.is_tool_result { 1 } else { 0 },
                msg.thinking,
                msg.tool_result_content,
                msg.tool_call_id,
                msg.tool_name,
                session_id,
                working_directory
            ],
        )?;
        Ok(())
    }
}
