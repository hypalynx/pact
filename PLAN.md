# Pact Development Plan

## Remaining Work (Nice-to-Have Polish)

### Not Started
- [ ] make loading animation brighter/more obvious
- [ ] config setting to turn off thinking via prompt
- [ ] format code blocks with markdown ```rust etc ```
- [ ] Event loop monitoring: test & monitor if event loop is blocked (>16ms)
- [ ] Slash command improvements:
  - [ ] Ctrl+W should still work for backward word delete
  - [ ] Handle /v1/model (shouldn't trigger slash command)
- [ ] Allow more lines in user input
- [ ] Tab to switch mode (instead of Ctrl+T)
- [ ] context counting is not accurate and also does not reset
  when context is cleared with /new
- [ ] no limit on read or bash output (generally no limit on tool
- [ ] remove scroll % in status bar
  output) - what is a good approach here? limit to.. 250? + add
  a line so the LLM knows it's truncated?
- [ ] no todowrite/read/list etc
- [ ] improve code coverage.. consider popular/well known ways to
  increase reliability of a cli tool
- [ ] consistent UI experience.. how to? also break down manually
  i.e no outline borders, consistent keybindings (i.e esc at any
  point to cancel even if input, esc during menu closes the menu)
- syntax highlighting in diffs
