# Tools in Pact

Tools are the mechanism by which the LLM can perform actions beyond generating text. This document explains:

1. **Current tools implemented in Pact**
2. **The 3-part tool system** (definition, calling, responding)
3. **OpenAI API standards** for tool definitions
4. **How Pact implements tool calling**
5. **Tool catalog with examples**

---

## Overview: The 3-Part Tool System

Tool handling involves three critical components that must work together:

### Part 1: Tool Definition
The LLM needs to know what tools exist and how to use them. This is expressed as a JSON schema sent in every request.

```rust
// Code: src/tools.rs → get_tool_definitions()
tools: [
    {
        "type": "function",
        "function": {
            "name": "Read",
            "description": "Read the complete contents of a file...",
            "parameters": {
                "type": "object",
                "properties": {
                    "filePath": {
                        "type": "string",
                        "description": "Path to the file - absolute or relative"
                    }
                },
                "required": ["filePath"],
                "additionalProperties": false
            }
        }
    },
    // ... more tools ...
]
```

**Where**: Sent in every `/v1/chat/completions` request as the `tools` parameter
**When**: Always included; OpenAI API (and llama.cpp) use this to constraint model output

### Part 2: Tool Calling (LLM → Agent)
When the LLM wants to invoke a tool, it returns a structured tool call in the response.

```rust
// From streaming SSE response:
// {"choices": [{"delta": {"tool_calls": [
//     {
//         "index": 0,
//         "id": "call_xxx",
//         "function": {
//             "name": "Read",
//             "arguments": "{\"filePath\": \"/path/to/file\"}"
//         }
//     }
// ]}}]}

// Pact code: src/llm.rs (lines 244-289)
// Parse streaming tool_calls and emit LlmEvent::ToolCall
let _ = tx.send(LlmEvent::ToolCall {
    name: partial.name.clone(),
    args: args_obj.clone(),
});
```

**Where**: Comes back in streaming response, one chunk per call
**Handling**: src/llm.rs parses SSE deltas and accumulates partial JSON until complete

### Part 3: Tool Response (Agent → LLM)
After the agent executes the tool, it sends the result back to the LLM so it can continue.

```rust
// Tool execution: src/tools.rs → execute_tool()
fn execute_tool(tool_call: &ToolCall) -> (String, String) {
    match tool_call.name.as_str() {
        "Read" => execute_read(&tool_call.args),
        "Glob" => execute_glob(&tool_call.args),
        "Grep" => execute_grep(&tool_call.args),
        _ => error response
    }
}

// Result format sent back to LLM:
{
    "role": "tool",
    "tool_call_id": "call_xxx",
    "content": "result text or error message"
}
```

**Where**: Appended to messages list and sent in the next request
**Format**: OpenAI standard `tool` role message with `tool_call_id` reference

---

## Current Tools in Pact

### 1. Read — File Content Reading
**Purpose**: Read complete file contents
**Source**: `src/tools.rs:13-33` (definition), `src/tools.rs:90-150` (execution)

**Definition**:
```json
{
    "type": "function",
    "function": {
        "name": "Read",
        "description": "Read the complete contents of a file. Accepts absolute paths (starting with /) or relative paths from current directory. Returns the raw text content. Files larger than 64KB are truncated with a warning.",
        "parameters": {
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "Path to the file - either absolute (e.g., /etc/hosts) or relative (e.g., ./README.md)"
                }
            },
            "required": ["filePath"],
            "additionalProperties": false
        }
    }
}
```

**Usage**:
```
User: "What's in the README?"
→ LLM: {"name": "Read", "args": {"filePath": "./README.md"}}
→ Pact executes: fs::read_to_string("./README.md")
→ LLM receives: "# Project Name\n\nDescription..."
```

**Constraints**:
- Max 64KB per file (larger files truncated)
- Returns raw text (no encoding conversion)
- Both absolute and relative paths supported
- Errors reported clearly (file not found, permission denied, etc.)

---

### 2. Glob — File Pattern Matching
**Purpose**: Find files matching glob patterns
**Source**: `src/tools.rs:34-50` (definition), `src/tools.rs:152-188` (execution)

**Definition**:
```json
{
    "type": "function",
    "function": {
        "name": "Glob",
        "description": "Find files matching a glob pattern from the current directory. Use * to match any characters within a name, ** to match across directories. Returns list of matching file paths.",
        "parameters": {
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g., '*.rs', 'src/**/*.ts', 'test_*.py')"
                }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }
    }
}
```

**Usage**:
```
User: "Find all Rust test files"
→ LLM: {"name": "Glob", "args": {"pattern": "**/*_test.rs"}}
→ Pact executes: glob::glob("**/*_test.rs")
→ LLM receives: "tests/unit_test.rs\nsrc/lib_test.rs\n..."
```

**Patterns**:
- `*.rs` — all Rust files in current directory
- `src/**/*.ts` — all TypeScript files under src/
- `test_*.py` — Python files starting with "test_"

---

### 3. Grep — Content Search with Regex
**Purpose**: Search file contents using regex patterns
**Source**: `src/tools.rs:51-72` (definition), `src/tools.rs:190-265` (execution)

**Definition**:
```json
{
    "type": "function",
    "function": {
        "name": "Grep",
        "description": "Search for lines matching a pattern in files. Use regex patterns to find text across one or more files.",
        "parameters": {
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "files": {
                    "type": "string",
                    "description": "File path or glob pattern (e.g., '*.rs', './src/main.rs', 'src/**/*.py')"
                }
            },
            "required": ["pattern", "files"],
            "additionalProperties": false
        }
    }
}
```

**Usage**:
```
User: "Find all TODO comments in Rust files"
→ LLM: {"name": "Grep", "args": {"pattern": "// TODO", "files": "**/*.rs"}}
→ Pact executes:
   1. glob::glob("**/*.rs") to find files
   2. regex::Regex::new("// TODO") to compile pattern
   3. Search each file line-by-line
→ LLM receives: "src/main.rs:42: // TODO: handle error\nsrc/lib.rs:7: // TODO: optimize"
```

**Output Format**:
```
file_path:line_number: matching_line
src/main.rs:42: let x = parse(input) // TODO: handle error
src/utils.rs:15: fn old_helper() { // TODO: remove after refactor
```

---

## Tool Definitions: OpenAI API Standard

Based on OpenAI's `/v1/chat/completions` API, tools follow this JSON schema:

```json
{
    "type": "function",
    "function": {
        "name": "string",
        "description": "string",
        "parameters": {
            "type": "object",
            "properties": {
                "param_name": {
                    "type": "string|number|boolean|object|array",
                    "description": "string"
                }
            },
            "required": ["array of required params"],
            "additionalProperties": false
        },
        "strict": true  // (optional) enforce strict schema compliance
    }
}
```

### Key Points

1. **Type**: Always `"function"` (reserved for future tool types like `"computer"`/`"code_interpreter"`)
2. **Name**: Alphanumeric identifier; used by LLM to invoke the tool
3. **Description**: Natural language explanation of what the tool does
4. **Parameters**: JSON Schema for the function's arguments
   - `properties`: Each parameter and its type
   - `required`: Array of mandatory parameters
   - `additionalProperties: false`: Reject unknown parameters (stricter)
5. **Strict mode**: If `true`, enforces exact schema compliance (prevents hallucinations)

### Design Principles for Tool Definitions

From OpenAI documentation and Pact's observations:

1. **Clear naming**: Function names should be self-documenting (Read, Glob, Grep — not Fn1, Fn2)
2. **Rich descriptions**: Include constraints, limits, and examples
   ```
   BAD:  "Read a file"
   GOOD: "Read the complete contents of a file. Supports absolute paths (/etc/passwd)
          and relative paths (./README.md). Files larger than 64KB are truncated."
   ```
3. **Parameter guidance**: Describe each parameter's purpose, valid formats, and examples
   ```json
   {
       "filePath": {
           "type": "string",
           "description": "Path to the file - either absolute (e.g., /etc/hosts)
                          or relative (e.g., ./README.md)"
       }
   }
   ```
4. **Minimal parameters**: More parameters = harder for LLM to use correctly
   - Max 4 parameters recommended (per Pact architecture guidelines)
   - Use objects to group related parameters if needed

---

## How Pact Implements Tool Calling

### Request Flow: Sending Tool Definitions

**File**: `src/llm.rs:105-115`

```rust
let mut body = json!({
    "model": "local",
    "max_tokens": max_tokens,
    "stream": true,
    "messages": msg_payload,
    "tools": tools::get_tool_definitions(),  // Include all available tools
    "tool_choice": "auto",                   // Let model decide when to use tools
    "stream_options": {
        "include_usage": true
    }
});
```

**Key settings**:
- `"tools"`: Array of tool definitions (from `get_tool_definitions()`)
- `"tool_choice": "auto"`: Model can decide to use tools or just respond with text
  - Other options: `"required"` (must use tool), `"none"` (never use), or specific tool name

### Response Flow: Receiving Tool Calls

**File**: `src/llm.rs:244-291`

The response is **streamed** as Server-Sent Events (SSE). Tool calls come as JSON fragments:

```
data: {"choices":[{"delta":{"tool_calls":[
    {
        "index": 0,
        "id": "call_abc123",
        "function": {
            "name": "Read",
            "arguments": "{\"filePath\": \"/path/to/file\"}"
        }
    }
]}}]}
```

**Parsing logic**:
1. Parse each SSE line as `data: {...}`
2. Extract `choices[0].delta.tool_calls` array
3. Accumulate `function.arguments` JSON fragments until complete (ends with `}`)
4. Emit `LlmEvent::ToolCall { name, args }` when complete
5. Main event loop receives this event and processes it

**Code flow**:
```rust
// Accumulate streaming tool calls
let mut partial_tool_call: Option<PartialToolCall> = None;

// For each SSE chunk...
if let Some(tool_calls) = delta_obj.and_then(|d| d.get("tool_calls")).and_then(|tc| tc.as_array()) {
    for tool_call in tool_calls {
        // Extract function name and start accumulating arguments
        if let Some(function) = tool_call.get("function") {
            // Accumulate JSON fragments into partial.arguments
            // When complete, emit LlmEvent::ToolCall
            if partial.arguments.ends_with('}') {
                if let Ok(args_json) = serde_json::from_str::<serde_json::Value>(&partial.arguments) {
                    let _ = tx.send(LlmEvent::ToolCall {
                        name: partial.name.clone(),
                        args: args_obj.clone(),
                    });
                }
            }
        }
    }
}
```

### Tool Execution: Running the Tool

**File**: `src/tools.rs:76-88`

```rust
pub fn execute_tool(tool_call: &ToolCall) -> (String, String) {
    match tool_call.name.as_str() {
        "Read" => execute_read(&tool_call.args),
        "Glob" => execute_glob(&tool_call.args),
        "Grep" => execute_grep(&tool_call.args),
        _ => {
            let error = format!("Unknown tool: {}", tool_call.name);
            (error.clone(), error)
        }
    }
}
```

Returns `(summary: String, output: String)`:
- `summary`: What the tool did (for logs)
- `output`: Tool result (sent back to LLM)

### Result Flow: Sending Tool Response Back to LLM

**File**: `src/event.rs` (event handler), `src/app.rs` (message construction)

When a tool call event is received:

```rust
LlmEvent::ToolCall { name, args } => {
    // 1. Execute the tool
    let (summary, output) = tools::execute_tool(&tool_call);

    // 2. Create a tool result message
    let tool_result_msg = Message {
        role: "tool".to_string(),
        content: output,
        is_tool_result: true,
        tool_call_id: Some(call_id),
        // ...
    };

    // 3. Add to messages
    app.messages.push(tool_result_msg);

    // 4. On next request, send messages + tool result back to LLM
    // LLM can then continue reasoning with the tool output
}
```

The tool result message is appended to the conversation and sent in the next request to `/v1/chat/completions`.

---

## Tool Response Format (OpenAI Standard)

When sending a tool result back to the LLM, use this format:

```json
{
    "role": "tool",
    "tool_call_id": "call_abc123",
    "content": "result of executing the tool"
}
```

**Fields**:
- `role`: Must be `"tool"` (literal string, not "user")
- `tool_call_id`: Must match the `id` from the LLM's tool call
- `content`: The result (success) or error message (failure)

**Example**:
```json
[
    {
        "role": "user",
        "content": "What's in README.md?"
    },
    {
        "role": "assistant",
        "content": null,
        "tool_calls": [
            {
                "id": "call_123",
                "function": {
                    "name": "Read",
                    "arguments": "{\"filePath\": \"README.md\"}"
                }
            }
        ]
    },
    {
        "role": "tool",
        "tool_call_id": "call_123",
        "content": "# My Project\nThis is a great project..."
    }
]
```

---

## Comparison: Pact vs. Opencode

Based on analysis of opencode's tool definitions and pact traffic:

| Aspect | Pact (Current) | Opencode | Gap |
|--------|---|---|---|
| **Tools** | 3 (Read, Glob, Grep) | 8-11 | Missing bash, edit, write, task, webfetch, etc. |
| **Descriptions** | Moderate | Detailed with examples | Could be richer |
| **Parameter Naming** | camelCase | camelCase | ✓ Aligned |
| **Tool Choice** | `"auto"` | `"auto"` | ✓ Aligned |
| **Streaming** | SSE with deltas | SSE with deltas | ✓ Aligned |
| **Tool Call Format** | `tool_calls` array | `tool_calls` array | ✓ Aligned |

### Tools Available in Opencode (Reference)

From `opencode.json` and traffic analysis:

1. **bash** — Execute shell commands
2. **read** — Read file contents
3. **write** — Create/overwrite files
4. **edit** — Search-and-replace in files
5. **glob** — Find files by pattern
6. **grep** — Search file contents
7. **question** — Ask user for input
8. **webfetch** — Fetch and process URLs
9. **todowrite** — Create/manage task lists
10. **task** — Launch agents for complex tasks
11. **skill** — Execute user-defined scripts

Pact currently has: **read, glob, grep** (3/11)

---

## Future Tools: Planning

Based on Opencode's toolset and Pact's architecture plan, candidate tools for Phase 2:

### High Priority
1. **bash** — Execute arbitrary shell commands (with restrictions)
2. **edit** — Search-and-replace in existing files
3. **write** — Create or overwrite files

### Medium Priority
4. **webfetch** — HTTP GET with content processing
5. **question** — Prompt user for input (interactive)

### Lower Priority (requires more infrastructure)
6. **task** — Launch background agents
7. **todowrite** — Persistent task tracking
8. **skill** — User script execution

See `PLAN.md` for implementation phases and timeline.

---

## Testing Tool Definitions

### For Developers

When adding a new tool, test:

1. **Definition validity**: Valid JSON schema
   ```rust
   let defs = get_tool_definitions();
   assert!(serde_json::to_value(&defs).is_ok());
   ```

2. **Execution**: Tool runs correctly with sample inputs
   ```rust
   let args = serde_json::Map::from(vec![
       ("filePath".to_string(), json!("./README.md"))
   ]);
   let (summary, output) = execute_read(&args);
   assert!(!output.is_empty());
   ```

3. **Error handling**: Graceful failures
   ```rust
   let args = serde_json::Map::from(vec![
       ("filePath".to_string(), json!("/nonexistent/file"))
   ]);
   let (summary, output) = execute_read(&args);
   assert!(output.contains("Error"));
   ```

### For LLM Users

When using tools with the LLM:

1. **Check tool is available**: `Ctrl+Shift+D` → debug panel → look for tool definitions
2. **Monitor execution**: Watch status bar for tool execution (should be instant for Read/Glob/Grep)
3. **Review results**: Check if tool output matches expectations
4. **Inspect errors**: Debug panel shows failed tool calls with error messages

---

## Architecture Notes

### Why Three Parts?

The 3-part system (definition, calling, responding) mirrors how real APIs work:

1. **Definition** = API documentation (swagger, OpenAPI spec)
2. **Calling** = Client making a request
3. **Response** = Server responding with results

This design allows:
- **Modularity**: Each part can be tested independently
- **Extensibility**: New tools added without changing the core loop
- **Standardization**: Follows OpenAI API conventions
- **Clarity**: Each component has a single responsibility

### Performance Considerations

- **Tool definitions** are sent with every request (~500 bytes for 3 tools, <100 bytes per tool)
- **Tool execution** is synchronous and blocking (tools run immediately, not in background)
  - Future: Could make async (requires redesign of main loop)
- **Result accumulation** happens in the SSE parser (no separate step)

### Security Considerations

- **Path validation**: Tools accept paths without sandboxing
  - Future: Add `ALLOWED_DIRS` config to prevent reading outside project
  - Currently: Trust user not to read sensitive files (same as shell access)
- **Command execution**: `bash` tool will need safety guardrails
  - Option 1: Allowlist dangerous commands (rm, dd, etc.)
  - Option 2: Confirmation prompt for destructive operations
  - Option 3: Dry-run mode (show what would happen without executing)

---

## References

- **OpenAI API Docs**: https://platform.openai.com/docs/api-reference/chat/create
- **Function Calling Guide**: https://developers.openai.com/api/docs/guides/function-calling/
- **OpenAI Cookbook**: https://cookbook.openai.com/examples/how_to_call_functions_with_chat_models
- **Pact Source**: `src/tools.rs`, `src/llm.rs` (lines 244-291, 105-115)
- **Opencode Reference**: `~/src/shell/config/opencode/opencode.json`

---

**Last Updated**: Feb 28, 2026
**Status**: Pact has 3 core tools; expanding based on Phase 2 plan
