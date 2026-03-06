# Pact Development Plan

## 📋 Research & Configuration
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt

## 🎨 UI/UX Improvements
- [ ] Paste handling improvement (show token for multiple lines pasted)
- [ ] Truncate bash command output more aggressively
- [ ] Show cursor when LLM is busy
- [ ] Format thinking text as markdown
- [ ] Reading file message should show offset/lines being read
- [ ] Fix email tokenization issue (e.g., @example.com shouldn't look like a file path)
- [ ] Remove AskUserQuestion/AskQuestion - LLM questions work normally
- [ ] Scrolling in debug view for specific records

## 🚀 Feature Implementation
- [ ] Add automated test action for CI/CD pipeline (run `make test` on every PR/commit)
- [ ] Implement subagents (possibly as tool calls)
- [ ] Handle intermittent connections with retry mechanism
- [ ] Creating a task should show full todo list in message buffer
- [ ] pending input or.. input passed in while the llm is
  responding doesn't retrigger a response and should also be
  inserted into message history relative to when the llm responds
  i.e initial input -> initial response -> queued response ->
  response to queued input

## 🐛 Bug Fixes
(none currently)

---

## 🎯 Release Needs
- need a way to release for both mac and linux (locally ideally)

### Recently Fixed
- [x] Context counting incorrect (Mar 6, 2026)
  - Fixed: Changed `+=` to `=` for token accumulation (only track last call, not cumulative)
- [x] Writing .md file was blocked in plan mode (Mar 6, 2026)
  - Fixed: Corrected parameter name from `"file_path"` to `"path"` in plan mode validation
- [x] AskUserQuestion UI - was showing as hidden modal with dimmed screen (Mar 4, 2026)
  - Fixed: moved from centered modal to input area replacement
  - Now draws above the input box like bash_confirm
  - Removed from is_modal_open check so screen doesn't dim
