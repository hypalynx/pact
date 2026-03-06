# Pact Development Plan

## 📋 Research & Configuration
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt

## 🎨 UI/UX Improvements
- [ ] Paste handling improvement (show token for multiple lines pasted)
- [ ] Truncate bash command output more aggressively
- [ ] Format thinking text as markdown
- [ ] Reading file message should show offset/lines being read
- [ ] Fix email tokenization issue (e.g., @example.com shouldn't look like a file path)
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

## 🎯 Release Needs
- need a way to release for both mac and linux (locally ideally)
