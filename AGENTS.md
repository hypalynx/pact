# Pact AI Agent Instructions

- When asked to look in the logs or debug, you should look in the
  users ~/.config/pact/pact.db sqlite database. You should be
  able to figure out the schema by reading ./src/db.rs where we
  do the migration to set it up.

- **Never make blocking HTTP calls on the main UI thread**. The app runs an event loop
  at ~60fps (`App::update()`). Any blocking operation (HTTP, file I/O, etc.) will freeze
  the entire UI. Always spawn background threads and send results back via channels.
  See `App::periodic_server_check()` for the pattern: clone `self.api_endpoint` and 
  `self.tx`, spawn a thread, do the blocking work, then `tx.send(LlmEvent::...)` to
  update state in the main thread.

- NEVER make blocking HTTP calls on the main thread/event loop - always
  spawn them in a background thread (via `std::thread::spawn`) and send
  results back through the event channel (`LlmEvent`). Blocking the main
  thread will freeze the UI and make the app unresponsive.

- **Never make blocking HTTP calls on the main UI thread**. The app uses an async event
  loop (`App::update()` runs ~60fps). Any blocking operation (HTTP requests, file I/O,
  etc.) will freeze the entire UI. Always spawn blocking work in a background thread
  and send results back via `self.tx.send(LlmEvent::...)` like we do for server info
  checks in `App::periodic_server_check()`.

- Never make blocking HTTP calls in the main UI thread or event loop.
  Slow responses will freeze the UI. Spawn background threads and
  send results back via channels instead.
