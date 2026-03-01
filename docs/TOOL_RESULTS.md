# Tool Results: Message Format & Standards

How tool results are sent back to the LLM varies by API. This document covers:

1. **Claude API** (Anthropic)
2. **OpenAI API** (Chat Completions & Responses)
3. **Llama.cpp** (OpenAI-compatible, what Pact uses)

---

## Claude API Format

### Tool Definition
```json
{
  "name": "get_weather",
  "description": "Get the current weather in a given location",
  "input_schema": {
    "type": "object",
    "properties": {
      "location": {
        "type": "string",
        "description": "The city and state, e.g. San Francisco, CA"
      }
    },
    "required": ["location"]
  }
}
```

### Tool Use Request (Claude initiates)
Claude's response includes a `tool_use` block in the message:

```json
{
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "I'll check the weather for you."
    },
    {
      "type": "tool_use",
      "id": "toolu_01A09q90qw90lq917835lq9",
      "name": "get_weather",
      "input": { "location": "San Francisco, CA" }
    }
  ]
}
```

### Tool Result Response (Send back to Claude)
After executing the tool, send results in a **user message** with `tool_result` blocks:

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
      "content": "Current weather in San Francisco: 72°F, sunny"
    }
  ]
}
```

### Error Response
If tool execution fails, set `is_error: true`:

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
      "is_error": true,
      "content": "Error: Location 'Atlantis' not found in weather database"
    }
  ]
}
```

### Complete Conversation Flow

```python
messages = [
    {"role": "user", "content": "What's the weather in SF?"}
]

# Step 1: Claude requests tool use
response = client.messages.create(
    model="claude-opus-4-6",
    max_tokens=1024,
    tools=[get_weather_tool],
    messages=messages
)
# response.content = [
#   {"type": "text", "text": "I'll check..."},
#   {"type": "tool_use", "id": "toolu_01...", "name": "get_weather", "input": {...}}
# ]

# Step 2: Execute the tool
result = execute_get_weather(response.content[1]["input"])  # "72°F, sunny"

# Step 3: Send result back to Claude
messages.append({"role": "assistant", "content": response.content})
messages.append({
    "role": "user",
    "content": [
        {
            "type": "tool_result",
            "tool_use_id": "toolu_01...",
            "content": result
        }
    ]
})

# Step 4: Claude generates final response with tool result context
final_response = client.messages.create(
    model="claude-opus-4-6",
    max_tokens=1024,
    tools=[get_weather_tool],
    messages=messages
)
# final_response.content[0].text = "The weather in San Francisco is 72°F and sunny."
```

### Parallel Tool Results
Claude can request multiple tools at once. Send all results in a single user message:

```json
{
  "role": "assistant",
  "content": [
    {"type": "tool_use", "id": "toolu_01", "name": "get_weather", "input": {...}},
    {"type": "tool_use", "id": "toolu_02", "name": "get_time", "input": {...}}
  ]
}

// Send both results in ONE message:
{
  "role": "user",
  "content": [
    {"type": "tool_result", "tool_use_id": "toolu_01", "content": "72°F"},
    {"type": "tool_result", "tool_use_id": "toolu_02", "content": "3:45 PM"}
  ]
}
```

**Important**: Tool result blocks MUST come FIRST in the content array. Any text must come AFTER:

```json
{
  "role": "user",
  "content": [
    {"type": "tool_result", "tool_use_id": "...", "content": "result"},
    {"type": "text", "text": "Additional context"}  // ✅ Text comes after
  ]
}
```

---

## OpenAI API Format (Chat Completions)

### Tool Definition
```json
{
  "type": "function",
  "function": {
    "name": "get_weather",
    "description": "Get the current weather in a given location",
    "parameters": {
      "type": "object",
      "properties": {
        "location": {
          "type": "string",
          "description": "The city and state, e.g. San Francisco, CA"
        }
      },
      "required": ["location"],
      "additionalProperties": false
    }
  }
}
```

### Tool Call Request (OpenAI initiates)
OpenAI's response includes `tool_calls` in the message:

```json
{
  "role": "assistant",
  "content": null,
  "tool_calls": [
    {
      "id": "call_abc123xyz",
      "type": "function",
      "function": {
        "name": "get_weather",
        "arguments": "{\"location\": \"San Francisco, CA\"}"
      }
    }
  ]
}
```

### Tool Call Result (Send back to OpenAI)
After executing the tool, send result as a **tool role message**:

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123xyz",
  "content": "Current weather in San Francisco: 72°F, sunny"
}
```

### Error Response
Same format, with error message as content:

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123xyz",
  "content": "Error: Location 'Atlantis' not found in weather database"
}
```

### Complete Conversation Flow

```python
messages = [
    {"role": "user", "content": "What's the weather in SF?"}
]

# Step 1: Send request with tools
response = openai.ChatCompletion.create(
    model="gpt-4",
    messages=messages,
    tools=[get_weather_tool],
    tool_choice="auto"
)
# response.choices[0].message = {
#   "role": "assistant",
#   "tool_calls": [
#     {"id": "call_abc123xyz", "type": "function", "function": {...}}
#   ]
# }

# Step 2: Execute the tool
tool_call = response.choices[0].message.tool_calls[0]
result = execute_get_weather(tool_call.function.arguments)  # "72°F, sunny"

# Step 3: Add assistant message and tool result to history
messages.append(response.choices[0].message)
messages.append({
    "role": "tool",
    "tool_call_id": tool_call.id,
    "content": result
})

# Step 4: OpenAI generates final response with context
final_response = openai.ChatCompletion.create(
    model="gpt-4",
    messages=messages,
    tools=[get_weather_tool],
    tool_choice="auto"
)
# final_response.choices[0].message.content = "The weather in SF is 72°F and sunny."
```

### Key Differences from Claude
1. Tool calls are in a separate `tool_calls` array, not in `content`
2. Assistant message content is usually `null` when calling tools
3. Tool results use `role: "tool"` (not `role: "user"` with `type: "tool_result"`)
4. Tool call ID format: `"call_abc123xyz"` (string), not `"toolu_01..."`

---

## OpenAI Responses API (Newer)

OpenAI has a **Responses API** with different structure:

### Tool Call Structure
```json
{
  "type": "function_call",
  "call_id": "call_123",
  "id": "msg_block_123",
  "function": {
    "name": "get_weather",
    "arguments": "{\"location\": \"San Francisco, CA\"}"
  }
}
```

### Tool Result Structure
```json
{
  "type": "function_call_output",
  "call_id": "call_123",
  "output": "72°F, sunny"
}
```

**Note**: This is different from Chat Completions API. Responses API treats tool calls and results as separate items, not embedded in messages.

---

## Llama.cpp / OpenAI-Compatible (What Pact Uses)

Pact uses **llama.cpp** which is compatible with **OpenAI's Chat Completions API**, but with some variations:

### Tool Definition (Same as OpenAI)
```json
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
          "description": "Path to the file"
        }
      },
      "required": ["filePath"],
      "additionalProperties": false
    }
  }
}
```

### Tool Call Request
Llama.cpp streams tool calls via SSE (Server-Sent Events):

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

**Current Pact behavior** (src/llm.rs:244-291):
- Parses streaming SSE chunks
- Accumulates `function.arguments` JSON fragments until complete
- Emits `LlmEvent::ToolCall { name, args }` when arguments JSON closes

### Tool Result Response (Pact Currently)

**Current implementation** (src/tools.rs, src/event.rs):
```rust
// Execute tool
let (summary, output) = tools::execute_tool(&ToolCall { name, args });

// Create message
Message {
    role: "tool".to_string(),
    text: output,
    is_tool_result: true,
    tool_call_id: Some(call_id),
    // ...
}

// Append to messages and send in next request
messages.push(message);
```

**What gets sent to llama.cpp**:
```json
{
  "role": "tool",
  "content": "result text here",
  "tool_call_id": "call_abc123"
}
```

**Issue**: The `tool_call_id` field may not be recognized by all llama.cpp variants. OpenAI uses this field, but some models expect just the `content`.

---

## Best Practices by Tool Type

### Read/Write Tools (Files)
**Content format**: Plain text or error message

```
Success: "# Project README\n\nThis is my project..."
Error: "Error reading file: Permission denied"
```

### Bash/Shell Tools
**Content format**: stdout/stderr as plain text

```
Success: "total 48\ndrwxr-xr-x  5 user  staff   160 Feb 28 10:30 .\ndrwxr-xr-x 19 user  staff   608 Feb 28 09:00 .."
Error: "command not found: nonexistent_cmd"
```

### Glob/Search Tools
**Content format**: Line-separated file paths or results

```
Success: "src/main.rs\nsrc/lib.rs\ntests/integration_test.rs"
Error: "No files match pattern: **/*.nonexistent"
```

### TodoWrite Tool
**Content format**: JSON or structured text showing what was created

```json
Success: {
  "created": true,
  "task_id": "task_123",
  "title": "Implement feature X",
  "description": "Add support for feature X",
  "status": "created"
}

Error: {
  "created": false,
  "error": "Task title is required"
}
```

Or plain text:

```
Success: "Task created: ID=task_123, Title='Implement feature X'"
Error: "Error: Task title is required"
```

### Web Request Tools
**Content format**: Response body or truncated content

```
Success: "<!DOCTYPE html>\n<html>\n<head>...</head>\n<body>...</body>\n</html>"
Error: "Error: Connection timeout after 30s"
```

### Interactive Tools (Question)
**Content format**: JSON with question context

```json
{
  "question": "What is your preferred language?",
  "options": ["Python", "Rust", "JavaScript"],
  "response_required": true
}
```

---

## Pact Implementation Guide for New Tools

### When Adding a New Tool

1. **Define it** in `tools.rs:get_tool_definitions()` with OpenAI schema
2. **Execute it** in `tools.rs:execute_tool()` returning `(summary: String, output: String)`
3. **Results are sent** automatically:
   - Tool execution creates a `Message` with `is_tool_result: true`
   - Message appended to conversation
   - Next LLM request includes the tool result
   - LLM receives it as a `role: "tool"` message

### Current Format Sent to Llama.cpp

```rust
// From src/llm.rs, messages are sent as:
for m in messages {
    let msg = if m.is_tool_result {
        json!({
            "role": m.role,
            "content": m.text  // or m.tool_result_content
        })
    } else {
        json!({ "role": m.role, "content": m.text })
    };
    msg_payload.push(msg);
}
```

### What Llama.cpp Expects

Most llama.cpp implementations expect OpenAI-compatible format:

```json
{
  "role": "tool",
  "content": "result text"
}
```

Some variants also accept:
```json
{
  "role": "tool",
  "tool_call_id": "call_123",
  "content": "result text"
}
```

---

## Differences Between APIs: Quick Reference

| Aspect | Claude | OpenAI (Chat) | Llama.cpp |
|--------|--------|---------------|----------|
| **Tool Definition** | `input_schema` | `parameters` | `parameters` (OpenAI) |
| **Tool Call Message** | `role: "assistant"` with `tool_use` block | `role: "assistant"` with `tool_calls` array | `role: "assistant"` with `tool_calls` (streamed SSE) |
| **Tool Call ID** | `id: "toolu_01..."` | `id: "call_abc123"` | `id: "call_abc123"` |
| **Result Message** | `role: "user"` with `tool_result` block | `role: "tool"` | `role: "tool"` |
| **Result Structure** | Block array with `type: "tool_result"` | Single string content | Single string content |
| **Error Handling** | `is_error: true` flag | Included in content string | Included in content string |
| **Parallel Tools** | All results in one user message | All results in sequence | All results in sequence (likely) |

---

## Example: TodoWrite Tool

### Definition (Pact)
```rust
json!({
    "type": "function",
    "function": {
        "name": "TodoWrite",
        "description": "Create or update a task in the task list. Returns task ID and status.",
        "parameters": {
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "Brief, imperative task title (e.g., 'Implement auth system')"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed requirements and context for the task"
                },
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form shown in spinner (e.g., 'Implementing auth system')"
                }
            },
            "required": ["subject", "description"],
            "additionalProperties": false
        }
    }
})
```

### Execution (Pact)
```rust
fn execute_todowrite(args: &serde_json::Map<String, Value>) -> (String, String) {
    let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("Untitled");
    let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("");

    // Create task in local storage
    let task_id = generate_task_id();
    let task = Task {
        id: task_id.clone(),
        subject: subject.to_string(),
        description: description.to_string(),
        status: "pending",
    };

    // Save task
    save_task(&task);

    let summary = format!("Created task: {}", subject);
    let output = serde_json::to_string(&json!({
        "created": true,
        "task_id": task.id,
        "subject": task.subject,
        "status": task.status
    })).unwrap_or_default();

    (summary, output)
}
```

### Result Sent Back
```json
{
  "role": "tool",
  "content": "{\"created\": true, \"task_id\": \"task_abc123\", \"subject\": \"Implement auth system\", \"status\": \"pending\"}"
}
```

### LLM Receives
The LLM sees the task was created and can reference it in future interactions.

---

## References

- [Claude API Tool Use Documentation](https://platform.claude.com/docs/en/agents-and-tools/tool-use/implement-tool-use)
- [OpenAI Function Calling Guide](https://developers.openai.com/api/docs/guides/tools/)
- [OpenAI Responses API](https://platform.openai.com/docs/api-reference/responses)
- [Llama.cpp API Compatibility](https://github.com/ggerganov/llama.cpp/blob/master/examples/server/PUBLIC_API.md)

---

**Last Updated**: Feb 28, 2026
**Status**: Pact uses OpenAI-compatible format via llama.cpp
