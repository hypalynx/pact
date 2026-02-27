# Pact Development Plan

## Next In Line

Items raised in this session that still need completion:

1. **Scroll verification** - User reported scroll up/down wasn't working. Changes were made to `calculate_total_lines()` to account for `pending_thinking`. Need to confirm if scrolling now works properly.

2. **Selection highlighting in UI** - When text is selected and copied to clipboard, visually highlight the selected text area in the message display (currently just shows "Copied to clipboard!" message). This requires refactoring the text rendering to track and display selected character ranges.

3. **Progress bar for LLM generation** - Llama.cpp sends progress data in streaming responses (e.g., `progress = 0.724581`). Need to:
   - Check if progress field is in the SSE streaming responses
   - Parse progress percentage from API responses
   - Display a progress indicator (could be percentage or progress bar) in the UI while LLM is generating

4. **Out-of-order messages** - User saw some weird message ordering. Need to investigate once scroll is confirmed working to see what messages are appearing out of order.

5. **Loading animation across multiple API calls** - Made change to clear `pending_thinking` in `send_to_llm()` to ensure loading spinner shows for subsequent requests (after tool execution). Need to verify the animation stays active for the full tool execution → LLM response cycle.

---

## Completed This Session

- ✅ Tool call streaming argument accumulation - Fixed SSE events that stream tool call arguments as JSON fragments
- ✅ File read tool execution - Tool calls now properly execute and send results back to LLM
- ✅ Fresh session on startup - Messages load from SQLite for history/debug, but chat starts fresh
- ✅ Full response reconstruction - SSE deltas now accumulated into complete response with [THINKING], [TEXT], [TOOL_CALLS], [USAGE] sections
- ✅ Error logging infrastructure - Added error_logs table (though not yet used)
- ✅ Clipboard timing - Added 100ms sleep to keep clipboard open longer for clipboard managers
- ✅ Compiler warnings as errors - Set up `.cargo/config.toml` with `--deny warnings`
- ✅ Removed file-based message.json system - Now SQLite-only for persistence
- ✅ Clippy linting - Fixed all clippy warnings without using suppression annotations
- ✅ Tool result display - Shows "Reading filename" instead of full file content
