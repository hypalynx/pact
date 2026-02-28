# AGENTS.md Configuration

Pact supports custom agent instructions through `AGENTS.md` files. These files contain system prompts that guide the AI's behavior.

## File Locations

Pact loads AGENTS.md files from two locations:

1. **Global/User**: `~/.config/pact/AGENTS.md`
   - Applied to all projects by default
   - Contains your preferred base instructions

2. **Project/Local**: `./AGENTS.md` (in current working directory)
   - Project-specific customizations
   - Extends or overrides global instructions

## Loading Behavior

When both files exist, Pact **concatenates** them with the following order:

1. Global/user content first
2. Project/local content second (with double newline separator)

This means **project AGENTS.md can override global instructions** by appearing later in the combined system prompt. This intentional design allows:

- **Global baseline**: Set your preferred defaults once
- **Project customization**: Adjust behavior for specific codebases or teams
- **Flexible layering**: Add additional constraints or capabilities per-project

## Custom Path

You can specify a custom path in your `pact.yaml`:

```yaml
agents_md_path: "/path/to/your/custom-agents.md"
```

This replaces the default `~/.config/pact/AGENTS.md` location.

## Usage in Prompts

When an AGENTS.md file is loaded, its content becomes the **system prompt** for all LLM interactions. This replaces any mode-specific system prompts configured in your modes.

## Tips

- **Start simple**: Begin with a basic global AGENTS.md and refine over time
- **Project patterns**: Use local AGENTS.md for codebase-specific conventions
- **Testing**: Check what instructions are active via the debug panel
- **Version control**: Consider committing local AGENTS.md files so team members share the same context

## Example

**Global** (`~/.config/pact/AGENTS.md`):
```markdown
# My Preferred Coding Style

- Use descriptive variable names
- Add type hints where possible
- Keep functions small and focused
```

**Project** (`./AGENTS.md`):
```markdown
# Project-Specific Rules

- This project uses snake_case for all identifiers
- Use the internal error handling crate, not anyhow
- Prefer match over if-else for enums
```

When working in that project, the AI receives both sets of instructions, with project rules appearing later and taking precedence where there might be conflict.
