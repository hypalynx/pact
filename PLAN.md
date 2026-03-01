# Pact Development Plan

## Status Summary (Mar 1, 2026)

### ✅ COMPLETED PHASES
- **Phase 0**: Communication Protocol (100%)
- **Phase 1**: SQLite Migration & Debug Infrastructure (100%)
- **Phase 2**: Control Panel, Debug UI, Core Tools (100%)
- **Phase 3 (Step 1)**: Event Loop Extraction & Model Tracking (100%)
- **Phase 3 (Step 2)**: SSE Parser Architecture Decision (100%)

### 📝 MOSTLY POLISH WORK
All major infrastructure complete. Remaining items are quality-of-life features and edge cases.

---

## ✅ Phase 0: Communication Protocol (100% COMPLETE - Mar 1, 2026)

### Completed Items
- ✅ Tool descriptions audit (all tools documented)
- ✅ Capture reasoning_content from SSE (llm.rs:256-260)
- ✅ System message refactor (dedicated role:system)
- ✅ **Array message format switch (DONE)**
  - Changed from `{"content": "text"}` to `{"content": [{"type": "text", "text": "..."}]}`
  - System messages, user messages, assistant messages all converted
  - Tool results remain string format (OpenAI spec)
  - Fully backward compatible at transport level
  - All 74 tests passing, clean build

---

## Phase 3: Architecture & Testing (IN PROGRESS)

### Step 1 - COMPLETED ✅
- ✅ Event loop extracted to `src/event.rs` (290 lines)
- ✅ Model name tracking in `api_logs` table
- ✅ All 74 tests passing

### Step 2 - DECISION MADE (NO EXTRACTION NEEDED)
- ✅ **SSE Parser Module**: Decided NOT to extract
  - Reason: OpenAI-compatible APIs only (llama.cpp), format is standardized
  - SSE parsing is straightforward JSON path navigation (lines 195-340 in llm.rs)
  - Tightly coupled with message handling, no benefit to extraction
  - Integration tests exist in `tests/sse_parser.rs`

### Step 3 - TODO (FUTURE, LOW PRIORITY)
- [ ] Collect real response data from DB for test fixtures (if needed for debugging)
- [ ] Create model-specific response examples (5-10 per model)

---

## UX & Quality Enhancements (LOWER PRIORITY)

### Already Implemented ✅
- ✅ User input line wrapping
- ✅ Markdown italic formatting (*text*)
- ✅ Tool result display (diffs visible in UI)
- ✅ Escape to cancel API calls (with confirmation)
- ✅ Webfetch output suppression (output hidden from UI, like Read/Glob/Grep)
- ✅ Ctrl+C confirm before quitting app
- ✅ Better model name extraction from GGUF files

### Decided NOT to Do
- ~~Progress % in status bar~~ - API limitation (llama.cpp doesn't provide streaming progress info)

### TODO
- [ ] Provide --resume flag to resume previous sessions
- [ ] Plan mode should not have access to Write/Edit tools
- [ ] Always start in plan mode (or user-configured default)
- [ ] Add vertical margin to user messages in messages section

### Mac OS Support
- [ ] Use ~/Library/Application Support/pact/ instead of ~/.config/pact/ on macOS
- [ ] But still prefer ~/.config if explicitly set

### API Key Management
- [ ] Allow API keys to be defined from environment variables
- [ ] Avoid storing sensitive keys in committed dotfiles

---

## Testing & Parser Enhancements (LOWER PRIORITY)

### Parser Tests
- [ ] Tool call block parsing
- [ ] Thinking block parsing
- [ ] Additional markdown features

### Database & Logging
- [ ] Migration strategy for SQLite schema changes
- [ ] Better error handling for DB failures (currently silent failures)

---

## Implementation Notes

### Phase 0 Array Format Details
When switching message format, verify:
1. No regression in existing tests (should pass)
2. LLM server compatibility (llama.cpp accepts both formats)
3. UI display unchanged

### Phase 3 Testing Approach
Collect real responses in DB first, then extract to fixtures:
1. Run Pact with various models (Qwen 3.5, 4B, 1.7B, etc.)
2. Query DB: `SELECT full_response FROM api_logs WHERE model_name = ?`
3. Pick representative examples (success, error, thinking, tool calls)
4. Create fixture files in `tests/fixtures/`
5. Use in SSE parser unit tests

### No Longer Needed
- ~~Load existing messages.json on startup~~ (deferred, not critical)
- ~~Safeguarding blocks for directory access~~ (nice-to-have, lower priority)
- ~~Stop hooks for verification scripts~~ (feature creep, not needed)

---
