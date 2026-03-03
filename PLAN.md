# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

### Not Started
- [ ] config setting to turn off thinking via prompt
- [ ] format code blocks with markdown ```rust etc ```
- [ ] Event loop monitoring: test & monitor if event loop is blocked (>16ms)
- [ ] Slash command improvements:
  - [x] Ctrl+W should still work for backward word delete
  - [ ] Handle /v1/model (shouldn't trigger slash command)
- [ ] Allow more lines in user input
- [x] Tab to switch mode (instead of Ctrl+T)
- [ ] context counting is not accurate and also does not reset
  when context is cleared with /new
- [x] remove scroll % in status bar
- [ ] no limit on read or bash output (generally no limit on tool
  output) - what is a good approach here? limit to.. 250? + add
  a line so the LLM knows it's truncated?
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
