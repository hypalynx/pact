# UI Module Migration Plan

## Overview

Break down `src/ui.rs` (1461 lines) into domain-focused modules following modern Rust conventions (no `mod.rs`, use `ui.rs` as the parent module).

## Target Structure

```
src/
├── ui.rs              (180 lines) - orchestration, re-exports, draw_app()
├── ui/
│   ├── colors.rs      (60 lines)  - color constants, parse_color()
│   ├── layout.rs      (80 lines)  - layout constants, area calculations
│   ├── messages.rs    (280 lines) - draw_messages(), message formatting, selection
│   ├── input.rs       (180 lines) - draw_input(), colorize_input(), cursor handling
│   ├── status.rs      (120 lines) - draw_status(), status helpers
│   ├── pickers.rs     (200 lines) - file picker, slash picker, API key input
│   ├── confirmations.rs (120 lines) - bash confirm, ask question
│   ├── panels.rs      (80 lines)  - control panel
│   └── debug.rs       (280 lines) - debug modal (complex, standalone)
```

## Migration Phases

### Phase 1: Foundation (colors + layout)
**Goal:** Extract constants and utility functions that have no dependencies.

**Files to create:**
1. `src/ui/colors.rs` - Move color constants and `parse_color()`
2. `src/ui/layout.rs` - Move layout constants and helper functions

**Changes to `src/ui.rs`:**
- Add `pub mod colors; pub mod layout;`
- Update imports to use `crate::ui::colors::*`

**Testing:**
- Add unit tests for `parse_color()` edge cases
- Add tests for layout constant values

---

### Phase 2: Core Components (input + status)
**Goal:** Extract self-contained rendering components.

**Files to create:**
1. `src/ui/input.rs` - Move `draw_input()`, `colorize_input()`, input constants
2. `src/ui/status.rs` - Move `draw_status()`, status helpers

**Dependencies:**
- Uses `colors.rs` for dimmed colors
- Uses `layout.rs` for margin constants

**Testing:**
- Test `colorize_input()` with @mentions
- Test status text formatting

---

### Phase 3: Messages Area (the big one)
**Goal:** Extract the most complex component - message rendering with selection.

**Files to create:**
1. `src/ui/messages.rs` - Move `draw_messages()`, `highlight_line_range()`

**Dependencies:**
- Uses `colors.rs` for all color logic
- Uses `crate::text` for wrapping/rendering

**Testing:**
- Test message formatting for different roles (user/assistant/tool)
- Test selection highlighting logic
- Test scrollbar calculations

---

### Phase 4: Pickers & Confirmations
**Goal:** Extract modal/picker components.

**Files to create:**
1. `src/ui/pickers.rs` - File picker, slash picker, API key input
2. `src/ui/confirmations.rs` - Bash confirm, ask question modal

**Dependencies:**
- Uses `colors.rs` for styling
- Uses `layout.rs` for sizing constants

**Testing:**
- Test picker filtering logic
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

| Module | Test Focus |
|--------|-----------|
| `colors.rs` | `parse_color()` edge cases (case insensitivity, unknown colors) |
| `layout.rs` | Constant values, area calculations |
| `input.rs` | `colorize_input()` with @mentions, cursor positioning math |
| `status.rs` | Status text formatting, token percentage calculation |
| `messages.rs` | Message formatting per role, selection highlighting, scrollbar math |
| `pickers.rs` | Picker scroll offset, entry truncation |
| `confirmations.rs` | Modal height calculations, command truncation |
| `panels.rs` | Control panel content generation |
| `debug.rs` | Log filtering, JSON description extraction, timestamp formatting |

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

1. **Phase 1:** colors.rs + layout.rs (foundation, no deps)
2. **Phase 2:** input.rs + status.rs (simple components)
3. **Phase 3:** messages.rs (complex component)
4. **Phase 4:** pickers.rs + confirmations.rs (modals)
5. **Phase 5:** panels.rs + debug.rs (panels)
6. **Phase 6:** Final cleanup of ui.rs

Each phase builds on the previous, minimizing merge conflicts and making rollback easy.
