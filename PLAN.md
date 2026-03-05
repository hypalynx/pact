# Pact Development Plan

- need a way to release for both mac and linux (locally ideally)

### Not Started
- [ ] Research: Control tokens for disabling thinking in supported models
  - Resources: https://github.com/QwenLM/Qwen3.5, Fireworks reasoning-parser docs
- [ ] config setting to turn off thinking via prompt
- paste just 'types' into the input box but we should be able to
  handle this better (you see each character input
  sequentially) - if we haven't already done so.. more than 3
  lines in a paste should have a token instead i.e [Pasted +7
  lines] I am pretty sure we have done this already but need to
  confirm the behaviour
- [ ] when using /new or /clear you can't use up arrow to access
  previous history
- [ ] **Add automated test action for CI/CD pipeline**
  - Run `make test` on every PR/commit to confirm formatting, linting, and all 118 tests pass
- [ ] we seem to be printing out the output for bash commands -
  see api logs for session 9013888 for more info, this is fine
  for small output but we should truncate in the middle like we
    do for reads but probably more agressively for message
      history display.
- [ ] when the LLM is busy, I can't see my cursor (though I can
  type) in the input box
- [ ] esc should work to cancel context even if halfway through
  typing in the input box
- [ ] creating a task should show the current todo list in full
  in the message buffer/section even if we are sending the json
  with id back to the LLM to work with.
- [ ] user input messages during generation are not queued and
  played after the LLM stops.. they just get input to the message
  history and not executed.
- [ ] how to implement subagents? is this done like a tool call?
- [ ] thinking text also seems to come back as markdown so we
  might as well format it too!
- [ ] /new or /clear should default back to the default mode
  (plan usually but whatever is set in config)
- [ ] writing an .md file was blocked (4th March around 21:16pm)
- [ ] context is still not counting correctly LOL I get like 700k
  in a kimi session which seems very wrong.
- [ ] Reading file message should say what lines are being read
  i.e offset used
- [ ] entering an email i.e mister@example.com tokenizes
  @example.com like a file address.
- [ ] we probably don't need AskUserQuestion/AskQuestion - LLMs
  ask you questions and you type back normally all the time lol
- [ ] handle intermittent connections, retry mechanism?
- [ ] scrolling in the debug view (when viewing a specific record)

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
