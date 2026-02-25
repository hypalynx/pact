use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph},
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    text: String,
}

struct App {
    messages: Vec<Message>,
    history: Vec<String>,
    history_index: Option<usize>,
    input: String,
    input_rect: Rect,
    messages_rect: Rect,
}

impl App {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            history: Vec::new(),
            history_index: None,
            input: String::new(),
            input_rect: Rect::default(),
            messages_rect: Rect::default(),
        }
    }

    fn messages_path() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".pact");
        fs::create_dir_all(&path).ok();
        path.push("messages.json");
        path
    }

    fn load_history() -> io::Result<Vec<String>> {
        let path = Self::messages_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        let messages: Vec<Message> = serde_json::from_str(&content).unwrap_or_default();
        Ok(messages.into_iter().map(|m| m.text).collect())
    }

    fn save_history(&self) -> io::Result<()> {
        let path = Self::messages_path();
        let messages: Vec<Message> = self
            .history
            .iter()
            .map(|t| Message { text: t.clone() })
            .collect();
        let content = serde_json::to_string_pretty(&messages)?;
        fs::write(path, content)
    }

    fn submit_message(&mut self) {
        if self.input.trim().is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.input);
        self.history.push(text.clone());
        self.messages.push(Message { text });
        self.history_index = None;
        self.save_history().ok();
        self.input = String::new();
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let new_index = match self.history_index {
            None => self.history.len() - 1,
            Some(i) if i == 0 => return,
            Some(i) => i - 1,
        };
        self.history_index = Some(new_index);
        self.input = self.history[new_index].clone();
    }

    fn history_down(&mut self) {
        let new_index = match self.history_index {
            None => return,
            Some(i) if i >= self.history.len() - 1 => {
                self.history_index = None;
                self.input.clear();
                return;
            }
            Some(i) => i + 1,
        };
        self.history_index = Some(new_index);
        self.input = self.history[new_index].clone();
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.history = App::load_history().unwrap_or_default();

    loop {
        terminal.draw(|f| app.draw(f))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    if app.input.is_empty() {
                        break;
                    } else {
                        app.input.clear();
                        app.history_index = None;
                    }
                }
                KeyCode::Char(c) => app.input.push(c),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Enter => app.submit_message(),
                KeyCode::Up => app.history_up(),
                KeyCode::Down => app.history_down(),
                _ => {}
            }
        }
    }

    ratatui::restore();
    Ok(())
}

impl App {
    fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]);

        let [messages_area, input_area] = vertical.areas(frame.area());

        self.messages_rect = messages_area;
        self.input_rect = input_area;

        self.draw_messages(frame);
        self.draw_input(frame);
    }

    fn draw_messages(&self, frame: &mut Frame) {
        let border = Block::bordered().title("Messages");
        frame.render_widget(border, self.messages_rect);

        let inner = self.messages_rect.inner(ratatui::layout::Margin::new(1, 1));

        let mut lines = Vec::new();
        for msg in &self.messages {
            lines.push(Line::from(msg.text.clone()));
            lines.push(Line::from(""));
        }

        let line_count = lines.len() as u16;
        let y_offset = inner.height.saturating_sub(line_count);

        let content_area = Rect {
            x: inner.x,
            y: inner.y + y_offset,
            width: inner.width,
            height: line_count.min(inner.height),
        };

        frame.render_widget(Paragraph::new(lines), content_area);
    }

    fn draw_input(&self, frame: &mut Frame) {
        let input = Paragraph::new(self.input.as_str())
            .block(Block::bordered().title("Type here (Enter to submit, Ctrl+C to clear/quit)"))
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(input, self.input_rect);
    }
}
