use pulldown_cmark::{Event as MdEvent, Parser as MdParser, Tag};
use ratatui::style::{Color, Style};
use ratatui::text::Span;

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
