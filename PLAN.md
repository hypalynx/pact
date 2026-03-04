# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

### Phase 6: Task Management & Planning Tools
- [ ] **TodoWrite** — Create/update structured task lists for current session
- [ ] **TodoRead** — Read existing tasks from task list
- [ ] **TaskWrite** — Create new tasks with status tracking
- [ ] **TaskRead** — Read specific task details
- [ ] **TaskUpdate** — Update task status (pending → in_progress → completed)
- [ ] **AskQuestion** — Query user for clarification/decisions during execution
- [ ] **ExitPlanMode** — Request user approval for implementation plans

### Not Started
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt
- [ ] improve code coverage.. consider popular/well known ways to increase reliability of a cli tool
- paste just 'types' into the input box but we should be able to
  handle this better (you see each character input
  sequentially)
- [ ] up/down should only go up/down message history if you are at the top/bottom of the current message input content, the latest message (unsent) should be preserved so you can return to it
- [ ] ctrl + p to cycle through providers (like we can already in the control panel)
- [ ] split up ui.rs now it's kinda large
- [ ] no matches for file picker should just close the file picker, i.e if you press "@ " or "@gmail.com" then the user wants to type that in the message rather than still be using the @ file picker
- [ ] **Add automated test action for CI/CD pipeline**
  - Run `make test` on every PR/commit to confirm formatting, linting, and all 118 tests pass
