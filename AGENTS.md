# Pact AI Agent Instructions

- When asked to look in the logs or debug, you should look in the
  users ~/.config/pact/pact.db sqlite database. You should be
  able to figure out the schema by reading ./src/db.rs where we
  do the migration to set it up.
