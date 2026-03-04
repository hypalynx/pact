# Pact Development Plan

### Not Started
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt
- [ ] improve code coverage.. consider popular/well known ways to increase reliability of a cli tool
  - good use case IF we can would be to include tests around
    up/down history to track cursor movement.. and somehow
    "screenshot" the TUI perhaps?
- paste just 'types' into the input box but we should be able to
  handle this better (you see each character input
  sequentially) - if we haven't already done so.. more than 3
  lines in a paste should have a token instead i.e [Pasted +7
  lines] I am pretty sure we have done this already but need to
  confirm the behaviour
- [ ] split up ui.rs now it's kinda large
- [ ] no matches for file picker should just close the file picker, i.e if you press "@ " or "@gmail.com" then the user wants to type that in the message rather than still be using the @ file picker
- [ ] **Add automated test action for CI/CD pipeline**
  - Run `make test` on every PR/commit to confirm formatting, linting, and all 118 tests pass

### Recently Fixed
- [x] AskUserQuestion UI - was showing as hidden modal with dimmed screen (Mar 4, 2026)
  - Fixed: moved from centered modal to input area replacement
  - Now draws above the input box like bash_confirm
  - Removed from is_modal_open check so screen doesn't dim
