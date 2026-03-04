# UI Module Migration Plan

## Overview

Break down `src/ui.rs` (1461 lines) into domain-focused modules following modern Rust conventions (no `mod.rs`, use `ui.rs` as the parent module).

## Target Structure

```
src/
├── ui.rs              (1159 lines → 180) - orchestration, re-exports, draw_app()
├── ui/
│   ├── colors.rs      (97 lines)  - color constants, parse_color()
│   ├── layout.rs      (59 lines)  - layout constants (widths, heights, margins)
│   ├── messages.rs    (~320 lines) - draw_messages(), message formatting, selection
│   ├── input.rs       (258 lines) - draw_input(), colorize_input(), cursor handling
│   ├── status.rs      (196 lines) - draw_status(), status helpers
│   ├── pickers.rs     (200 lines) - file picker, slash picker, API key input
│   ├── confirmations.rs (120 lines) - bash confirm, ask question
│   ├── panels.rs      (80 lines)  - control panel
│   └── debug.rs       (280 lines) - debug modal (complex, standalone)
```

## Migration Phases

---

### ✅ Phase 1: Foundation (colors + layout) - COMPLETE
**Status:** Done - Extracted constants and utility functions with no dependencies.

**Files created:**
1. `src/ui/colors.rs` (97 lines) - Color constants (dimmed palette) and `parse_color()`
2. `src/ui/layout.rs` (59 lines) - Layout constants (margins, widths, heights)

**Changes to `src/ui.rs`:**
- Added `pub mod colors; pub mod layout;`
- Added imports `use crate::ui::colors::*; use crate::ui::layout::*;`
- Removed 41 lines of duplicate constants/functions

**Tests added:**
- `colors.rs`: 6 tests (parsing, case insensitivity, gray variants)
- `layout.rs`: 4 tests (bounds validation, percentage ranges)

**Verification:**
- ✅ All 10 new tests pass
- ✅ Compiles successfully
- ✅ No regressions

---

### ✅ Phase 2: Core Components (input + status) - COMPLETE
**Status:** Done - Extracted self-contained rendering components.

**Files created:**

#### 1. `src/ui/input.rs` (258 lines)
- `draw_input()` function (~70 lines)
- `colorize_input()` function with @mention highlighting
- Input-specific logic (cursor positioning, scrolling)
- 13 unit tests for mention parsing edge cases

**Dependencies:**
- `use crate::ui::colors::*;` - for dimmed colors (DIM_BG, DIM_TEXT)
- `use crate::ui::layout::*;` - for INPUT_HORIZONTAL_MARGIN, INPUT_VERTICAL_MARGIN
- `use crate::app::App;` - for app state access
- `use crate::text::{cursor_position, wrap_text};` - for text handling

#### 2. `src/ui/status.rs` (196 lines)
- `draw_status()` function (~100 lines)
- `calculate_token_percentage()` helper function
- Status bar rendering logic (git branch, pwd, tokens, mode)
- 5 unit tests for percentage calculations

**Dependencies:**
- `use crate::ui::colors::*;` - for dimmed status colors
- `use crate::app::App;` - for app state
- `use crate::utils::{format_tokens, get_git_branch, get_pwd_display};` - for formatting

**Changes to `src/ui.rs`:**
- Added `pub mod input; pub mod status;`
- Added imports for `draw_input` and `draw_status`
- Removed inline `draw_input()`, `colorize_input()`, and `draw_status()` functions (-302 lines)

**Verification:**
- ✅ All 18 new tests pass (13 input + 5 status)
- ✅ ui.rs reduced: 1461 → 1159 lines (-302 lines)
- ✅ Compiles successfully
- ✅ No regressions

---

### ✅ Phase 3: Messages Area (the big one) - COMPLETE
**Status:** Done - Extracted the complex message rendering component.

**Files created:**
1. `src/ui/messages.rs` (558 lines) - Move `draw_messages()`, `highlight_line_range()`

**Functions extracted:**
- `draw_messages()` - Main message rendering (~250 lines)
  - Handles user messages, assistant messages, tool results
  - Thinking token rendering
  - Pending response/thinking rendering
  - Selection highlighting integration
  - Scrollbar rendering
- `highlight_line_range()` - Selection highlighting helper (~35 lines)

**Dependencies:**
- `use crate::ui::colors::*;` - for all color logic (DIM_BG, DIM_TEXT, DIM_THINKING, etc.)
- `use crate::app::App;` - for app state access
- `use crate::text::{render_message, wrap_text};` - for message formatting
- `ratatui` types for rendering

**Tests added:**
- 6 tests for `highlight_line_range()` covering:
  - No overlap scenarios
  - Full overlap scenarios
  - Partial overlap (start and end)
  - Multiple span overlap
  - Empty selection

**Changes to `src/ui.rs`:**
- Added `pub mod messages;`
- Added import `use crate::ui::messages::draw_messages;`
- Removed inline `draw_messages()` and `highlight_line_range()` functions (-347 lines)
- Removed unused imports (`render_message`, colors import at top level)

**Verification:**
- ✅ All 34 new tests pass (6 messages + existing 28)
- ✅ Compiles successfully
- ✅ No regressions

---

### Phase 4: Pickers & Confirmations
**Goal:** Extract modal/picker components.

**Files to create:**
1. `src/ui/pickers.rs` - File picker, slash picker, API key input
2. `src/ui/confirmations.rs` - Bash confirm, ask question modal

**Dependencies:**
- Uses `colors.rs` for styling
- Uses `layout.rs` for sizing constants (DEBUG_FILE_PICKER_MAX_VISIBLE)

**Testing:**
- Test picker scroll offset calculations
- Test modal height calculations

---

### Phase 5: Panels
**Goal:** Extract panel components.

**Files to create:**
1. `src/ui/panels.rs` - Control panel
2. `src/ui/debug.rs` - Debug modal (complex, deserves its own file)

**Dependencies:**
- Uses all previous modules
- `debug.rs` is the most complex - has list view, expanded view, JSON parsing

**Testing:**
- Test debug log filtering
- Test JSON description extraction from request body

---

### Phase 6: Final Cleanup
**Goal:** Refactor `src/ui.rs` to a thin orchestrator.

**Final `src/ui.rs` structure:**
```rust
// Module declarations
pub mod colors;
pub mod layout;
pub mod messages;
pub mod input;
pub mod status;
pub mod pickers;
pub mod confirmations;
pub mod panels;
pub mod debug;

// Re-exports for convenience
pub use colors::*;
pub use layout::*;

// Main orchestration function
pub fn draw_app(app: &mut App, frame: &mut Frame) {
    // Layout calculation
    // Call each component's draw function
    // Handle modal state dispatch
}
```

---

## Testing Strategy

Each module gets inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature() {
        // Test implementation
    }
}
```

### Test Coverage Goals

| Module | Test Focus | Status |
|--------|-----------|--------|
| `colors.rs` | `parse_color()` edge cases (case insensitivity, unknown colors) | ✅ 6 tests |
| `layout.rs` | Constant values, bounds validation | ✅ 4 tests |
| `input.rs` | `colorize_input()` with @mentions, cursor positioning | ✅ 13 tests |
| `status.rs` | Token percentage calculation | ✅ 5 tests |
| `messages.rs` | Message formatting, selection highlighting, scrollbar | ✅ 6 tests |
| `pickers.rs` | Picker scroll offset, entry truncation | ⏳ Pending |
| `confirmations.rs` | Modal height calculations | ⏳ Pending |
| `panels.rs` | Control panel content generation | ⏳ Pending |
| `debug.rs` | Log filtering, JSON description extraction | ⏳ Pending |

---

## Rollback Plan

If any phase fails:
1. Keep the previous phase's changes
2. Fix issues in isolation
3. Re-run `make test` before proceeding

## Verification Checklist

After each phase:
- [ ] `make test` passes
- [ ] Application compiles
- [ ] UI renders correctly (manual smoke test)
- [ ] New tests added for extracted functions
- [ ] No regression in functionality

## Migration Order Summary

| Phase | Status | Files | Lines Reduced |
|-------|--------|-------|---------------|
| 1 | ✅ Complete | colors.rs, layout.rs | 41 |
| 2 | ✅ Complete | input.rs, status.rs | 302 |
| 3 | ✅ Complete | messages.rs | ~347 |
| 4 | ⏳ Pending | pickers.rs, confirmations.rs | ~200 |
| 5 | ⏳ Pending | panels.rs, debug.rs | ~350 |
| 6 | ⏳ Pending | Final ui.rs cleanup | orchestration |

**Total progress:** 1461 → 776 lines (-685 lines so far)
**Target final ui.rs:** ~180 lines (orchestrator only)

Each phase builds on the previous, minimizing merge conflicts and making rollback easy.
