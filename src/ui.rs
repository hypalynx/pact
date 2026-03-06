use crate::app::App;
use crate::text::wrap_text;
use crate::ui::confirmations::draw_bash_confirm;
use crate::ui::debug::draw_debug_modal;
use crate::ui::input::draw_input;
use crate::ui::layout::*;
use crate::ui::messages::draw_messages;
use crate::ui::panels::draw_control_panel;
use crate::ui::pickers::{draw_api_key_input, draw_file_picker, draw_slash_picker};
use crate::ui::status::draw_status;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

pub mod colors;
pub mod confirmations;
pub mod debug;
pub mod input;
pub mod layout;
pub mod messages;
pub mod panels;
pub mod pickers;
pub mod status;

pub fn draw_app(app: &mut App, frame: &mut Frame) {
    let margin = ratatui::layout::Margin::new(1, 1);
    let area = frame.area().inner(margin);

    // Calculate input height based on wrapped lines, not just actual newlines
    let available_input_width = (area.width.saturating_sub(INPUT_HORIZONTAL_MARGIN * 2)) as usize;
    let wrapped_lines = wrap_text(&app.input, available_input_width);
    let input_height = ((wrapped_lines.len() + 2) as u16).clamp(INPUT_MIN_HEIGHT, INPUT_MAX_HEIGHT);

    let vertical = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(input_height),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);

    let [messages_area, _gap1, input_area, _gap2, status_area] = vertical.areas(area);

    app.messages_rect = messages_area;
    app.input_rect = input_area;

    // Check if any modal is open (excluding small pickers which don't need dimming)
    let is_modal_open = matches!(
        app.panel_state,
        crate::app::PanelState::ControlPanel | crate::app::PanelState::Debug
    ) || app.api_key_input.is_some()
        || app.pending_bash_confirm.is_some();

    draw_messages(app, frame, is_modal_open);
    draw_input(app, frame, is_modal_open);
    draw_status(app, frame, status_area, is_modal_open);

    // Note: We intentionally don't draw a solid overlay here.
    // The modals (control panel, debug, etc.) have their own backgrounds
    // that provide sufficient contrast with the main UI.
    // A solid color overlay would hide the UI behind it completely.

    // Draw panels
    match app.panel_state {
        crate::app::PanelState::None => {}
        crate::app::PanelState::ControlPanel => {
            draw_control_panel(app, frame);
        }
        crate::app::PanelState::Debug => {
            draw_debug_modal(app, frame);
        }
    }

    // Draw file picker if open
    if app.file_picker.is_some() {
        draw_file_picker(app, frame);
    }

    // Draw slash picker if open
    if app.slash_picker.is_some() {
        draw_slash_picker(app, frame);
    }

    // Draw API key input prompt if active
    if app.api_key_input.is_some() {
        draw_api_key_input(app, frame);
    }

    // Draw bash confirmation prompt if active
    if app.pending_bash_confirm.is_some() {
        draw_bash_confirm(app, frame);
    }
}
