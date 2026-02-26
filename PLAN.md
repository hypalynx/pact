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

## Known Gaps

- No tests for UI rendering (ratatui widgets are hard to test)
- No performance tests (token accumulation speed, large message counts)
- No stress tests (very long input, many modes, rapid mode switching)
- Display rendering tested manually
