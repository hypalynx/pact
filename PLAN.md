# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

### In Progress
- [ ] `--resume` flag to resume previous sessions
- [ ] make loading animation brighter/more obvious

### Not Started
- [ ] config setting to turn off thinking via prompt
- [ ] format code blocks with markdown ```rust etc ```
- [ ] Event loop monitoring: test & monitor if event loop is blocked (>16ms)
- [ ] Slash command improvements:
  - [ ] Ctrl+W should still work for backward word delete
  - [ ] /new to reset/kill context
  - [ ] Handle /v1/model (shouldn't trigger slash command)
- [ ] Allow more lines in user input
- [ ] Tab to switch mode (instead of Ctrl+T)
- [ ] Always start in plan mode (or user-configured default)
- [ ] Mac OS support (~/Library/Application Support/pact/)

### Tests
- [ ] Tool call block parsing tests
- [ ] Thinking block parsing tests

### Database
- [ ] Better error handling for DB failures (currently silent)
