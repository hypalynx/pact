use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind, EnableMouseCapture, DisableMouseCapture};
use crossterm::execute;
use pulldown_cmark::{Parser as MdParser, Event as MdEvent, Tag};
use ratatui::Frame;
use std::io::stdout;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
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
    Usage { input_tokens: usize, output_tokens: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Build,
    Plan,
}

struct App {
    messages: Vec<Message>,
    history: Vec<String>,
    history_index: Option<usize>,
    input: String,
    cursor_pos: usize,
    input_rect: Rect,
    messages_rect: Rect,
    rx: mpsc::Receiver<LlmEvent>,
    tx: mpsc::Sender<LlmEvent>,
    loading: bool,
    pending_response: String,
    debug: bool,
    scroll_offset: usize,
    user_scrolled: bool,
    was_at_bottom: bool,
    dragging_scrollbar: bool,
    mode: Mode,
    context_window: usize,
    total_input_tokens: usize,
    total_output_tokens: usize,
    frame_count: u32,
}

impl App {
    fn new(debug: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            messages: Vec::new(),
            history: Vec::new(),
            history_index: None,
            input: String::new(),
            cursor_pos: 0,
            input_rect: Rect::default(),
            messages_rect: Rect::default(),
            rx,
            tx,
            loading: false,
            pending_response: String::new(),
            debug,
            scroll_offset: 0,
            user_scrolled: false,
            was_at_bottom: true,
            dragging_scrollbar: false,
            mode: Mode::Build,
            context_window: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            frame_count: 0,
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
        self.cursor_pos = 0;
        self.loading = true;
        self.pending_response.clear();
        // Reset user_scrolled so the response auto-scrolls to the bottom
        self.user_scrolled = false;

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
        self.cursor_pos = self.input.len();
    }

    fn history_down(&mut self) {
        let new_index = match self.history_index {
            None => return,
            Some(i) if i >= self.history.len() - 1 => {
                self.history_index = None;
                self.input.clear();
                self.cursor_pos = 0;
                return;
            }
            Some(i) => i + 1,
        };
        self.history_index = Some(new_index);
        self.input = self.history[new_index].clone();
        self.cursor_pos = self.input.len();
    }

    fn calculate_total_lines(&self) -> usize {
        // Calculate total line count from all messages
        let mut total_lines = 0;
        let available_width = (self.messages_rect.width.saturating_sub(4)) as usize;

        for msg in &self.messages {
            let wrapped = wrap_text(&msg.text, available_width);
            total_lines += wrapped.len() + 1; // +1 for blank line
        }

        if !self.pending_response.is_empty() {
            let wrapped = wrap_text(&self.pending_response, available_width);
            total_lines += wrapped.len();
        }

        total_lines
    }

    fn scroll_up(&mut self) {
        // Scrolling up shows earlier content (decrease absolute line number)
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
        self.user_scrolled = true;
    }

    fn scroll_down(&mut self) {
        let total_lines = self.calculate_total_lines();
        let max_scroll = total_lines.saturating_sub(self.messages_rect.height as usize);
        // Scrolling down shows later content (increase absolute line number)
        self.scroll_offset = (self.scroll_offset + 3).min(max_scroll);
        // If we're at the bottom, user is no longer manually scrolled
        if self.scroll_offset >= max_scroll {
            self.user_scrolled = false;
        }
    }

    fn calculate_scroll_info(&self) -> (bool, usize) {
        let total_lines = self.calculate_total_lines();
        let max_scroll = total_lines.saturating_sub(self.messages_rect.height as usize);
        let at_bottom = self.scroll_offset >= max_scroll;

        (at_bottom, total_lines)
    }

    fn handle_scrollbar_click(&mut self, mouse_y: u16) {
        let (_at_bottom, total_lines) = self.calculate_scroll_info();
        if total_lines as u16 <= self.messages_rect.height {
            return;
        }

        // scrollbar_height is the visual height of the thumb
        let scrollbar_height = (self.messages_rect.height as f64 * self.messages_rect.height as f64 / total_lines as f64).max(1.0) as u16;
        let scrollable_height = self.messages_rect.height.saturating_sub(scrollbar_height);
        let scrollable_lines = total_lines.saturating_sub(self.messages_rect.height as usize);

        // Where in the scrollbar (relative to messages_rect.y) did the user click?
        let click_offset = mouse_y.saturating_sub(self.messages_rect.y).min(scrollable_height);

        // Map to scroll offset (absolute line number to start from)
        if scrollable_height > 0 {
            let proportion = click_offset as f64 / scrollable_height as f64;
            // Clicking at bottom (proportion=1) should show the last page
            self.scroll_offset = (proportion * scrollable_lines as f64) as usize;
        } else {
            self.scroll_offset = 0;
        }

        self.user_scrolled = true;
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            let byte_pos = self.input
                .char_indices()
                .filter(|(i, _)| *i < self.cursor_pos)
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.remove(byte_pos);
            self.cursor_pos = self.cursor_pos.saturating_sub(1);
        }
    }

    fn move_cursor_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    fn move_cursor_to_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    fn kill_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let input_chars: Vec<char> = self.input.chars().collect();
        let mut pos = self.cursor_pos.saturating_sub(1);

        // Skip any whitespace
        while pos > 0 && input_chars[pos].is_whitespace() {
            pos = pos.saturating_sub(1);
        }

        // Find word boundary
        while pos > 0 && !input_chars[pos - 1].is_whitespace() {
            pos = pos.saturating_sub(1);
        }

        // Delete from pos to cursor_pos
        let byte_pos = self.input
            .chars()
            .take(pos)
            .map(|c| c.len_utf8())
            .sum::<usize>();
        let byte_end = self.input
            .chars()
            .take(self.cursor_pos)
            .map(|c| c.len_utf8())
            .sum::<usize>();

        self.input.drain(byte_pos..byte_end);
        self.cursor_pos = pos;
    }

    fn kill_line(&mut self) {
        // Clear from start of current line to cursor
        // Find the start of the current line (last newline before cursor, or start of input)
        let line_start = self.input[..self.cursor_pos]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        self.input.drain(line_start..self.cursor_pos);
        self.cursor_pos = line_start;
    }
}

fn parse_markdown_line(text: &str) -> Vec<Span<'static>> {
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
                    // Add amber/yellow color for bold text
                    style = style.fg(Color::Yellow);
                } else if italic {
                    style = style.italic();
                }
                spans.push(Span::styled(s, style));
            }
            MdEvent::Code(text) => {
                spans.push(Span::styled(text.to_string(), Style::default().fg(Color::Cyan)));
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                // Line breaks shouldn't happen in a single line
            }
            _ => {}
        }
    }

    spans
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
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
                    // Check for token deltas
                    if let Some(delta) = json_val
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        let _ = tx.send(LlmEvent::Token(delta.to_string()));
                    }

                    // Check for usage in message_delta or message_stop events
                    if let Some(usage) = json_val.get("usage") {
                        let input_tokens = usage.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as usize;
                        let output_tokens = usage.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as usize;

                        if input_tokens > 0 || output_tokens > 0 {
                            let _ = tx.send(LlmEvent::Usage {
                                input_tokens,
                                output_tokens,
                            });
                        }
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

fn get_pwd_display() -> String {
    match std::env::current_dir() {
        Ok(path) => {
            let home = dirs::home_dir();
            let path_str = path.to_string_lossy().to_string();

            if let Some(home_path) = home {
                let home_str = home_path.to_string_lossy().to_string();
                if path_str.starts_with(&home_str) {
                    let remainder = path_str[home_str.len()..].to_string();
                    if remainder.is_empty() {
                        "~".to_string()
                    } else {
                        format!("~{}", remainder)
                    }
                } else {
                    path_str
                }
            } else {
                path_str
            }
        }
        Err(_) => ".".to_string(),
    }
}

fn get_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

fn format_tokens(tokens: usize) -> String {
    if tokens >= 1000 {
        format!("{:.0}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    }
}

fn fetch_context_window() -> usize {
    // Try to fetch models from the API
    let client = reqwest::blocking::Client::new();

    // Try /v1/models endpoint
    if let Ok(response) = client.get("http://127.0.0.1:7777/v1/models").send() {
        if let Ok(text) = response.text() {
            // Try to parse the response as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                // Look for data array with models
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if let Some(first_model) = data.first() {
                        // Try to get max_tokens from first model
                        if let Some(max_tokens) = first_model.get("max_tokens").and_then(|m| m.as_u64()) {
                            return max_tokens as usize;
                        }
                    }
                }

                // Alternative: try single model response
                if let Some(max_tokens) = json.get("max_tokens").and_then(|m| m.as_u64()) {
                    return max_tokens as usize;
                }
            }
        }
    }

    // Fallback to default
    65535
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut terminal = ratatui::init();

    // Enable mouse support
    execute!(stdout(), EnableMouseCapture)?;

    let mut app = App::new(args.debug);
    app.history = App::load_history().unwrap_or_default();
    app.context_window = fetch_context_window();

    loop {
        terminal.draw(|f| app.draw(f))?;

        // Process LLM events
        while let Ok(event) = app.rx.try_recv() {
            match event {
                LlmEvent::Token(t) => {
                    app.pending_response.push_str(&t);
                    // Only auto-scroll if user hasn't manually scrolled
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                LlmEvent::Done => {
                    let text = std::mem::take(&mut app.pending_response);
                    app.messages.push(Message {
                        role: "assistant".to_string(),
                        text,
                    });
                    app.loading = false;
                    // Only auto-scroll if user hasn't manually scrolled
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                LlmEvent::Error(e) => {
                    let text = format!("Error: {}", e);
                    app.messages.push(Message {
                        role: "assistant".to_string(),
                        text,
                    });
                    app.loading = false;
                    // Only auto-scroll if user hasn't manually scrolled
                    if !app.user_scrolled {
                        let new_line_count = app.calculate_total_lines();
                        let height = app.messages_rect.height as usize;
                        app.scroll_offset = new_line_count.saturating_sub(height);
                    }
                }
                LlmEvent::Usage { input_tokens, output_tokens } => {
                    app.total_input_tokens += input_tokens;
                    app.total_output_tokens += output_tokens;
                }
            }
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            if app.input.is_empty() {
                                break;
                            } else {
                                app.input.clear();
                                app.cursor_pos = 0;
                                app.history_index = None;
                            }
                        }
                        KeyCode::Char('a') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.move_cursor_to_start();
                        }
                        KeyCode::Char('e') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.move_cursor_to_end();
                        }
                        KeyCode::Char('w') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.kill_word_backward();
                        }
                        KeyCode::Char('u') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.kill_line();
                        }
                        KeyCode::Char('j') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            app.insert_char('\n');
                        }
                        KeyCode::Char(c) => app.insert_char(c),
                        KeyCode::Backspace => {
                            app.delete_char();
                        }
                        KeyCode::Enter => app.submit_message(),
                        KeyCode::Tab => {
                            app.mode = match app.mode {
                                Mode::Build => Mode::Plan,
                                Mode::Plan => Mode::Build,
                            };
                        }
                        KeyCode::Up => app.history_up(),
                        KeyCode::Down => app.history_down(),
                        KeyCode::PageUp => app.scroll_up(),
                        KeyCode::PageDown => app.scroll_down(),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => app.scroll_up(),
                        MouseEventKind::ScrollDown => app.scroll_down(),
                        MouseEventKind::Down(_) => {
                            // Check if click is on the scrollbar (right edge of messages area)
                            let scrollbar_x = app.messages_rect.x + app.messages_rect.width.saturating_sub(1);
                            if mouse.column == scrollbar_x && mouse.row >= app.messages_rect.y && mouse.row < app.messages_rect.y + app.messages_rect.height {
                                app.handle_scrollbar_click(mouse.row);
                                app.dragging_scrollbar = true;
                            }
                        }
                        MouseEventKind::Drag(_) if app.dragging_scrollbar => {
                            app.handle_scrollbar_click(mouse.row);
                        }
                        MouseEventKind::Up(_) => {
                            app.dragging_scrollbar = false;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Increment frame counter for animations
        app.frame_count = app.frame_count.wrapping_add(1);
    }

    // Disable mouse support on exit
    execute!(stdout(), DisableMouseCapture)?;
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

        let vertical = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(1),  // gap between messages and input
            Constraint::Length(input_height),
            Constraint::Length(1),  // gap between input and status
            Constraint::Length(1),  // status line
        ]);

        let [messages_area, _gap1, input_area, _gap2, status_area] = vertical.areas(area);

        self.messages_rect = messages_area;
        self.input_rect = input_area;

        // Calculate if we're at the bottom before drawing
        let (at_bottom, _) = self.calculate_scroll_info();
        self.was_at_bottom = at_bottom;

        self.draw_messages(frame);
        self.draw_input(frame);
        self.draw_status(frame, status_area);
    }

    fn draw_messages(&self, frame: &mut Frame) {
        let mut lines = Vec::new();
        let available_width = (self.messages_rect.width.saturating_sub(4)) as usize;

        for msg in &self.messages {
            if msg.role == "user" {
                // User message with black background (matches input box)
                let wrapped = wrap_text(&msg.text, available_width);
                for line_text in wrapped {
                    let padded = format!("  {}  ", line_text);
                    lines.push(Line::from(
                        vec![
                            Span::styled(
                                padded,
                                Style::default().bg(Color::Black),
                            )
                        ]
                    ));
                }
            } else {
                // Assistant message - wrapped text with markdown formatting
                let wrapped = wrap_text(&msg.text, available_width);
                for line_text in wrapped {
                    let spans = parse_markdown_line(&line_text);
                    lines.push(Line::from(spans));
                }
            }
            lines.push(Line::from(""));
        }

        // Add pending response if streaming
        if !self.pending_response.is_empty() {
            let wrapped = wrap_text(&self.pending_response, available_width);
            for line_text in wrapped {
                let spans = parse_markdown_line(&line_text);
                lines.push(Line::from(spans));
            }
        }

        let line_count = lines.len() as u16;
        let max_scroll = line_count.saturating_sub(self.messages_rect.height);

        // scroll_offset is absolute line number to start from, clamp to valid range
        let start_line = (self.scroll_offset as u16).min(max_scroll);

        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(start_line as usize)
            .take(self.messages_rect.height as usize)
            .collect();

        // Draw messages
        frame.render_widget(Paragraph::new(visible_lines), self.messages_rect);

        // Draw position indicator [current/total] only if not at the bottom
        let at_bottom = start_line >= max_scroll;
        if !at_bottom {
            let current_line = start_line.saturating_add(1);
            let position_text = format!("[{}/{}]", current_line, line_count);
            let pos_width = position_text.len() as u16;
            if pos_width < self.messages_rect.width {
                let pos_x = self.messages_rect.x + self.messages_rect.width.saturating_sub(pos_width + 1);
                let pos_area = Rect {
                    x: pos_x,
                    y: self.messages_rect.y,
                    width: pos_width,
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(position_text).style(Style::default().fg(Color::DarkGray)),
                    pos_area,
                );
            }
        }

        // Draw scrollbar on the right side
        if line_count > self.messages_rect.height {
            let scrollbar_height = (self.messages_rect.height as f64 * self.messages_rect.height as f64 / line_count as f64).max(1.0) as u16;
            let scrollable_height = self.messages_rect.height.saturating_sub(scrollbar_height);
            let scrollbar_pos = ((start_line as f64 / max_scroll.max(1) as f64).min(1.0) * scrollable_height as f64) as u16;

            let mut scrollbar_lines = Vec::new();
            for y_offset in 0..self.messages_rect.height {
                if y_offset >= scrollbar_pos && y_offset < scrollbar_pos + scrollbar_height {
                    scrollbar_lines.push(Line::from(Span::styled("█", Style::default().fg(Color::DarkGray))));
                } else {
                    scrollbar_lines.push(Line::from(Span::raw(" ")));
                }
            }

            let scrollbar_area = Rect {
                x: self.messages_rect.x + self.messages_rect.width.saturating_sub(1),
                y: self.messages_rect.y,
                width: 1,
                height: self.messages_rect.height,
            };
            frame.render_widget(Paragraph::new(scrollbar_lines), scrollbar_area);
        }
    }

    fn draw_input(&self, frame: &mut Frame) {
        let margin = Paragraph::new("").style(Style::default().bg(Color::Black));
        frame.render_widget(margin, self.input_rect);

        let inner = self.input_rect.inner(ratatui::layout::Margin {
            horizontal: 3,
            vertical: 1,
        });

        let input = Paragraph::new(self.input.clone())
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
        let mut byte_count = 0;

        for c in self.input.chars() {
            if byte_count >= self.cursor_pos {
                break;
            }
            if c == '\n' {
                y += 1;
                x = 0;
            } else {
                x += 1;
            }
            byte_count += c.len_utf8();
        }
        (x, y)
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let pwd = get_pwd_display();
        let git_branch = get_git_branch();

        let mut left_spans = vec![Span::styled(pwd, Style::default().fg(Color::DarkGray))];

        if let Some(branch) = git_branch {
            left_spans.push(Span::raw(" "));
            left_spans.push(Span::styled(
                format!("[{}]", branch),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Add mode indicator
        left_spans.push(Span::raw(" "));
        let mode_color = match self.mode {
            Mode::Build => Color::Cyan,
            Mode::Plan => Color::Green,
        };
        let mode_text = match self.mode {
            Mode::Build => "build",
            Mode::Plan => "plan",
        };
        left_spans.push(Span::styled(mode_text, Style::default().fg(mode_color)));

        // Add braille spinner only when loading
        if self.loading {
            left_spans.push(Span::raw(" "));
            let braille_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            // Slow down animation: each frame shows for ~3 iterations (48ms per frame)
            let braille = braille_frames[((self.frame_count / 3) as usize) % braille_frames.len()];
            left_spans.push(Span::styled(braille.to_string(), Style::default().fg(Color::DarkGray)));
        }

        // Calculate token usage (using actual tracked tokens)
        let tokens_used = self.total_input_tokens + self.total_output_tokens;
        let percentage = if self.context_window > 0 {
            (tokens_used * 100) / self.context_window
        } else {
            0
        };
        let right_text = format!(
            "{}/{} ({}%)",
            format_tokens(tokens_used),
            format_tokens(self.context_window),
            percentage
        );

        let status_style = Style::default().fg(Color::DarkGray);

        // Create line with left and right with spacing
        let total_width = area.width as usize;
        let left_width: usize = left_spans.iter().map(|s| s.content.len()).sum();
        let right_width = right_text.len();

        if left_width + right_width + 2 > total_width {
            // If they don't fit, just show left
            frame.render_widget(Paragraph::new(Line::from(left_spans)), area);
        } else {
            let gap = total_width - left_width - right_width;
            left_spans.push(Span::raw(" ".repeat(gap)));
            left_spans.push(Span::styled(right_text, status_style));
            frame.render_widget(Paragraph::new(Line::from(left_spans)), area);
        }
    }
}
