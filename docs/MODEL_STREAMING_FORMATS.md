# Model Streaming Format Differences

## Problem Statement

Different LLM models send streaming responses in different formats via the OpenAI `/v1/chat/completions` endpoint, particularly regarding how they handle reasoning/thinking content and tool calls.

## Observed Formats

### Standard Format (Qwen 3.5 35B A3B)

Content and reasoning are sent in **separate fields**:

```json
{
  "choices": [{
    "delta": {
      "content": "Regular response text",
      "reasoning_content": "Model's internal reasoning"
    }
  }]
}
```

**Behavior:**
- Regular response → `content` field → displayed as normal text
- Internal reasoning → `reasoning_content` field → displayed as thinking
- Tool calls → embedded in `content` field as `<tool_call>{json}</tool_call>`

**Database Result:**
```
Message {
  text: "Hello! How can I help?",
  thinking: "The user asked hello - simple greeting..."
}
```

### Instruct Format (Qwen 3.4B Instruct)

Everything sent through **reasoning field only**:

```json
{
  "choices": [{
    "delta": {
      "reasoning_content": "Hello! Can I assist you?"
    }
  }]
}
```

Also:
```json
{
  "choices": [{
    "delta": {
      "reasoning_content": "<tool_call>{\"name\": \"Read\", ...}</tool_call>"
    }
  }]
}
```

**Behavior:**
- All content → `reasoning_content` field
- Regular text, thinking, and tool calls all mixed together
- `content` field never populated or ignored

**Database Result:**
```
Message {
  text: "",
  thinking: "Hello! Can I assist you?"  // Displayed as dark gray italic
}

Message {
  text: "",
  thinking: "<tool_call>{...}</tool_call>"  // NOT parsed, displayed as thinking
}
```

### Problem: Tool Calls Not Detected

Current Pact logic in `src/llm.rs:207`:

```rust
// Check if this contains a tool call (for Qwen and text-format models)
if delta.contains("<tool_call>") {
    // Parse tool call...
}
```

This check only happens when processing `content` field:

```rust
if let Some(delta) = delta_obj
    .and_then(|d| d.get("content"))
    .and_then(|t| t.as_str())
{
    if delta.contains("<tool_call>") {
        // Parse tool call
    }
}
```

**For instruct models**: Tool calls come in `reasoning_content`, never checked → never parsed.

## Solution: Dual-Path Checking

Check **both fields** for tool calls:

1. **`content` field** - for standard models (Qwen 3.5, etc.)
2. **`reasoning_content` field** - for instruct models or fallback

```rust
// Extract and check content field
if let Some(delta) = delta_obj.and_then(|d| d.get("content")).and_then(|t| t.as_str()) {
    if delta.contains("<tool_call>") {
        // Parse tool call from content
    } else {
        // Regular text token
        accumulated_text.push_str(delta);
    }
}

// ALSO check reasoning_content field for tool calls (instruct format)
if let Some(delta) = delta_obj.and_then(|d| d.get("reasoning_content")).and_then(|t| t.as_str()) {
    if delta.contains("<tool_call>") {
        // Parse tool call from reasoning_content
    } else {
        // Actual thinking/reasoning token
        accumulated_thinking.push_str(delta);
    }
}
```

**Why this works:**
- **Standard models**: Content parsed from `content` field, thinking from `reasoning_content` ✓
- **Instruct models**: Tool calls detected in `reasoning_content`, rest goes to thinking ✓
- **Fallback**: If a model puts tool calls in wrong field, we still catch them ✓

## Test Results (Mar 1, 2026)

### Working Models

**Qwen 3.5 35B A3B** (`qwen-qwen3.5-35b-a3b`)
- ✅ Standard format (dual-field)
- Text in `content`, thinking in `reasoning_content`
- Tool calls detected and executed properly
- Correct database: `text: "response"`, `thinking: "reasoning..."`

### Broken Models

**Qwen 3.4B Instruct** (`qwen3-4b-instruct-2507`)
- ❌ Non-standard format (reasoning-only)
- Everything sent via `reasoning_content` field
- Tool calls embedded in reasoning field, not detected
- Incorrect database: `text: ""`, `thinking: "response + <tool_call>..."`
- **Fix**: Check `reasoning_content` field for `<tool_call>` tags

**Llama 3.2 1B Instruct** (`llama-3.2-1b-instruct`)
- ❌ Model crash/error (not format issue)
- Error: "Failed to parse input at pos 0"
- Likely too small to handle tool definitions
- Returns empty response
- **Not fixable** - model capability limit

### Recommendations

1. **Implement dual-path checking** for tool calls (fixes Qwen 3.4B)
2. **Skip 1B models** for agent work (too small for tools)
3. **Test other instruct models** if available (Mistral, Phi, etc.)
4. **Monitor for new streaming formats** from future models

## References

- `src/llm.rs:200-324` - SSE delta parsing logic
- `src/event.rs:36-54` - Thinking event handler
- `src/event.rs:127-194` - Tool call event handler
- `docs/TOOL_RESULTS.md` - Tool response format details

## Future: Model Configuration

Long-term, we could add a model config system (like OpenCode does) to specify the streaming format per model, but dual-path checking handles all cases without configuration.
