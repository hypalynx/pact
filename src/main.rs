use chrono::{DateTime, Local};
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
    id: u64,
    text: String,
    timestamp: DateTime<Local>,
}

struct App {
    messages: Vec<Message>,
    input: String,
    next_id: u64,
    scroll_offset: usize,
    input_rect: Rect,
    messages_rect: Rect,
}

impl App {
    fn new() -> Self {
        let messages = Self::load_messages().unwrap_or_default();
        let next_id = messages
            .iter()
            .map(|m| m.id)
            .max()
            .map(|id| id + 1)
            .unwrap_or(1);
        Self {
            messages,
            input: String::new(),
            next_id,
            scroll_offset: 0,
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

    fn load_messages() -> io::Result<Vec<Message>> {
        let path = Self::messages_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        let messages: Vec<Message> = serde_json::from_str(&content).unwrap_or_default();
        Ok(messages)
    }

    fn save_messages(&self) -> io::Result<()> {
        let path = Self::messages_path();
        let content = serde_json::to_string_pretty(&self.messages)?;
        fs::write(path, content)
    }

    fn submit_message(&mut self) {
        if self.input.trim().is_empty() {
            return;
        }
        let message = Message {
            id: self.next_id,
            text: std::mem::take(&mut self.input),
            timestamp: Local::now(),
        };
        self.next_id += 1;
        self.messages.push(message);
        self.save_messages().ok();
        self.input = String::new();
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self) {
        let max_scroll = self.messages.len().saturating_sub(1);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();

    loop {
        terminal.draw(|f| app.draw(f))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char(c) => app.input.push(c),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Enter => app.submit_message(),
                KeyCode::Up => app.scroll_up(),
                KeyCode::Down => app.scroll_down(),
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
        let visible_count = self.messages_rect.height as usize / 4;
        let start = self.scroll_offset;
        let end = (start + visible_count).min(self.messages.len());

        let mut lines = Vec::new();

        for i in start..end {
            let msg = &self.messages[i];
            let time = msg.timestamp.format("%H:%M").to_string();
            lines.push(Line::from(vec![
                format!("[{}] ", time).into(),
                msg.text.clone().into(),
            ]));
            lines.push(Line::from(""));
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::bordered().title("Messages"))
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, self.messages_rect);

        if self.messages.len() > visible_count {
            let scrollbar = Paragraph::new("")
                .block(Block::bordered())
                .scroll((self.scroll_offset as u16, 0));
            let sb_area = Rect {
                x: self.messages_rect.right() - 3,
                y: self.messages_rect.y,
                width: 3,
                height: self.messages_rect.height,
            };
            frame.render_widget(scrollbar, sb_area);
        }
    }

    fn draw_input(&self, frame: &mut Frame) {
        let input = Paragraph::new(self.input.as_str())
            .block(Block::bordered().title("Type here (Enter to submit, q to quit)"))
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(input, self.input_rect);
    }
}
