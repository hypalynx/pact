# Pact Testing Plan

## Overview
This document outlines the testing strategy for Pact TUI as the codebase grows. Tests should focus on behavior and integration points rather than implementation details.

## Unit Tests

### `config.rs`
- **Mode merging**: User config overrides defaults correctly
  - Test: build mode (cyan) can be overridden to (magenta)
  - Test: New custom mode (e.g., "chat") is added alongside defaults
  - Test: Empty user config uses all defaults
- **Color parsing**: Invalid colors fall back gracefully
  - Test: "invalid_color" → defaults to white
  - Test: Case insensitivity: "CYAN", "Cyan", "cyan" all work
- **Config loading**
  - Test: Missing config file → defaults loaded
  - Test: Malformed YAML → defaults loaded
  - Test: Valid YAML with partial overrides merges correctly

### `text.rs`
- **Text wrapping**
  - Test: Text wrapping respects width limits
  - Test: Word boundaries preserved (no mid-word breaks)
  - Test: Newlines in input create separate lines
  - Test: Single long word wider than width is not broken
  - Test: Empty strings handled
- **Markdown parsing**
  - Test: **bold** text parsed correctly
  - Test: *italic* text parsed correctly
  - Test: `code` text parsed correctly
  - Test: Mixed formatting works
  - Test: Invalid markdown handled gracefully

### `utils.rs`
- **Color parsing** (already in config, but test independently)
  - Test: All supported color names work
  - Test: Variants (dark_gray, darkgray, dark-gray) all work
  - Test: Invalid color defaults to white
- **Server info parsing**
  - Test: Standard `/v1/models` response with data array parsed
  - Test: Single model response parsed
  - Test: Missing fields use sensible defaults
  - Test: Network error returns unknown model, default context

## Integration Tests

### Mode System
- **Mode cycling**
  - Test: Tab key cycles through all modes in config order
  - Test: Cycling wraps around to first mode
  - Test: Temperature updates when mode changes
  - Test: Color updates when mode changes
  - Test: System prompt updates when mode changes
- **Mode persistence**
  - Test: Default mode from config is loaded on startup

### Message Handling
- **User message submission**
  - Test: Ctrl+J creates newline in input
  - Test: Enter submits message and clears input
  - Test: Message saved to history
  - Test: System prompt prepended to LLM request
- **LLM response streaming**
  - Test: Token events accumulate into pending_response
  - Test: Done event moves pending_response to messages
  - Test: Error event shows error message
  - Test: Usage events update token counters

### Server Info Refresh
- **Periodic refresh**
  - Test: Server info queried on startup
  - Test: Server info refreshed every ~3 seconds
  - Test: Model name updates in status bar when server changes
  - Test: Context window updates when model changes
- **Network resilience**
  - Test: Failed refresh doesn't crash app
  - Test: Model name remains unchanged on failed refresh
  - Test: Continues retrying after transient failures

### Text Input/History
- **Readline commands**
  - Test: Ctrl+A moves cursor to start
  - Test: Ctrl+E moves cursor to end
  - Test: Ctrl+W kills word backward
  - Test: Ctrl+U kills line
  - Test: Up/Down arrows navigate history
  - Test: History preserved across sessions (from messages.json)
- **Multi-line input**
  - Test: Ctrl+J adds newline
  - Test: Backspace deletes characters correctly
  - Test: Cursor position tracked correctly with newlines

### Scrolling
- **Mouse wheel**
  - Test: Scroll up/down changes scroll_offset
  - Test: At bottom → user_scrolled = false
  - Test: Manually scrolled → auto-scroll disabled until bottom
- **Keyboard**
  - Test: Page Up/Down scroll 3 lines
  - Test: Scrollbar position reflects scroll offset

## End-to-End Tests

### Full workflow
1. Start app → loads config, detects model, shows status bar
2. Switch mode (Tab) → system prompt/temperature/color change
3. Type message with Ctrl+J newlines → submit with Enter
4. Receive streaming response → tokens accumulate, auto-scroll
5. Scroll up → disable auto-scroll, scroll position maintained
6. Switch modes → temperature/prompt change for next message
7. Restart LLM with different model → detected within 3 seconds

## Test Execution Plan

1. **Unit tests first** - Fast feedback on individual functions
2. **Integration tests** - Verify modules work together
3. **E2E tests** - Catch regressions in full workflow

## Mock/Fixture Strategy

- Mock LLM server responses (SSE format)
- Mock filesystem for config/history
- Create test configs with various combinations
- Pre-made message histories for testing

## Tool Use Implementation

### Overview
Add OpenAI-compatible tool/function calling to Pact. LLM can request tools like read, edit, bash to interact with the filesystem and system. Results are sent back to the LLM in the conversation.

### Tools to Implement (Priority Order)

1. **read** (NEXT)
   - Read file contents at given path
   - Return file contents or error message
   - Max size limit? (e.g., 64KB per read to avoid huge responses)

2. **bash** (AFTER read)
   - Execute shell commands
   - Capture stdout/stderr
   - Return exit code + output
   - Security: Consider sandboxing/restrictions

3. **edit**
   - Modify file at path (similar to our Edit tool)
   - Line-based or full replacement

4. **glob**
   - Find files matching pattern
   - Return list of paths

5. **grep**
   - Search file contents for patterns
   - Return matching lines with context

6. **todowrite**
   - Create/update todo items
   - Store in standard location

### Tool Schema Format (OpenAI-compatible)

```json
{
  "type": "function",
  "function": {
    "name": "read",
    "description": "Read contents of a file",
    "parameters": {
      "type": "object",
      "properties": {
        "path": {
          "type": "string",
          "description": "Absolute file path to read"
        }
      },
      "required": ["path"]
    }
  }
}
```

### Implementation Plan

1. **New module: `tools.rs`**
   - Define tool schemas
   - Implement tool execution functions
   - Handle tool responses

2. **Update `llm.rs`**
   - Include tools array in request
   - Parse `tool_use` / `function_call` responses
   - Return tool calls to main loop instead of just tokens

3. **Update `app.rs` / event loop**
   - Handle tool call responses
   - Execute tools synchronously (for now)
   - Send tool results back to LLM as assistant message
   - Continue conversation

4. **New LlmEvent variant**
   - `ToolCall { name, args }` - LLM wants to use a tool
   - Display in UI or execute immediately?

### Response Flow

1. User sends message
2. LLM responds with potential tool calls
3. App executes tool, gets result
4. App sends tool result back to LLM as new message
5. LLM continues with tool result context
6. Process repeats until LLM stops calling tools

### Considerations

- **Sync vs Async**: Tool execution blocks? Or spawn threads?
- **Security**: What paths can be read? Sandboxing?
- **UI**: How to show tool execution to user? Transparent or explicit?
- **Error handling**: What if tool fails? Send error to LLM?
- **Large responses**: Truncate huge file reads?

## Known Gaps

- No tests for UI rendering (ratatui widgets are hard to test)
- No performance tests (token accumulation speed, large message counts)
- No stress tests (very long input, many modes, rapid mode switching)
- Display rendering tested manually

## Upcoming Work

### 1. Analyze opencode + mitmproxy
- Run opencode with mitmproxy intercepting traffic to local LLM
- Understand tool use patterns with Qwen model
- Inform better understanding of tool invocation protocol

### 2. Implement slash commands (/clear, etc.)
- Add support for `/clear` to clear conversation context
- Parse slash commands from user input before sending to LLM
- Foundation for adding more commands later

### 3. Implement @ file reference autocomplete
- Type `@` to trigger file autocomplete suggestions
- Display matched file paths in cyan (highlighted) in input
- Pass absolute file path to LLM when submitting
- Similar to GitHub/other tools' @ mention syntax

### 4. Extend tool support (glob, grep)
- Add `glob` tool for file pattern matching
- Add `grep` tool (use ripgrep if available, fallback to standard grep)
- Expand LLM's ability to search and explore codebase
