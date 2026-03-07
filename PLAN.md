# Pact Development Plan

## 📋 Research & Configuration
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt
- [ ] config settings to control temperature etc and apply
  unsloth's recommended settings for Qwen 3.5 (for coding,
  thinking, reasoning, writing etc)

## 🚀 Feature Implementation
- [ ] Add automated test action for CI/CD pipeline (run `make test` on every PR/commit)
- [ ] Handle intermittent connections with retry mechanism
- [ ] Creating a task should show full todo list in message buffer

## 🎯 Release Needs
- need a way to release for both mac and linux (locally ideally)
