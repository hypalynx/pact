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
- syntax highlighting in diffs, diffs should also contain
  line numbers and show the lines +/- 5 around the context
  being added/changed.
- consider supporting xml tool call (like the one qwen emitted)
- up/down should only go up/down message history if you are at
  the top/bottom of the current message input content, the latest
  message (unsent) should be preseved so you can return to
  it
- ctrl + p to cycle through providers (like we can already in
  the control panel
- split up ui.rs now it's kinda large
- also print the offset + lines provided in the Read command
  feedback to the user/message history i.e Read X lines at
  offset Y (last part only if offset provided and/or != 0)
- no matches for file picker should just close the file
  picker, i.e if you press "@ " or "@gmail.com" then the user
  wants to type that in the message rather than still be using
  the @ file picker.
- only show the last 10 sessions with --resume no args, also
  show the date time rather than just date of the session
