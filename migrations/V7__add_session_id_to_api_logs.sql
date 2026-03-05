-- Add session_id to api_logs for log grouping
ALTER TABLE api_logs ADD COLUMN session_id TEXT;
