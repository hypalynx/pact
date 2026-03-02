-- Add session support
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY,
    session_id TEXT UNIQUE NOT NULL,
    created_at TEXT NOT NULL,
    working_directory TEXT NOT NULL,
    first_prompt TEXT,
    message_count INTEGER DEFAULT 0
);

-- Add session columns to messages
ALTER TABLE messages ADD COLUMN session_id TEXT;
ALTER TABLE messages ADD COLUMN working_directory TEXT;
