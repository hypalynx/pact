-- Drop unused message_count column from sessions table
-- We now calculate message count dynamically via subquery
ALTER TABLE sessions DROP COLUMN message_count;
