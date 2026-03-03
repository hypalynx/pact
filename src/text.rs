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
