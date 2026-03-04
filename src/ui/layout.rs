// Input box layout constants
pub const INPUT_MIN_HEIGHT: u16 = 3;
pub const INPUT_MAX_HEIGHT: u16 = 20;
pub const INPUT_HORIZONTAL_MARGIN: u16 = 3;
pub const INPUT_VERTICAL_MARGIN: u16 = 1;

// Control panel constants
pub const CONTROL_PANEL_WIDTH: u16 = 40;

// Debug modal constants
pub const DEBUG_MODAL_WIDTH_PERCENT: u16 = 9; // out of 10
pub const DEBUG_MODAL_HEIGHT_PERCENT: u16 = 8; // out of 10
pub const DEBUG_MODAL_MIN_WIDTH: u16 = 40;
pub const DEBUG_MODAL_MIN_HEIGHT: u16 = 10;
pub const DEBUG_FILE_PICKER_MAX_VISIBLE: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_constants_exist() {
        // Just verify constants are defined with correct types
        let _: u16 = INPUT_MIN_HEIGHT;
        let _: u16 = INPUT_MAX_HEIGHT;
        let _: u16 = INPUT_HORIZONTAL_MARGIN;
        let _: u16 = INPUT_VERTICAL_MARGIN;
        let _: u16 = CONTROL_PANEL_WIDTH;
        let _: u16 = DEBUG_MODAL_WIDTH_PERCENT;
        let _: u16 = DEBUG_MODAL_HEIGHT_PERCENT;
        let _: u16 = DEBUG_MODAL_MIN_WIDTH;
        let _: u16 = DEBUG_MODAL_MIN_HEIGHT;
        let _: usize = DEBUG_FILE_PICKER_MAX_VISIBLE;
    }

    #[test]
    fn test_input_height_bounds() {
        // Verify min <= max for input height
        assert!(INPUT_MIN_HEIGHT <= INPUT_MAX_HEIGHT);
        assert!(INPUT_MIN_HEIGHT > 0);
        assert!(INPUT_MAX_HEIGHT > 0);
    }

    #[test]
    fn test_debug_modal_percentages() {
        // Verify percentages are reasonable (out of 10)
        assert!(DEBUG_MODAL_WIDTH_PERCENT > 0);
        assert!(DEBUG_MODAL_WIDTH_PERCENT <= 10);
        assert!(DEBUG_MODAL_HEIGHT_PERCENT > 0);
        assert!(DEBUG_MODAL_HEIGHT_PERCENT <= 10);
    }

    #[test]
    fn test_debug_modal_min_sizes() {
        // Verify minimum sizes are reasonable
        assert!(DEBUG_MODAL_MIN_WIDTH > 0);
        assert!(DEBUG_MODAL_MIN_HEIGHT > 0);
    }
}
