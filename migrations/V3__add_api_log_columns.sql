-- Add columns to api_logs for richer logging
ALTER TABLE api_logs ADD COLUMN response_body TEXT;
ALTER TABLE api_logs ADD COLUMN full_response TEXT;
ALTER TABLE api_logs ADD COLUMN tokens_prompt INTEGER;
ALTER TABLE api_logs ADD COLUMN tokens_completion INTEGER;
ALTER TABLE api_logs ADD COLUMN duration_ms INTEGER;
ALTER TABLE api_logs ADD COLUMN error_message TEXT;
ALTER TABLE api_logs ADD COLUMN model_name TEXT;
ALTER TABLE api_logs ADD COLUMN provider TEXT;
