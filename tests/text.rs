use pact::text::{parse_markdown_line, wrap_text};

#[test]
fn test_wrap_text_simple() {
    let text = "Hello world";
    let lines = wrap_text(text, 20);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "Hello world");
}

#[test]
fn test_wrap_text_long_line() {
    let text = "This is a very long line that should definitely be wrapped";
    let lines = wrap_text(text, 15);
    assert!(lines.len() > 1);
    for line in &lines {
        assert!(line.len() <= 15, "Line '{}' exceeds width of 15", line);
    }
}

#[test]
fn test_wrap_text_with_newlines() {
    let text = "First paragraph\nSecond paragraph";
    let lines = wrap_text(text, 50);
    assert!(lines.len() >= 2);
}

#[test]
fn test_wrap_text_empty_lines() {
    let text = "Line one\n\nLine three";
    let lines = wrap_text(text, 50);
    assert_eq!(lines[1], "");
}

#[test]
fn test_wrap_text_word_boundary() {
    let text = "Hello world test";
    let lines = wrap_text(text, 11);
    assert_eq!(lines[0], "Hello world");
    assert_eq!(lines[1], "test");
}

#[test]
fn test_wrap_text_single_long_word() {
    let text = "Supercalifragilisticexpialidocious";
    let lines = wrap_text(text, 10);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], text);
}

#[test]
fn test_wrap_text_long_word_wraps_early() {
    // When a word is wider than the width, it should wrap to its own line
    // rather than waiting for the next word to trigger wrapping
    let text = "hi Supercalifragilisticexpialidocious";
    let lines = wrap_text(text, 10);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "hi");
    assert_eq!(lines[1], "Supercalifragilisticexpialidocious");
}

#[test]
fn test_wrap_text_long_word_after_partial_line() {
    // A long word should wrap to a new line even when following partial text
    let text = "hello world Supercalifragilisticexpialidocious";
    let lines = wrap_text(text, 15);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "hello world");
    assert_eq!(lines[1], "Supercalifragilisticexpialidocious");
}

#[test]
fn test_wrap_text_exact_fit() {
    let text = "Hello";
    let lines = wrap_text(text, 5);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "Hello");
}

#[test]
fn test_parse_markdown_bold() {
    let text = "**bold text**";
    let spans = parse_markdown_line(text);
    assert!(!spans.is_empty());
}

#[test]
fn test_parse_markdown_italic() {
    let text = "*italic text*";
    let spans = parse_markdown_line(text);
    assert!(!spans.is_empty());
}

#[test]
fn test_parse_markdown_code() {
    let text = "`code`";
    let spans = parse_markdown_line(text);
    assert!(!spans.is_empty());
}

#[test]
fn test_parse_markdown_mixed() {
    let text = "**bold** and *italic* and `code`";
    let spans = parse_markdown_line(text);
    assert!(spans.len() >= 3);
}

#[test]
fn test_parse_markdown_plain_text() {
    let text = "plain text without any formatting";
    let spans = parse_markdown_line(text);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "plain text without any formatting");
}
