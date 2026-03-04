use crate::app::App;
use crate::ui::layout::CONTROL_PANEL_WIDTH;
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Draw the control panel modal showing available panels and provider info.
pub fn draw_control_panel(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();
    let panel_height = 11_u16;
    let modal_x = (frame_area.width.saturating_sub(CONTROL_PANEL_WIDTH)) / 2;
    let modal_y = (frame_area.height.saturating_sub(panel_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: CONTROL_PANEL_WIDTH,
        height: panel_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::EMPTY)
        .title(" Control Panel ")
        .style(Style::default().bg(Color::Black));

    let inner = modal_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let mut lines = vec![
        Line::from(vec![Span::raw("Available Panels:")]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "[D] Debug - API Logs & Performance",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    // Show provider info and switch option
    lines.push(Line::from(""));
    let current_provider = app
        .active_provider
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("local");
    let current_model = app
        .active_provider
        .as_ref()
        .and_then(|p| p.default_model.as_ref())
        .map(|m| m.as_str())
        .unwrap_or("local");

    if app.providers.len() > 1 {
        lines.push(Line::from(vec![Span::styled(
            format!("[P] Switch Provider ({})", current_provider),
            Style::default().fg(Color::Yellow),
        )]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            format!("Provider: {}", current_provider),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    // Show current model
    lines.push(Line::from(vec![Span::styled(
        format!("Model: {}", current_model),
        Style::default().fg(Color::DarkGray),
    )]));

    let text = Paragraph::new(lines).style(Style::default().bg(Color::Black));
    frame.render_widget(block, modal_area);
    frame.render_widget(Clear, inner);
    frame.render_widget(text, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, PanelState};
    use indexmap::IndexMap;

    fn create_test_app() -> App {
        App::new(
            false,
            None,
            "default".to_string(),
            IndexMap::new(),
            None,
            "test-session".to_string(),
            "/tmp".to_string(),
            vec![],
        )
    }

    #[test]
    fn test_control_panel_width_constant() {
        let _: u16 = CONTROL_PANEL_WIDTH;
        assert!(CONTROL_PANEL_WIDTH > 0);
    }

    #[test]
    fn test_panel_state_variants() {
        // Ensure PanelState variants exist and are distinct
        let none = PanelState::None;
        let control = PanelState::ControlPanel;
        let debug = PanelState::Debug;

        assert!(matches!(none, PanelState::None));
        assert!(matches!(control, PanelState::ControlPanel));
        assert!(matches!(debug, PanelState::Debug));
    }

    #[test]
    fn test_app_default_provider_is_local() {
        let app = create_test_app();
        let provider = app.active_provider.as_ref().map(|p| p.name.as_str());
        assert!(provider.is_none() || provider == Some("local"));
    }
}
