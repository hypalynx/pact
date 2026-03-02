-- Initial schema
-- Messages table for chat history
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    created_at TEXT NOT NULL,
    role TEXT NOT NULL,
    text TEXT NOT NULL,
    is_tool_result INTEGER NOT NULL DEFAULT 0,
    thinking TEXT
);

-- API request/response logging
CREATE TABLE api_logs (
    id INTEGER PRIMARY KEY,
    created_at TEXT NOT NULL,
    request_body TEXT NOT NULL
);

-- LLM provider configuration
CREATE TABLE providers (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    endpoint TEXT NOT NULL,
    api_key TEXT,
    default_model TEXT,
    is_active INTEGER NOT NULL DEFAULT 0
);

-- Available models per provider
CREATE TABLE provider_models (
    id INTEGER PRIMARY KEY,
    provider_name TEXT NOT NULL,
    model_id TEXT NOT NULL,
    UNIQUE(provider_name, model_id)
);
