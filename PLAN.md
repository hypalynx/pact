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
- [ ] **Add automated test action for CI/CD pipeline**
  - Run `make test` on every PR/commit to confirm formatting, linting, and all 118 tests pass
- [ ] when the LLM is busy, I can't see my cursor (though I can
  type) in the input box
- [ ] thinking text also seems to come back as markdown so we
  might as well format it too!
- [ ] /new or /clear should default back to the default mode
  (plan usually but whatever is set in config)
- [ ] writing an .md file was blocked (4th March around 21:16pm)
- [ ] need to include session ids in the debug menu
- [ ] context is still not counting correctly LOL I get like 700k
  in a kimi session which seems very wrong.
- [ ] Reading file message should say what lines are being read
  i.e offset used
- [ ] entering an email i.e mister@example.com tokenizes
  @example.com like a file address.
- [ ] we probably don't need AskUserQuestion/AskQuestion - LLMs
  ask you questions and you type back normally all the time lol
- [ ] handle intermittent connections, retry mechanism?

### Recently Fixed
- [x] AskUserQuestion UI - was showing as hidden modal with dimmed screen (Mar 4, 2026)
  - Fixed: moved from centered modal to input area replacement
  - Now draws above the input box like bash_confirm
  - Removed from is_modal_open check so screen doesn't dim

Bad tool call example:
<tool_call>
<function=Read>
<parameter=filePath>
src/lib.rs
</parameter>
</function>
</tool_call>
