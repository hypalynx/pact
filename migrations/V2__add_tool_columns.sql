-- Add tool-related columns to messages
ALTER TABLE messages ADD COLUMN tool_call_id TEXT;
ALTER TABLE messages ADD COLUMN tool_result_content TEXT;
ALTER TABLE messages ADD COLUMN tool_name TEXT;
