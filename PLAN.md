# Pact Development Roadmap

## Quick Reference: What's Next

### ✅ Phase 0: Communication Protocol (COMPLETED Feb 27, 2026)
- ✅ Switched endpoint to `/v1/chat/completions` (OpenAI format)
- ✅ Fixed system message handling (dedicated role: system)
- ✅ Fixed token field names (prompt_tokens/completion_tokens)
- ✅ Added reasoning_content streaming (`LlmEvent::Thinking`)
- ✅ Kept both tool call parsers (Qwen text format + OpenAI structured)
- ✅ Improved tool descriptions and renamed path → filePath

### Current Focus: Communication & Debugging (Feb 27, 2026)
**Priority:** Now implementing debug/logging infrastructure and UI rendering

1. **Remaining Communication Tasks**
   - [ ] Render thinking tokens in UI (dimmed/grey, inline before response)
   - [ ] Message content array format `[{"type": "text", "text": "..."}]` (optional, low priority)

2. **Debug Infrastructure**
   - [ ] Add debug UI inside pact (accessible from control panel)
   - [ ] Migrate from `api.log` to SQLite database storage
   - [ ] Control panel UI for cache/log management

3. **Data Storage**
   - [ ] Move from JSON (`messages.json`) to SQLite
   - [ ] Store debug logs in SQLite (with timestamps, type filtering)
   - [ ] Control panel for managing stored data

4. **Other tasks**
   - [ ] Read user AGENTS.md and AGENTS.md for files in projects
   - [ ] make sure that.. temperature: null isn't being sent if
     the value is null.
   - [ ] message queuing.. should saying "queued" if a message
     has been submitted but not submitted to be processed by
     llama.cpp.

5. **Start writing tests for the parser**

  - [ ] tool_call blocks
  - [ ] thinking blocks
  - [ ] ..markdown? I don't know

---

## Analysis & Findings

### mitmproxy Analysis Results (Feb 26, 2026)

**6 traffic files analyzed** (llama.cpp ↔ opencode communication):
- traffic.json, traffic-reading-readme.json, traffic-review-mainrs.json
- traffic-potentially-dangerous.json, non-editing-4b-traffic.json, yellow-to-red-edit-traffic.json

#### Key Differences: Opencode vs Pact

| Aspect | Opencode | Pact | Gap |
|--------|----------|------|-----|
| Tool Count | 8-11 per request | 1 (read) | Missing bash, glob, grep, edit, write, etc. |
| Tool Descriptions | Full JSON schema + examples | Minimal | Need better descriptions |
| Message Format | `[{"type": "text", "text": "..."}]` | Plain strings | Array format more extensible |
| System Message | Dedicated `role: "system"` message | Prepended to first user | Standard approach cleaner |
| Parameter Names | camelCase (filePath) | snake_case (file_path) | Alignment needed |
| Thinking Tokens | Streams `reasoning_content` separately | Not processed | Need to capture & display |
| Stream Options | `{"include_usage": true}` | Present | Already good |
| Auth | `Bearer none` for local | Same | ✓ Compatible |

#### Tool Patterns from Opencode
- **Execution**: Synchronous, immediate results via SSE delta.tool_calls
- **Tool Results**: Sent as `{"role": "tool", "tool_call_id": "...", "content": "result"}`
- **Error Handling**: All observed calls succeeded; assume failures sent as tool result content
- **Parameters**: Consistently camelCase with clear descriptions and optional fields marked

#### Protocol Constants
- Endpoint: POST `/v1/chat/completions`
- Headers: `Content-Type: application/json`, `Accept: */*`
- Response: Server-Sent Events (text/event-stream)
- Model field: Always `"model": "local"` (or explicit model name)

---

## Communication Protocol Improvements

### 1. Tool Description Enhancement

**Current Problem**: Pact's tool definitions are minimal; opencode provides comprehensive schemas with examples.

**Action Items**:
- [ ] Review opencode's tool definitions from traffic files
- [ ] Extract "best practices" for clear, LLM-friendly descriptions
- [ ] Create tool description guidelines:
  - Clear, single-sentence summary
  - Parameter descriptions: type, constraints, examples
  - Mark optional vs required
  - Note any restrictions (file size limits, path requirements, etc.)
- [ ] Apply to existing `read` tool and incoming `bash`, `glob`, `grep`

**Example: Current `read` vs Improved**
```rust
// Current
"description": "Read file contents"

// Improved
"description": "Read the complete contents of a file. Returns text content up to 64KB; larger files are truncated with a warning message."
```

### 2. Message Formatting

**Sent Messages**:
- [ ] Switch to array format: `{"role": "user", "content": [{"type": "text", "text": "..."}]}`
- [ ] Maintain backward compatibility during transition
- [ ] Benefits: Extensible (images, tool results, etc.), matches OpenAI standard

**Received Responses**:
- [ ] Capture `reasoning_content` from SSE chunks (separate from `content`)
- [ ] Process thinking tokens:
  - Store in message history with role/type tracking
  - Display in UI (toggleable? separate pane? gray/dimmed?)
  - Include in token counts
- [ ] Current: Only process `delta.content`, miss thinking tokens

**Implementation Note**: For now, collect reasoning content but defer UI rendering decision to control panel phase.

### 3. System Message Refactor

**Current Approach**: Prepend system prompt to first user message
```rust
messages.push(Message {
    role: "user",
    content: format!("{}\n\n{}", system_prompt, user_text)
});
```

**Proposed Approach**: Dedicated system message
```rust
messages.push(Message { role: "system", content: system_prompt });
messages.push(Message { role: "user", content: user_text });
```

**Benefits**:
- Standard OpenAI-compatible format
- Models may weight system messages differently
- Cleaner message list logic
- Easier to swap system prompts without touching user content

**Implementation**: Low risk refactor, should not affect model behavior.

---

## Debug Infrastructure (New)

### Problem
- `api.log` is file-based, cumbersome to review mid-session
- No way to view communication errors from inside pact
- Need to tail file separately or switch windows
- No filtering/search capabilities
- Will need frequent debugging as we expand tool support

### Solution: Debug UI inside Pact

**Location**: Accessible via Tab or dedicated key (e.g., Ctrl+D or from control panel)

**Debug View Features**:
1. **Request/Response Log**
   - Timestamp, HTTP method/endpoint
   - Request body (tools, messages count, model)
   - Response status, model name, tokens
   - Search/filter by timestamp, type, error status
   - Collapse/expand full request/response bodies

2. **Communication Errors**
   - Show last N errors prominently
   - SSE parse failures
   - Network errors
   - Tool execution failures

3. **Tool Execution Log**
   - Which tools called, with arguments (truncated)
   - Execution time, success/failure
   - Tool result summary

4. **Control Panel** (accessible from debug view)
   - Clear all logs
   - Toggle log capture (on/off)
   - Export logs to file
   - Filter by log level (info, warning, error)

**Data Storage**: SQLite database (see below)

---

## Data Storage: JSON → SQLite

### Current State
- Messages: `~/.local/share/pact/messages.json` (JSON array)
- Logs: `api.log` in current working directory (plain text)
- Config: `~/.config/pact/pact.yaml` (YAML, keep as-is)

### Proposed: SQLite Database

**Location**: `~/.local/share/pact/pact.db`

**Schema**:

```sql
-- Messages table
CREATE TABLE messages (
  id INTEGER PRIMARY KEY,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  role TEXT NOT NULL,           -- 'user', 'assistant', 'system', 'tool'
  content TEXT NOT NULL,        -- message text
  is_tool_result BOOLEAN,       -- true if tool result
  tool_call_id TEXT,            -- link to tool call if is_tool_result
  tokens_prompt INTEGER,        -- from usage event
  tokens_completion INTEGER,    -- from usage event
  reasoning_content TEXT        -- thinking tokens (optional)
);

-- API request/response log
CREATE TABLE api_logs (
  id INTEGER PRIMARY KEY,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  request_body TEXT NOT NULL,   -- full JSON
  response_status TEXT,         -- status code / error
  response_body TEXT,           -- full response (truncate if huge?)
  tokens_prompt INTEGER,
  tokens_completion INTEGER,
  duration_ms INTEGER,
  error_message TEXT            -- if request failed
);

-- Tool execution log
CREATE TABLE tool_logs (
  id INTEGER PRIMARY KEY,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  tool_name TEXT NOT NULL,
  tool_args TEXT,               -- JSON arguments (truncated)
  success BOOLEAN,
  result_text TEXT,             -- tool output (truncated)
  execution_ms INTEGER,
  error_message TEXT
);

-- Debug settings
CREATE TABLE debug_settings (
  key TEXT PRIMARY KEY,
  value TEXT
);
```

**Benefits**:
- Query-able: filter by timestamp, status, tool name
- Compact: better compression than text/JSON
- Structured: timestamps, types already parsed
- Control panel can query efficiently
- Can export subsets easily (last 100 messages, errors only, etc.)

**Migration**:
- [ ] Load existing `messages.json` on first startup
- [ ] Create DB, populate from JSON
- [ ] Continue using DB for new messages
- [ ] Keep JSON as backup/export option

---

## Control Panel UI

### Access Point
- Modal accessible from main view (e.g., `Ctrl+Shift+P` or menu)
- Or dedicated view (switchable from main conversation)

### Sections

#### 1. Message History
- List of saved conversations
- Quick view: message count, last message date, token usage
- Actions: Load conversation, delete, export
- Filter by date range, model used

#### 2. Debug Logs
- Last 50 API calls (scroll to see more)
- Each entry: timestamp, endpoint, status, duration, tokens
- Click to expand full request/response
- Filter: error only, time range, tool filter
- Control: Clear all logs, pause logging, export

#### 3. Tool Execution History
- Recent tool calls and results
- Each: tool name, args, success/failure, timing
- Filter: tool type, status, time range
- Good for debugging tool failures

#### 4. Settings
- Log capture: on/off
- Log retention: days (auto-cleanup old logs)
- Export format: JSON, CSV, SQL
- Database size: show current size, option to compact/vacuum

#### 5. Status
- DB file size
- Message count
- Total tokens used (lifetime)
- Last request timestamp

---

## Implementation Phases

### Phase 0: Communication Protocol (NEXT)
**Goal**: Ensure pact sends/receives data like opencode, ready for tool expansion

1. Tool description audit
   - [ ] Review opencode schemas from traffic files
   - [ ] Define description guidelines
   - [ ] Update existing tools

2. Message format improvements
   - [ ] Capture reasoning_content from SSE
   - [ ] Switch to array format (messages)
   - [ ] System message refactor

3. Testing
   - [ ] Verify compatible with same LLM server
   - [ ] Compare requests to opencode traffic
   - [ ] No regression in existing functionality

**Estimated Scope**: 5-8 hours (includes refactoring + testing)

### Phase 1: Debug Infrastructure
**Goal**: Have debug UI + SQLite ready before expanding tools

1. SQLite migration
   - [ ] Design schema
   - [ ] Create database module
   - [ ] Load existing messages.json on startup
   - [ ] Update app to save messages to DB

2. Debug UI
   - [ ] Create debug view (ratatui panel)
   - [ ] Query recent API logs
   - [ ] Display request/response
   - [ ] Show errors

3. Control panel
   - [ ] Message history view
   - [ ] Debug log viewer
   - [ ] Settings/cache management
   - [ ] Data export

**Estimated Scope**: 10-15 hours

### Phase 2: Core Tools
**Goal**: bash, glob, grep with proper logging

1. Implement bash tool
2. Implement glob tool
3. Implement grep tool
4. Test with debug UI to catch edge cases

**Estimated Scope**: 6-10 hours (benefits from phases 0-1 being solid)

### Phase 3: UX Enhancements (Lower Priority)
- Slash commands (/clear, etc.)
- @ file references
- Better tool execution feedback

---

## Testing Strategy

### Unit Tests
- Tool execution (success, errors, timeouts)
- Message formatting (array structure, role handling)
- SQLite schema queries
- Reasoning content capture

### Integration Tests
- Full message flow (send → receive → store in DB)
- Tool invocation → execution → result storage
- Debug log querying
- Message history loading

### Manual Testing
- Verify requests match opencode format
- Test debug UI with various log scenarios
- Control panel functionality (filters, export)
- Performance with large message history (1000+ messages)

---

## Known Gaps & Risks

- **UI Complexity**: Control panel is significant new surface area
- **Database Migration**: Existing `messages.json` must not be lost
- **Performance**: Querying SQLite with many messages (>10k)
- **Thinking Tokens**: UI rendering approach TBD (impacts user experience)
- **Backwards Compatibility**: Message format change might affect saved data

---

## Decision Points (Before Proceeding)

1. **Debug UI Approach**
   - Option A: Modal overlay (like control panel)
   - Option B: Dedicated switchable view (like chat view)
   - Option C: Side panel (if screen space permits)

2. **Thinking Token Display**
   - Option A: Hide by default, show in control panel
   - Option B: Show as separate "thinking:" section in messages
   - Option C: Collapse/expand arrows next to assistant messages
   - Option D: Separate "reasoning" pane

3. **Log Retention**
   - Keep all logs indefinitely?
   - Auto-delete after N days?
   - Manual management in control panel?

4. **Message Format Migration**
   - All-at-once (breaking change if loading old history)?
   - Gradual (support both formats)?
   - Just for new messages, old format grandfathered?
