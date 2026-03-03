# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

### Not Started
- [ ] config setting to turn off thinking via prompt
- [ ] format code blocks with markdown ```rust etc ```
- [ ] Event loop monitoring: test & monitor if event loop is blocked (>16ms)
- [ ] no todowrite/read/list etc
- [ ] improve code coverage.. consider popular/well known ways to
  increase reliability of a cli tool
- [ ] consistent UI experience.. how to? also break down manually
  i.e no outline borders, consistent keybindings (i.e esc at any
  point to cancel even if input, esc during menu closes the menu)
- syntax highlighting in diffs, diffs should also contain
  line numbers and show the lines +/- 5 around the context
  being added/changed.
- consider supporting xml tool call (like the one qwen emitted)
- up/down should only go up/down message history if you are at
  the top/bottom of the current message input content, the latest
  message (unsent) should be preseved so you can return to
  it
