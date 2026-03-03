# LLM Context Management: Tool Output Limits

When tools return large outputs (file reads, command results, web fetches), we must balance **utility** against **context consumption**. This document explains Pact's approach.

---

## Design Rationale

LLMs have finite context windows and attention is expensive. Tool outputs directly consume tokens that could otherwise be used for:
- Reasoning and planning
- Tracking conversation history
- Handling follow-up requests
- Maintaining working context across multiple tool calls

Unbounded tool outputs create pathological cases:
- `cat /var/log/syslog` → millions of lines → context explosion
- `grep -r "pattern" .` on large codebases → thousands of matches
- Single 10MB file read → unintelligible token waste

**Solution**: Reasonable limits per tool type with escape hatches for power users.

---

## Implemented Limits

### Read Tool: 500 Lines
- **Limit**: 500 lines maximum
- **Truncation**: Shows first 450 lines + last 50 lines with `[... N lines truncated ...]` in middle
- **Escape hatch**: `offset` parameter allows navigation
  ```
  Read(file_path="/large_file.txt", offset=1000)  // Skip to line 1000
  ```
- **Rationale**: 500 lines covers ~20KB of code, most functions/files, and typical config files
- **Smart truncation**: Preserves file structure (start + end) while hiding middle bulk

### Bash Tool: 500 Lines + 64KB Bytes
- **Line limit**: 500 lines maximum
- **Byte limit**: 64KB as hard safety net
- **Applies to**: stdout/stderr from shell commands
- **Escape hatch**: Users can pipe output to `tail`, `head`, `grep` to filter before Pact sees it
- **Rationale**:
  - 500 lines catches most command outputs (directory listings, grep results, etc.)
  - 64KB safety net prevents binary data or runaway logging from destroying context

### Other Tools (Glob, Grep, WebFetch)
- **Glob**: Returns one filename per line, limited by practical filesystem size (~10K files)
- **Grep**: Returns matching lines with context, output follows same 500 line limit as Bash
- **WebFetch**: Returns markdown-converted HTML, subject to same 500 line limit

---

## When Limits Help

### Pathological Cases Prevented

| Scenario | Without Limit | With Limit | Escape |
|----------|---------------|-----------|--------|
| `cat /var/log/syslog` | ~1M lines, context destroyed | 500 lines, 450+50 truncated | Use `tail -50 /var/log/syslog` |
| `ls -la /` on huge dir | 10K+ files | 500 files shown | Use `ls \| grep pattern` |
| Large JSON config (10MB) | 500K lines of tokens | 500 lines visible | Use `jq '.section'` to extract |
| Recursive grep on repo | 50K matches | 500 lines of matches | Use `grep -l` to list files only |

### Normal Cases Unaffected

| Scenario | Output | Status |
|----------|--------|--------|
| Read a function (50-100 LOC) | 100 lines | ✅ No truncation |
| Read a config file | 200 lines | ✅ No truncation |
| `ls -la` in typical dir | 50 lines | ✅ No truncation |
| `git diff` on single file | 150 lines | ✅ No truncation |
| Run tests (simple output) | 30 lines | ✅ No truncation |

---

## Context Impact Analysis

**Token rough estimates** (Claude):
- 1 line of code ≈ 5-10 tokens
- 500 lines ≈ 2500-5000 tokens
- 64KB raw text ≈ 16K tokens (conservative)

**Comparison**:
- Full conversation history (10 messages): ~1K-2K tokens
- Single large file without limit: ~10K-50K tokens
- With limits: ~2.5K-5K tokens per tool call

**Conclusion**: Limits reduce overhead by 70-90% in pathological cases while being transparent for normal use.

---

## Truncation Strategy: First + Last

When a tool output exceeds limits, we show:
- **First N lines**: Preserves file/output structure, headers, early context
- **Truncation marker**: Explicit `[... X lines truncated ...]` helps LLM understand
- **Last N lines**: Captures results, final state, end-of-file markers

**Example**:
```
Line 1: # My Large File
Line 2: ## Introduction
...
Line 450: ...content...
[... 3,500 lines truncated ...]
Line 3501: ## Conclusion
...
Line 3550: # End of file
```

LLM gets:
1. ✅ File context and purpose (lines 1-2)
2. ✅ Some content (450 lines)
3. ✅ Indicator that output was large (marker)
4. ✅ Final conclusions/state (last 50 lines)

---

## When to Use Escape Hatches

### Use offset parameter (Read tool)
```rust
// Initial read (lines 1-500)
Read(file="/large_file.rs")

// Jump to different section
Read(file="/large_file.rs", offset=2000)  // Lines 2000-2500
```

### Use shell piping (Bash tool)
```bash
# Instead of: ls -la > destroys context
# Use: ls -la | head -20

# Instead of: cat huge_log.txt > too much
# Use: tail -100 huge_log.txt

# Instead of: grep -r "pattern" . > thousands of matches
# Use: grep -l "pattern" .   # Just file names
```

### Use specialized tools
```bash
# Instead of: grep pattern file.txt | wc -l
# Just ask the tool: "Grep(pattern=..., file_type=...)"  # handles truncation

# Instead of: jq '.field' huge.json > might be large
# Use: Glob to find configs, Read smaller portions
```

---

## Design Decisions & Trade-offs

### ✅ Chosen: Per-Tool Limits, Not Global
**Alternative**: Single global context budget across all tools
**Why chosen**:
- Each tool has different value density (grep results differ from code files)
- Users understand "file reads cap at 500 lines" better than "you have 50K tokens"
- Easier to reason about and document

### ✅ Chosen: Smart Truncation (First + Last)
**Alternative 1**: Just first 500 lines (no last 50)
- ❌ Misses crucial end-of-file context
- ❌ Truncates conclusions, final states

**Alternative 2**: Middle truncation (random 500 lines)
- ❌ Loses file structure entirely
- ❌ LLM confused about context

**Alternative 3**: Summarization (compress middle)
- ❌ Requires pre-processing, complexity
- ❌ Loses code structure, syntax highlighting

### ✅ Chosen: Offset Parameter Over Pagination
**Alternative**: Automatic pagination ("use next chunk")
- ❌ Requires state tracking
- ❌ Implicit pagination is confusing

**Why offset**:
- Explicit and simple
- User controls what they need
- No magic state

### ✅ Chosen: 500 Lines (Not 1000 or 100)
**Analysis**:
- **100 lines** = Most functions fit, but small codebases feel restricted
- **500 lines** = Typical module/feature in one view, token usage reasonable (~2.5K tokens)
- **1000 lines** = Feels unlimited in practice, token usage high (~5K tokens)

**500 chosen**: Sweet spot between "doesn't feel limiting" and "manages tokens well"

---

## Future Considerations

### If limits become problematic:
1. **User feedback**: Track which tools users repeatedly offset/navigate
2. **Increase limits**: If normal case hitting limits, re-evaluate (500 → 800?)
3. **Smart detection**: Could detect file type (code vs logs) and adjust per-file
4. **Compression**: Could optionally strip comments/whitespace before sending to LLM

### Not planned (over-engineering):
- ❌ Automatic summarization of middle content
- ❌ Per-file-type limits (too complex)
- ❌ Adaptive limits based on context budget (overkill)
- ❌ Gzip compression of tool outputs (premature optimization)

---

## References

- **Context Window**: Claude 3.5 Sonnet = 200K tokens, Haiku = 100K tokens
- **Token Cost**: ~1 token per 3-4 characters in typical source code
- **Streaming**: Limits apply to what LLM receives, not to streaming chunk size

---

**Last Updated**: March 3, 2026
**Status**: Implemented and in use across all tool types
**Decision**: Final
