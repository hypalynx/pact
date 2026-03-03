# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

### Not Started
- [ ] Research: Find list of tools LLMs typically support and implement them
  - Focus on todo list tools (todowrite/read/list) - research shows these help LLM focus
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt
- [ ] format code blocks with markdown ```rust etc ```
- [ ] improve code coverage.. consider popular/well known ways to
  increase reliability of a cli tool
- [ ] consistent UI experience - consistent keybindings
  (i.e esc at any point to cancel even if input, esc during menu closes the menu)
  - [x] Remove outline borders from modals (using EMPTY border set)
- syntax highlighting in diffs, diffs should also contain
  line numbers and show the lines +/- 5 around the context
  being added/changed.
- consider supporting xml tool call (like the one qwen emitted)
- up/down should only go up/down message history if you are at
  the top/bottom of the current message input content, the latest
  message (unsent) should be preseved so you can return to
  it
