use ratatui::style::Color;

// Dimmed color palette (used when modal is open)
pub const DIM_BG: Color = Color::Rgb(20, 20, 20); // Darker background for input/message boxes
pub const DIM_TEXT: Color = Color::DarkGray; // Muted text color
pub const DIM_THINKING: Color = Color::Rgb(60, 60, 60); // Even darker for thinking/tool text
pub const DIM_STATUS: Color = Color::Rgb(60, 60, 60); // Dark gray for status bar elements
pub const DIM_MODE: Color = Color::Rgb(100, 100, 100); // Grayed out mode color
pub const DIM_ERROR: Color = Color::Rgb(100, 0, 0); // Muted red for errors
pub const DIM_COPYING: Color = Color::Rgb(100, 100, 0); // Muted yellow for copy notification
pub const DIM_WARN: Color = Color::Rgb(100, 100, 0); // Muted yellow for warnings
pub const DIM_AT: Color = Color::Rgb(100, 100, 0); // Muted @mention highlight
pub const DIM_SCROLLBAR: Color = Color::Rgb(40, 40, 40); // Darker scrollbar when dimmed

// Thinking text markdown colors (darker versions to match thinking aesthetic)
pub const THINKING_BOLD: Color = Color::Rgb(200, 170, 0); // Darker yellow for bold in thinking
pub const THINKING_CODE: Color = Color::Rgb(100, 180, 200); // Darker cyan for code in thinking

/// Parse a color string into a ratatui Color.
/// Supports standard color names (case-insensitive).
pub fn parse_color(color_str: &str) -> Color {
    match color_str.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "dark_gray" | "darkgray" | "dark-gray" => Color::DarkGray,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_standard() {
        assert_eq!(parse_color("red"), Color::Red);
        assert_eq!(parse_color("green"), Color::Green);
        assert_eq!(parse_color("blue"), Color::Blue);
        assert_eq!(parse_color("yellow"), Color::Yellow);
        assert_eq!(parse_color("magenta"), Color::Magenta);
        assert_eq!(parse_color("cyan"), Color::Cyan);
        assert_eq!(parse_color("white"), Color::White);
        assert_eq!(parse_color("black"), Color::Black);
    }

    #[test]
    fn test_parse_color_case_insensitive() {
        assert_eq!(parse_color("RED"), Color::Red);
        assert_eq!(parse_color("Red"), Color::Red);
        assert_eq!(parse_color("rEd"), Color::Red);
        assert_eq!(parse_color("GREEN"), Color::Green);
        assert_eq!(parse_color("Blue"), Color::Blue);
    }

    #[test]
    fn test_parse_color_gray_variants() {
        assert_eq!(parse_color("gray"), Color::Gray);
        assert_eq!(parse_color("grey"), Color::Gray);
        assert_eq!(parse_color("GRAY"), Color::Gray);
        assert_eq!(parse_color("GREY"), Color::Gray);
    }

    #[test]
    fn test_parse_color_dark_gray_variants() {
        assert_eq!(parse_color("dark_gray"), Color::DarkGray);
        assert_eq!(parse_color("darkgray"), Color::DarkGray);
        assert_eq!(parse_color("dark-gray"), Color::DarkGray);
        assert_eq!(parse_color("DARK_GRAY"), Color::DarkGray);
        assert_eq!(parse_color("DarkGray"), Color::DarkGray);
    }

    #[test]
    fn test_parse_color_unknown_defaults_to_white() {
        assert_eq!(parse_color("unknown"), Color::White);
        assert_eq!(parse_color(""), Color::White);
        assert_eq!(parse_color("orange"), Color::White);
        assert_eq!(parse_color("purple"), Color::White);
    }

    #[test]
    fn test_dimmed_colors_exist() {
        // Verify dimmed colors are Color type
        let _: Color = DIM_BG;
        let _: Color = DIM_TEXT;
        let _: Color = DIM_THINKING;
        let _: Color = DIM_STATUS;
        let _: Color = DIM_MODE;
        let _: Color = DIM_ERROR;
        let _: Color = DIM_COPYING;
        let _: Color = DIM_WARN;
        let _: Color = DIM_AT;
        let _: Color = DIM_SCROLLBAR;
    }
}
