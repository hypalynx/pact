use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "pact")]
struct Args {
    #[arg(long)]
    debug: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    text: String,
}

enum LlmEvent {
    Token(String),
    Done,
    Error(String),
}

struct App {
    messages: Vec<Message>,
    history: Vec<String>,
    history_index: Option<usize>,
    input: String,
    input_rect: Rect,
    messages_rect: Rect,
    rx: mpsc::Receiver<LlmEvent>,
    tx: mpsc::Sender<LlmEvent>,
    loading: bool,
    pending_response: String,
    debug: bool,
}

impl App {
    fn new(debug: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            messages: Vec::new(),
            history: Vec::new(),
            history_index: None,
            input: String::new(),
            input_rect: Rect::default(),
            messages_rect: Rect::default(),
            rx,
            tx,
            loading: false,
            pending_response: String::new(),
            debug,
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
            .messages
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| Message {
                role: m.role.clone(),
                text: m.text.clone(),
            })
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
        self.messages.push(Message {
            role: "user".to_string(),
            text: text.clone(),
        });
        self.history_index = None;
        self.save_history().ok();
        self.input = String::new();
        self.loading = true;
        self.pending_response.clear();

        let messages = self.messages.clone();
        let tx = self.tx.clone();
        let debug = self.debug;

        thread::spawn(move || {
            call_llm(messages, tx, debug);
        });
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

fn call_llm(messages: Vec<Message>, tx: mpsc::Sender<LlmEvent>, debug: bool) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(LlmEvent::Error(format!("Failed to create client: {}", e)));
            return;
        }
    };

    let msg_payload: Vec<_> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.text }))
        .collect();

    let body = json!({
        "model": "local",
        "max_tokens": 1024,
        "stream": true,
        "messages": msg_payload,
    });

    if debug {
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("api.log")
            .and_then(|mut file| {
                use std::io::Write;
                writeln!(file, "=== REQUEST {} ===", chrono::Local::now())?;
                writeln!(file, "POST http://127.0.0.1:7777/v1/messages")?;
                writeln!(file, "{}", serde_json::to_string_pretty(&body).unwrap_or_default())?;
                writeln!(file, "\n=== RESPONSE ===")?;
                Ok(())
            });
    }

    let response = match client
        .post("http://127.0.0.1:7777/v1/messages")
        .json(&body)
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(LlmEvent::Error(format!("Request failed: {}", e)));
            return;
        }
    };

    let mut log_file = if debug {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("api.log")
            .ok()
    } else {
        None
    };

    use std::io::{BufRead, BufReader, Write};
    let reader = BufReader::new(response);

    for line in reader.lines() {
        if let Ok(line) = line {
            if debug {
                if let Some(ref mut f) = log_file {
                    let _ = writeln!(f, "{}", line);
                }
            }

            if line == "data: [DONE]" {
                break;
            }

            if let Some(data_str) = line.strip_prefix("data: ") {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(data_str) {
                    if let Some(delta) = json_val
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        let _ = tx.send(LlmEvent::Token(delta.to_string()));
                    }
                }
            }
        }
    }

    if debug {
        if let Some(mut f) = log_file {
            let _ = writeln!(f, "===\n");
        }
    }

    let _ = tx.send(LlmEvent::Done);
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut terminal = ratatui::init();
    let mut app = App::new(args.debug);
    app.history = App::load_history().unwrap_or_default();

    loop {
        terminal.draw(|f| app.draw(f))?;

        // Check for LLM events
        while let Ok(event) = app.rx.try_recv() {
            match event {
                LlmEvent::Token(t) => app.pending_response.push_str(&t),
                LlmEvent::Done => {
                    let text = std::mem::take(&mut app.pending_response);
                    app.messages.push(Message {
                        role: "assistant".to_string(),
                        text,
                    });
                    app.loading = false;
                }
                LlmEvent::Error(e) => {
                    let text = format!("Error: {}", e);
                    app.messages.push(Message {
                        role: "assistant".to_string(),
                        text,
                    });
                    app.loading = false;
                }
            }
        }

        if event::poll(Duration::from_millis(16))? {
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
                    KeyCode::Char('j') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        app.input.push('\n');
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
    }

    ratatui::restore();
    Ok(())
}

impl App {
    fn draw(&mut self, frame: &mut Frame) {
        let margin = ratatui::layout::Margin::new(1, 1);
        let area = frame.area().inner(margin);

        let input_lines = (self.input.lines().count()
            + if self.input.ends_with('\n') { 1 } else { 0 })
        .max(1) as u16;
        let input_height = (input_lines + 2).min(10).max(3);

        let vertical = Layout::vertical([Constraint::Min(3), Constraint::Length(input_height)]);

        let [messages_area, input_area] = vertical.areas(area);

        self.messages_rect = messages_area;
        self.input_rect = input_area;

        self.draw_messages(frame);
        self.draw_input(frame);
    }

    fn draw_messages(&self, frame: &mut Frame) {
        let mut lines = Vec::new();

        for msg in &self.messages {
            if msg.role == "user" {
                // User message with dark gray background
                for line in msg.text.lines() {
                    lines.push(Line::from(
                        vec![
                            ratatui::text::Span::styled(
                                format!("  {}  ", line),
                                Style::default().bg(Color::DarkGray),
                            )
                        ]
                    ));
                }
            } else {
                // Assistant message (plain)
                for line in msg.text.lines() {
                    lines.push(Line::from(line.to_string()));
                }
            }
            lines.push(Line::from(""));
        }

        // Add pending response if streaming
        if !self.pending_response.is_empty() {
            for line in self.pending_response.lines() {
                lines.push(Line::from(line.to_string()));
            }
        }

        let line_count = lines.len() as u16;
        let y_offset = self
            .messages_rect
            .height
            .saturating_sub(line_count)
            .saturating_sub(1);

        let content_area = Rect {
            x: self.messages_rect.x,
            y: self.messages_rect.y + y_offset,
            width: self.messages_rect.width,
            height: line_count.min(self.messages_rect.height),
        };

        frame.render_widget(Paragraph::new(lines), content_area);
    }

    fn draw_input(&self, frame: &mut Frame) {
        let margin = Paragraph::new("").style(Style::default().bg(Color::Black));
        frame.render_widget(margin, self.input_rect);

        let inner = self.input_rect.inner(ratatui::layout::Margin {
            horizontal: 3,
            vertical: 1,
        });

        let input_text = if self.loading {
            "thinking...".to_string()
        } else {
            self.input.clone()
        };

        let input = Paragraph::new(input_text)
            .style(Style::default().fg(Color::White).bg(Color::Black));
        frame.render_widget(input, inner);

        if !self.loading {
            let (cursor_x, cursor_y) = self.cursor_position();
            let cursor_pos = ratatui::layout::Position {
                x: inner.x + cursor_x as u16,
                y: inner.y + cursor_y as u16,
            };
            frame.set_cursor_position(cursor_pos);
        }
    }

    fn cursor_position(&self) -> (usize, usize) {
        let mut x = 0;
        let mut y = 0;
        for c in self.input.chars() {
            if c == '\n' {
                y += 1;
                x = 0;
            } else {
                x += 1;
            }
        }
        (x, y)
    }
}
