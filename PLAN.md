# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

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

### Platform Support
- [ ] Mac OS: Use ~/Library/Application Support/pact/ instead of ~/.config/pact/
- [ ] Allow API keys to be defined from environment variables

### Tests & Parser
- [ ] Tool call block parsing tests
- [ ] Thinking block parsing tests

### Database
- [ ] SQLite schema migration strategy
- [ ] Better error handling for DB failures (currently silent)
