# Pact Development Plan

## Completed Work (Mar 1, 2026)

**Phases 0-3 Complete**:
- Communication Protocol (array format, reasoning content, system messages)
- SQLite Migration & Debug Infrastructure
- Control Panel, Debug UI, Core Tools (bash/glob/grep/read/write/edit/webfetch)
- Event Loop Extraction, Model Tracking, SSE Architecture
- All major UX polish (line wrapping, markdown, diffs, Ctrl+C confirm, webfetch suppression)
- Better GGUF model name extraction

**Status**: All 74 tests passing, clean build, no warnings

---

## Remaining Work (Nice-to-Have Polish)

### UX Features
- [ ] `--resume` flag to resume previous sessions
- [ ] Always start in plan mode (or user-configured default)
- [ ] Plan mode should not have access to Write/Edit tools
- [ ] Add vertical margin between user messages

### Platform Support
- [ ] Mac OS: Use ~/Library/Application Support/pact/ instead of ~/.config/pact/
- [ ] Allow API keys to be defined from environment variables

### Tests & Parser
- [ ] Tool call block parsing tests
- [ ] Thinking block parsing tests

### Database
- [ ] SQLite schema migration strategy
- [ ] Better error handling for DB failures (currently silent)

---
