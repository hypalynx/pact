# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

- [ ] config setting to turn off thinking via prompt

### UX Features
- [ ] `--resume` flag to resume previous sessions
- [ ] Always start in plan mode (or user-configured default)
- [ ] Plan mode should not have access to Write/Edit tools
- [ ] Add vertical margin between user messages
- [ ] How can we test AND monitor if the even loop is blocked and
  becomes longer than 16ms? e.g previous processing of huge
  amounts of LLM response e.g 2 blocking http calls LOL.
- /slash command should still kill backward word with Ctrl + W
- /new to rest/kill context
- switching mode using tab not ctrl + t lol
- allow more lines in user input lol
- make loading animation brighter/more obvious
- typing /v1/model is treated as a slash command.. not ideal,
  also I might legitimately want to write '/model' without
  actually using the slash command so we need to handle this.

### Platform Support
- [ ] Mac OS: Use ~/Library/Application Support/pact/ instead of ~/.config/pact/ - we are using the dirs crate and this gives us a OS specific/preferred dir but tbh let's just use the XDG .config, most cli tools use that anyway even on Mac OS.
- [ ] Allow API keys to be defined from environment variables

### Tests & Parser
- [ ] Tool call block parsing tests
- [ ] Thinking block parsing tests

### Database
- [ ] SQLite schema migration strategy
- [ ] Better error handling for DB failures (currently silent)
