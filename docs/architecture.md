# Pact Architecture

## Overview

Pact is a non-blocking TUI (Terminal User Interface) for interacting with local LLMs. The architecture prioritizes responsiveness and clean separation of concerns.

```
┌─────────────────────────────────────────────────────────────┐
│                        main.rs (235 lines)                  │
│                     Event Loop & Coordination                │
│  - Runs the TUI event loop (16ms polling)                   │
│  - Routes events (keyboard, mouse) to app.rs                │
│  - Handles LlmEvents from background threads                │
│  - Coordinates UI drawing and app state updates             │
└──────────────┬──────────────────────────────────────────────┘
               │
      ┌────────┼────────┐
      │        │        │
      ▼        ▼        ▼
   ┌──────┐ ┌──────┐ ┌────────┐
   │ui.rs │ │app.rs│ │llm.rs  │
   └──────┘ └──────┘ └────────┘
      │        │        │
      └────────┼────────┘
               │
    ┌──────────┴──────────┐
    │                     │
    ▼                     ▼
┌────────────┐    ┌──────────────┐
│ config.rs  │    │ utils.rs     │
│ text.rs    │    │ tools.rs     │
└────────────┘    └──────────────┘
```

## Module Breakdown

### 1. **State Management (app.rs - 460 lines)**

The central state machine holding all application data:

```rust
pub struct App {
    pub messages: Vec<Message>,      // Conversation history
    pub input: String,               // Current user input being edited
    pub pending_response: String,    // LLM response streaming in
    pub loading: bool,               // Waiting for LLM response?
    pub scroll_offset: usize,        // Message area scroll position
    pub history: Vec<String>,        // Previous user messages
    // ... modes, tokens, configuration, etc
}
```

**Responsibilities:**
- Store and update conversation state
- Handle user input (text editing, cursor movement, history navigation)
- Trigger LLM requests via `send_to_llm()`
- Save/load message history to disk
- Manage scrolling, selection, and other UI state

**Key Methods:**
- `submit_message()` - User pressed Enter
- `send_to_llm()` - Spawn background thread to call LLM
- Text editing: `insert_char()`, `delete_char()`, `move_cursor_*()`, `kill_word_backward()`
- Scrolling: `scroll_up()`, `scroll_down()`, `calculate_total_lines()`

### 2. **Event Loop (main.rs - 235 lines)**

The synchronization heartbeat of the application:

```
Loop (every 16ms):
  1. Draw UI (render current state)
  2. Check for LLM events from background thread
     - Token arrived? Accumulate in pending_response
     - Tool called? Execute it, add result, send back to LLM
  3. Poll for user input (keyboard/mouse) with 16ms timeout
     - Update app state based on input
  4. Repeat
```

**Why 16ms?** Feels smooth at ~60 FPS while remaining responsive to user input.

**Event Sources:**
- **LlmEvent channel** (from background thread): Token, Done, Error, Usage, ToolCall
- **Crossterm events** (keyboard/mouse): Key presses, scrolling, text selection

**Key insight:** This loop is the synchronization point between:
- Background HTTP requests (threads)
- User input (system events)
- Screen rendering
- Tool execution

### 3. **LLM Communication (llm.rs - 259 lines)**

Handles all communication with the local LLM server:

```
send_to_llm() called
    ↓
Spawn background thread
    ↓
POST to http://127.0.0.1:7777/v1/messages with:
  - All messages (history + user input)
  - System prompt (from current mode)
  - Tool definitions
  - Temperature setting
    ↓
Stream SSE (Server-Sent Events) response:
  - Parse "data: {...}" lines
  - Extract delta.text (streaming tokens)
  - Extract delta.tool_calls (tool invocations)
    ↓
Send LlmEvent via mpsc channel back to main thread
```

**Thread Safety:** Uses `mpsc::channel()` for safe communication between threads without locks.

**Message Format:**
```rust
pub struct Message {
    pub role: String,           // "user" or "assistant"
    pub text: String,           // Message content
    pub is_tool_result: bool,   // Tool results rendered in grey
}
```

**Tool Calls:** When LLM wants to use a tool, it's detected from SSE stream and sent as `LlmEvent::ToolCall`, then executed synchronously in main thread.

### 4. **UI Rendering (ui.rs - 269 lines)**

Draws the terminal UI every frame:

```
┌─ Messages Area
│  - User messages: black background (matches input box)
│  - Assistant messages: markdown-formatted
│  - Tool results: dark grey text (differentiated)
├─ Input Box (with cursor or "thinking..." spinner)
└─ Status Bar (pwd | git branch | mode | loading spinner | model | tokens)
```

**Rendering Pipeline:**
1. Iterate through all messages
2. Text wrapping: Break to terminal width (word-aware)
3. For each line: Parse markdown (`**bold**`, `*italic*`, `` `code` ``)
4. Convert to ratatui `Span` and `Line` structures with colors/styles
5. Render to frame

**Markdown Support:**
- `**bold**` → rendered with bold modifier
- `*italic*` → rendered with italic modifier
- `` `code` `` → rendered in cyan color
- `# Heading` → rendered in yellow, bold, underlined

### 5. **Configuration (config.rs - 127 lines)**

Loads settings from `~/.config/pact/pact.yaml`:

```yaml
api:
  endpoint: "http://127.0.0.1:7777"
  max_tokens: 1024
  api_key: null
ui:
  default_mode: "build"
  modes:
    build:
      system_prompt: "You are a helpful coding assistant..."
      temperature: null          # null = use server default
      color: "cyan"
    plan:
      system_prompt: "You are an expert at planning..."
      temperature: 0.5
      color: "green"
```

**Key Features:**
- XDG-compliant paths (`~/.config/pact/`, `~/.local/share/pact/`)
- User-defined modes: press Tab to cycle through
- Each mode has its own system prompt, temperature, and color
- Configuration reloaded on startup

### 6. **Tool Execution (tools.rs - 69 lines)**

Defines and executes available tools:

```rust
pub fn execute_tool(tool_call: &ToolCall) -> String {
    match tool_call.name.as_str() {
        "read" => execute_read(&tool_call.args),
        // Future tools can be added here
    }
}
```

**Implemented Tools:**
- `read` - Read file contents from absolute paths (max 64KB)

**Tool Results:** Returned as strings, added to conversation as special `is_tool_result: true` messages.

### 7. **Utilities (utils.rs - 116 lines)**

Helper functions:

- `get_pwd_display()` - Current working directory with ~ expansion
- `get_git_branch()` - Current git branch (if in a repo)
- `format_tokens()` - Human-readable token counts (K, M suffixes)
- `fetch_server_info()` - Query LLM server for model name and context window

### 8. **Text Processing (text.rs - 74 lines)**

String utilities:

- `wrap_text()` - Break text to terminal width, preserving words
- `parse_markdown_line()` - Parse a single line for markdown syntax and return styled spans

---

## Data Flow: User Message to Response

Here's how a user message flows through the system:

```
1. USER TYPES MESSAGE
   └─ app.rs: insert_char()
      Updates app.input as user types

2. USER PRESSES ENTER
   └─ app.rs: submit_message()
      - Moves input text to app.messages
      - Clears input field
      - Calls send_to_llm()

3. BACKGROUND THREAD: LLM REQUEST
   └─ app.rs: send_to_llm()
      Spawns thread that calls:

      llm.rs: call_llm()
      - Gathers all messages (history + new user message)
      - Prepends mode's system prompt
      - POSTs to LLM with tool definitions
      - Begins streaming SSE response

4. STREAMING RESPONSE
   └─ llm.rs parses SSE and sends events via channel:

      Loop:
        - LlmEvent::Token("text") → streamed text
        - LlmEvent::Usage{...} → token counts
        - LlmEvent::ToolCall{name, args} → tool invocation
        - LlmEvent::Done → response complete

5. MAIN LOOP RECEIVES EVENTS
   └─ main.rs: receives on mpsc channel

      match event {
        Token(t) => app.pending_response.push_str(&t)
        ToolCall{name, args} => {
          result = tools::execute_tool(...)
          app.messages.push(tool_result)
          app.send_to_llm()  // Send result back!
        }
        Done => {
          app.messages.push(Message{role: "assistant", text: pending})
          app.loading = false
        }
      }

6. UI RENDERS EVERY FRAME
   └─ main.rs: ui::draw_app()
      Converts app state to visual output:

      - Renders all messages (with markdown formatting)
      - Renders pending_response (streamed text appearing)
      - Renders input box with cursor position
      - Renders status bar with metadata

7. USER SEES RESPONSE IN REAL-TIME
   └─ Every frame, new tokens appear
   └─ Tool results show in grey
   └─ LLM can respond to tool results
```

---

## Key Design Patterns

### Non-Blocking Architecture

The 16ms event loop ensures the UI stays responsive:

- User input is checked with `event::poll(Duration::from_millis(16))`
- LLM requests run in background threads
- Events are communicated via `mpsc::channel()` (lock-free)
- No blocking I/O in the main thread

### Thread Safety Without Locks

Uses Rust's ownership model:

```rust
let messages = self.messages.clone();  // Full copy
std::thread::spawn(move || {
    call_llm(messages, ...)  // Thread owns its copy
});
```

No mutex needed because each thread has its own data. Results sent back via channel.

### Error Handling

Rust's `Result` and `Option` types prevent silent failures:

```rust
// Option: might be None
if let Some(path) = args.get("path").and_then(|v| v.as_str()) { ... }

// Result: might be error
fs::read_to_string(path)
    .map_err(|e| format!("Error: {}", e))
```

Errors displayed to user, details logged server-side only.

### Configuration Management

Mode-based system allows runtime switching:

```rust
pub struct Mode {
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub color: Option<String>,
}

// Press Tab to cycle modes
app.cycle_mode()  // Updates system_prompt and temperature
```

---

## Performance Characteristics

| Aspect | Implementation | Benefit |
|--------|---|---|
| **Polling interval** | 16ms | ~60 FPS, responsive to user input |
| **LLM requests** | Background thread | UI never blocks |
| **Message streaming** | SSE parsing + mpsc | Tokens appear in real-time |
| **Text rendering** | Calculated on-demand | Works at any terminal size |
| **History persistence** | Lazy save on submit | Fast submissions |

---

## Future Extension Points

1. **More Tools** - Add to `tools.rs` and update `get_tool_definitions()`
2. **Search/Filter** - Add state to `App` for filtering messages
3. **Export** - Save conversations in different formats
4. **Syntax Highlighting** - Extend markdown parser for code blocks
5. **Keybinding Customization** - Move hardcoded keys to config
6. **Plugin System** - Load tools dynamically at runtime

---

## Testing Strategy

- **Unit tests** in each module for utilities (text wrapping, parsing)
- **Integration tests** for message flow and tool execution
- **Manual testing** - TUI behavior hard to automate, prefer human verification

---

## Dependencies

Key crates:

- **ratatui** - TUI rendering
- **crossterm** - Terminal events and control
- **reqwest** - HTTP client (blocking)
- **serde_json** - JSON parsing
- **pulldown-cmark** - Markdown parsing
- **serde_yaml** - Config file parsing
- **indexmap** - Ordered HashMap for modes

