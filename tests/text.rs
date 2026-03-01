use pact::text::{cursor_position, parse_markdown_line, wrap_text};

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

#[test]
fn test_cursor_position_no_wrap() {
    // Cursor at position 5 in "hello" with width 10
    let (x, y) = cursor_position("hello", 5, 10);
    assert_eq!(x, 5);
    assert_eq!(y, 0);
}

#[test]
fn test_cursor_position_at_end_of_line() {
    // Width 10, with two short words "abcde fghi" (10 chars including space)
    // "abcde" (5) + space (1) + "fghi" (4) = 10, fits exactly
    let lines = wrap_text("abcde fghi", 10);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "abcde fghi");

    // After all 9 chars, cursor should be at x=9, y=0
    let (x, y) = cursor_position("abcde fghi", 9, 10);
    assert_eq!(y, 0);
    assert_eq!(x, 9);
}

#[test]
fn test_cursor_position_second_line() {
    // Width 10, text wraps to second line
    // "hello world" = "hello" (5) + space + "world" (5) = 11 > 10
    // wrap_text: line 0 = "hello", line 1 = "world"
    let text = "hello world";
    let lines = wrap_text(text, 10);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "hello");
    assert_eq!(lines[1], "world");

    // After "hello" (5 chars), cursor is at end of first line
    let (x, y) = cursor_position(text, 5, 10);
    assert_eq!(y, 0);
    assert_eq!(x, 5);

    // After "hello " (6 chars), cursor is at position 6 on line 0
    // The space doesn't cause wrap until the next word is typed
    let (x, y) = cursor_position(text, 6, 10);
    assert_eq!(y, 0);
    assert_eq!(x, 6);

    // After "hello world" (11 chars), cursor at end of second line
    let (x, y) = cursor_position(text, 11, 10);
    assert_eq!(y, 1);
    assert_eq!(x, 5);
}

#[test]
fn test_cursor_position_first_line_full() {
    // Width 10, after typing "abcdefghij" (10 chars)
    // Cursor should be at x=10, y=0
    let (x, y) = cursor_position("abcdefghij", 10, 10);
    assert_eq!(y, 0);
    assert_eq!(x, 10);
}

#[test]
fn test_cursor_position_second_line_multiple_chars() {
    // Width 10, with words "abcde fghijklmnop" (17 chars)
    // "abcde" + space + "fghijklmnop" - but "fghijklmnop" is 11 chars > 10
    // So wrap_text should put "fghijklmnop" on its own line
    let text = "abcde fghijklmnop";
    let lines = wrap_text(text, 10);
    eprintln!("wrap_text result: {:?}", lines);
    // Long word stays on its own line

    // After 17 chars (end), cursor should be at x=11, y=1
    let (x, y) = cursor_position(text, 17, 10);
    eprintln!("cursor_position result: x={}, y={}", x, y);
    assert_eq!(y, 1);
    assert_eq!(x, 11);
}

#[test]
fn test_cursor_position_with_spaces() {
    // Width 10, text "hello world"
    // "hello" (5) + space (1) + "world" (5) = 11 > 10
    // So wrap_text: line 0 = "hello", line 1 = "world"
    let text = "hello world";
    let lines = wrap_text(text, 10);
    eprintln!("wrap_text of 'hello world': {:?}", lines);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "hello");
    assert_eq!(lines[1], "world");

    // After "hello " (6 chars), cursor is at position 6 on line 0
    // The space is on line 0, wrapping happens when next word starts
    let (x, y) = cursor_position(text, 6, 10);
    eprintln!("cursor_position at pos 6: x={}, y={}", x, y);
    assert_eq!(y, 0);
    assert_eq!(x, 6);
}
