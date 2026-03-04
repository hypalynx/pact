use pulldown_cmark::{Event as MdEvent, Parser as MdParser, Tag};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::sync::OnceLock;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub fn parse_markdown_line(text: &str) -> Vec<Span<'static>> {
    let parser = MdParser::new(text);
    let mut spans = Vec::new();
    let mut bold = false;
    let mut italic = false;

    for event in parser {
        match event {
            MdEvent::Start(tag) => match tag {
                Tag::Strong => bold = true,
                Tag::Emphasis => italic = true,
                _ => {}
            },
            MdEvent::End(tag) => match tag {
                Tag::Strong => bold = false,
                Tag::Emphasis => italic = false,
                _ => {}
            },
            MdEvent::Text(text) => {
                let s = text.to_string();
                let mut style = Style::default();
                if bold {
                    style = style.bold();
                    style = style.fg(Color::Yellow);
                } else if italic {
                    style = style.italic();
                }
                spans.push(Span::styled(s, style));
            }
            MdEvent::Code(text) => {
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                // Line breaks shouldn't happen in a single line
            }
            _ => {}
        }
    }

    spans
}

pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            let word_len = word.len();
            if current_line.is_empty() {
                // Starting a new line with this word
                if word_len > width {
                    // Long word doesn't fit on any line - wrap it to its own line now
                    // (it will overflow visually, which is acceptable)
                    lines.push(word.to_string());
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word_len <= width {
                // Word fits on current line
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                // Word doesn't fit - wrap to new line
                lines.push(std::mem::take(&mut current_line));
                if word_len > width {
                    // Long word gets its own line
                    lines.push(word.to_string());
                } else {
                    current_line = word.to_string();
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

pub fn cursor_position(input: &str, cursor_pos: usize, width: usize) -> (usize, usize) {
    // Handle empty input
    if input.is_empty() || cursor_pos == 0 {
        return (0, 0);
    }

    // Get substring up to cursor position
    let mut byte_pos = 0;
    for (char_idx, c) in input.chars().enumerate() {
        if char_idx >= cursor_pos {
            break;
        }
        byte_pos += c.len_utf8();
    }
    let substr = &input[..byte_pos];

    // Use wrap_text to wrap the substring
    let wrapped = wrap_text(substr, width);

    if wrapped.is_empty() {
        return (0, 0);
    }

    // Check if the cursor is at whitespace that was stripped
    // by comparing character count before and after split_whitespace
    let last_char = substr.chars().last();
    let ends_with_whitespace = last_char.is_some_and(|c| c.is_whitespace());

    if ends_with_whitespace && !wrapped.is_empty() {
        // Check if the whitespace is a newline (which moves to next line, col 0)
        let is_newline = last_char.is_some_and(|c| c == '\n');
        if is_newline {
            // Newline: cursor should be at the beginning of the newly created line
            // wrapped.len() includes the new empty line, so cursor is at wrapped.len() - 1
            return (0, wrapped.len() - 1);
        }

        // Cursor is at trailing whitespace that was stripped
        // We need to figure out if this whitespace would cause wrapping
        let last_line = wrapped.last().unwrap();
        let last_line_len = last_line.len();

        // If adding a space would exceed width, cursor is on next line
        if last_line_len >= width {
            return (0, wrapped.len());
        }

        // Otherwise cursor is at end of current line (after the space)
        // But wait - the space was stripped, so we need to add 1
        // However, we need to check if the space fits
        if last_line_len < width {
            return (last_line_len + 1, wrapped.len() - 1);
        } else {
            return (0, wrapped.len());
        }
    }

    // Normal case: cursor is at end of last wrapped line
    let last_line = wrapped.last().unwrap();
    (last_line.len(), wrapped.len() - 1)
}

fn get_syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_nonewlines)
}

fn get_theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub fn highlight_code_block(code: &str, language: &str) -> Vec<Vec<Span<'static>>> {
    // Special handling for diff blocks
    if language == "diff" || language == "patch" {
        return highlight_diff(code);
    }

    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    let theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .unwrap_or_else(|| {
            theme_set
                .themes
                .values()
                .next()
                .expect("at least one theme")
        });

    let syntax = syntax_set
        .find_syntax_by_token(language)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighted_lines = Vec::new();
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);

    for line in code.lines() {
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();

        let mut spans = Vec::new();
        for (style, text) in ranges {
            let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            spans.push(Span::styled(text.to_string(), Style::default().fg(color)));
        }
        if spans.is_empty() {
            spans.push(Span::raw(""));
        }
        highlighted_lines.push(spans);
    }

    highlighted_lines
}

fn highlight_diff(code: &str) -> Vec<Vec<Span<'static>>> {
    let mut highlighted_lines = Vec::new();

    for line in code.lines() {
        let spans = if line.starts_with("+++") || line.starts_with("---") {
            // File headers in cyan
            vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Cyan),
            )]
        } else if line.starts_with('+') && !line.starts_with("+++") {
            // Added lines in green
            vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Green),
            )]
        } else if line.starts_with('-') && !line.starts_with("---") {
            // Removed lines in red
            vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Red),
            )]
        } else if line.starts_with('@') {
            // Hunk headers in magenta
            vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Magenta),
            )]
        } else {
            // Context lines - default color
            vec![Span::raw(line.to_string())]
        };
        highlighted_lines.push(spans);
    }

    highlighted_lines
}

pub fn render_message(text: &str, width: usize) -> Vec<(String, Vec<Span<'static>>)> {
    let mut result = Vec::new();

    // Find all code block ranges
    #[derive(Clone, Copy)]
    struct CodeBlock {
        start: usize,
        end: usize,
        language: usize, // byte position of language start
        language_len: usize,
    }

    let mut code_blocks = Vec::new();
    let mut in_fence = false;
    let mut fence_start = 0;
    let mut fence_lang_start = 0;
    let mut fence_lang_len = 0;

    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Look for fence start (``` at start of line or after newline)
        if (i == 0 || bytes[i - 1] == b'\n') && i + 3 <= bytes.len() && &bytes[i..i + 3] == b"```" {
            if in_fence {
                // This closes the fence
                code_blocks.push(CodeBlock {
                    start: fence_start,
                    end: i,
                    language: fence_lang_start,
                    language_len: fence_lang_len,
                });
                in_fence = false;
                i += 3;
            } else {
                // This opens a fence
                in_fence = true;
                fence_start = i;
                i += 3;

                // Extract language identifier
                let line_end = bytes[i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|p| i + p)
                    .unwrap_or(bytes.len());
                fence_lang_start = i;
                fence_lang_len = line_end - i;
                i = line_end;
            }
        } else {
            i += 1;
        }
    }

    // Process text in segments (prose and code blocks)
    let mut pos = 0;

    for block in code_blocks {
        // Process prose before the block
        if pos < block.start {
            let prose = &text[pos..block.start];
            let wrapped = wrap_text(prose, width);
            for line_text in wrapped {
                let spans = parse_markdown_line(&line_text);
                result.push((line_text, spans));
            }
        }

        // Extract language tag
        let lang_bytes = &text.as_bytes()[block.language..block.language + block.language_len];
        let language = std::str::from_utf8(lang_bytes).unwrap_or("").trim();

        // Extract code block content (skip opening ``` line and closing ``` line)
        let fence_open_end = text[block.start..]
            .find('\n')
            .map(|p| block.start + p + 1)
            .unwrap_or(block.start + 3);
        let code_content = &text[fence_open_end..block.end];

        // Highlight and preserve code block lines as-is (no word wrap)
        let highlighted_lines = highlight_code_block(code_content, language);
        for (idx, spans) in highlighted_lines.into_iter().enumerate() {
            let line_text = if idx < code_content.lines().count() {
                code_content.lines().nth(idx).unwrap_or("").to_string()
            } else {
                String::new()
            };
            result.push((line_text, spans));
        }

        // Move past the closing fence
        pos = block.end + 3;
    }

    // Process remaining prose after last code block
    if pos < text.len() {
        let prose = &text[pos..];
        let wrapped = wrap_text(prose, width);
        for line_text in wrapped {
            let spans = parse_markdown_line(&line_text);
            result.push((line_text, spans));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== wrap_text tests ====================

    #[test]
    fn test_wrap_text_empty() {
        let lines = wrap_text("", 80);
        // Empty string produces vec![""] because split('\n') returns one empty string
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn test_wrap_text_single_word() {
        let lines = wrap_text("hello", 80);
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn test_wrap_text_no_wrap_needed() {
        let lines = wrap_text("hello world", 80);
        assert_eq!(lines, vec!["hello world"]);
    }

    #[test]
    fn test_wrap_text_simple_wrap() {
        let lines = wrap_text("hello world", 5);
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn test_wrap_text_multiple_lines() {
        let lines = wrap_text("one two three four", 8);
        assert_eq!(lines, vec!["one two", "three", "four"]);
    }

    #[test]
    fn test_wrap_text_preserves_empty_lines() {
        let lines = wrap_text("hello\n\nworld", 80);
        assert_eq!(lines, vec!["hello", "", "world"]);
    }

    #[test]
    fn test_wrap_text_long_word_overflow() {
        // Word longer than width gets its own line (overflows visually)
        let lines = wrap_text("supercalifragilistic", 10);
        assert_eq!(lines, vec!["supercalifragilistic"]);
    }

    #[test]
    fn test_wrap_text_exact_width() {
        // Word exactly at width boundary
        let lines = wrap_text("hello world", 5);
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn test_wrap_text_width_one() {
        // Extreme case: width of 1
        let lines = wrap_text("ab cd", 1);
        // Each word gets its own line, each char would overflow but we keep words
        assert_eq!(lines, vec!["ab", "cd"]);
    }

    #[test]
    fn test_wrap_text_multiple_paragraphs() {
        let lines = wrap_text("first para\n\nsecond para here", 10);
        assert_eq!(lines, vec!["first para", "", "second", "para here"]);
    }

    #[test]
    fn test_wrap_text_leading_trailing_whitespace() {
        // split_whitespace() handles this
        let lines = wrap_text("  hello  world  ", 80);
        assert_eq!(lines, vec!["hello world"]);
    }

    // ==================== parse_markdown_line tests ====================

    #[test]
    fn test_parse_markdown_line_empty() {
        let spans = parse_markdown_line("");
        assert!(spans.is_empty());
    }

    #[test]
    fn test_parse_markdown_line_plain_text() {
        let spans = parse_markdown_line("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello world");
    }

    #[test]
    fn test_parse_markdown_line_bold() {
        let spans = parse_markdown_line("**bold text**");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "bold text");
    }

    #[test]
    fn test_parse_markdown_line_italic() {
        let spans = parse_markdown_line("*italic text*");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "italic text");
    }

    #[test]
    fn test_parse_markdown_line_inline_code() {
        let spans = parse_markdown_line("`code`");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "code");
    }

    #[test]
    fn test_parse_markdown_line_mixed() {
        let spans = parse_markdown_line("hello **bold** and *italic* and `code`");
        assert_eq!(spans.len(), 6);
        assert_eq!(spans[0].content, "hello ");
        assert_eq!(spans[1].content, "bold");
        assert_eq!(spans[2].content, " and ");
        assert_eq!(spans[3].content, "italic");
        assert_eq!(spans[4].content, " and ");
        assert_eq!(spans[5].content, "code");
    }

    #[test]
    fn test_parse_markdown_line_double_asterisk_bold() {
        // __bold__ style
        let spans = parse_markdown_line("__bold__");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "bold");
    }

    #[test]
    fn test_parse_markdown_line_underscore_italic() {
        // _italic_ style
        let spans = parse_markdown_line("_italic_");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "italic");
    }

    // ==================== cursor_position tests ====================

    #[test]
    fn test_cursor_position_empty() {
        let (x, y) = cursor_position("", 0, 80);
        assert_eq!((x, y), (0, 0));
    }

    #[test]
    fn test_cursor_position_start() {
        let (x, y) = cursor_position("hello world", 0, 80);
        assert_eq!((x, y), (0, 0));
    }

    #[test]
    fn test_cursor_position_middle() {
        let (x, y) = cursor_position("hello world", 5, 80);
        assert_eq!((x, y), (5, 0));
    }

    #[test]
    fn test_cursor_position_end() {
        let (x, y) = cursor_position("hello", 5, 80);
        assert_eq!((x, y), (5, 0));
    }

    #[test]
    fn test_cursor_position_after_space() {
        let (x, y) = cursor_position("hello world", 6, 80);
        assert_eq!((x, y), (6, 0));
    }

    #[test]
    fn test_cursor_position_wrapped() {
        // "hello world" with width 8 wraps to ["hello", "world"]
        // Cursor at position 6 (at the space between "hello" and "world")
        // The space is at position 5, so we're just after the word "hello"
        let (x, y) = cursor_position("hello world", 6, 8);
        // "hello " wraps at width 8, "hello" is 5 chars + space = would exceed
        // Actually the space is stripped by wrap_text, so cursor is at end of line 0
        assert_eq!((x, y), (6, 0));
    }

    #[test]
    fn test_cursor_position_at_newline() {
        // Cursor right after \n should be at start of new line
        let (x, y) = cursor_position("hello\nworld", 6, 80);
        assert_eq!((x, y), (0, 1));
    }

    #[test]
    fn test_cursor_position_trailing_whitespace() {
        // Cursor at trailing space that gets stripped
        let (x, y) = cursor_position("hello ", 6, 80);
        assert_eq!((x, y), (6, 0));
    }

    #[test]
    fn test_cursor_position_multiline() {
        let (x, y) = cursor_position("line1\nline2\nline3", 12, 80);
        // "line1\nline2\n" = 12 chars, cursor at position 12 should be at start of line3
        assert_eq!((x, y), (0, 2));
    }

    #[test]
    fn test_cursor_position_beyond_length() {
        // Cursor position beyond string length
        let (x, y) = cursor_position("hi", 10, 80);
        assert_eq!((x, y), (2, 0));
    }

    // ==================== highlight_diff tests ====================

    #[test]
    fn test_highlight_diff_empty() {
        let lines = highlight_diff("");
        assert!(lines.is_empty());
    }

    #[test]
    fn test_highlight_diff_addition() {
        let lines = highlight_diff("+added line");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 1);
        assert_eq!(lines[0][0].content, "+added line");
    }

    #[test]
    fn test_highlight_diff_removal() {
        let lines = highlight_diff("-removed line");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0][0].content, "-removed line");
    }

    #[test]
    fn test_highlight_diff_file_header_plus() {
        let lines = highlight_diff("+++ b/src/main.rs");
        assert_eq!(lines[0][0].content, "+++ b/src/main.rs");
    }

    #[test]
    fn test_highlight_diff_file_header_minus() {
        let lines = highlight_diff("--- a/src/main.rs");
        assert_eq!(lines[0][0].content, "--- a/src/main.rs");
    }

    #[test]
    fn test_highlight_diff_hunk_header() {
        let lines = highlight_diff("@@ -1,5 +1,5 @@");
        assert_eq!(lines[0][0].content, "@@ -1,5 +1,5 @@");
    }

    #[test]
    fn test_highlight_diff_context() {
        let lines = highlight_diff(" context line");
        assert_eq!(lines[0][0].content, " context line");
    }

    #[test]
    fn test_highlight_diff_mixed() {
        let diff = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n context\n-removed\n+added";
        let lines = highlight_diff(diff);
        // File headers (2) + hunk header (1) + context (1) + removed (1) + added (1) = 6 lines
        assert_eq!(lines.len(), 6);
    }

    // ==================== highlight_code_block tests ====================

    #[test]
    fn test_highlight_code_block_rust() {
        let lines = highlight_code_block("fn main() {}", "rust");
        assert_eq!(lines.len(), 1);
        // Should have multiple spans for syntax highlighting
        assert!(!lines[0].is_empty());
    }

    #[test]
    fn test_highlight_code_block_unknown_lang() {
        // Unknown language falls back to plain text
        let lines = highlight_code_block("some text", "unknown_lang");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_highlight_code_block_empty() {
        let lines = highlight_code_block("", "rust");
        // Empty code produces no lines since .lines() returns empty iterator
        assert!(lines.is_empty());
    }

    #[test]
    fn test_highlight_code_block_multiline() {
        let code = "line1\nline2\nline3";
        let lines = highlight_code_block(code, "text");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_highlight_code_block_diff() {
        // Diff language uses special highlighting
        let lines = highlight_code_block("+added\n-removed", "diff");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_highlight_code_block_patch() {
        // "patch" is also treated as diff
        let lines = highlight_code_block("+added", "patch");
        assert_eq!(lines.len(), 1);
    }

    // ==================== render_message tests ====================

    #[test]
    fn test_render_message_plain_text() {
        let result = render_message("hello world", 80);
        assert!(!result.is_empty());
        assert_eq!(result[0].0, "hello world");
    }

    #[test]
    fn test_render_message_empty() {
        let result = render_message("", 80);
        assert!(result.is_empty());
    }

    #[test]
    fn test_render_message_with_code_block() {
        let msg = "Some text\n```rust\nfn main() {}\n```\nMore text";
        let result = render_message(msg, 80);
        // Should have lines for prose, code, and more prose
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_message_code_block_no_lang() {
        let msg = "```\nplain code\n```";
        let result = render_message(msg, 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_message_multiple_code_blocks() {
        let msg = "```rust\ncode1\n```\ntext\n```python\ncode2\n```";
        let result = render_message(msg, 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_message_wrapping() {
        let msg =
            "this is a very long line that should wrap to multiple lines when the width is small";
        let result = render_message(msg, 20);
        // Should produce multiple wrapped lines
        assert!(result.len() > 1);
    }
}
