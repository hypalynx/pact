use crate::llm::{Message, LlmEvent};
use crate::text::wrap_text;
use std::fs;
use std::io;
use std::sync::mpsc;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Build,
    Plan,
}

pub struct App {
    pub messages: Vec<Message>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub input: String,
    pub cursor_pos: usize,
    pub input_rect: Rect,
    pub messages_rect: Rect,
    pub rx: mpsc::Receiver<LlmEvent>,
    pub tx: mpsc::Sender<LlmEvent>,
    pub loading: bool,
    pub pending_response: String,
    pub debug: bool,
    pub scroll_offset: usize,
    pub user_scrolled: bool,
    pub was_at_bottom: bool,
    pub dragging_scrollbar: bool,
    pub mode: Mode,
    pub context_window: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub frame_count: u32,
    pub api_endpoint: String,
    pub max_tokens: usize,
}

impl App {
    pub fn new(debug: bool, api_endpoint: String, max_tokens: usize) -> Self {
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
            api_endpoint,
            max_tokens,
        }
    }

    pub fn load_history() -> io::Result<Vec<String>> {
        let path = crate::utils::messages_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        let messages: Vec<Message> = serde_json::from_str(&content).unwrap_or_default();
        Ok(messages.into_iter().map(|m| m.text).collect())
    }

    pub fn save_history(&self) -> io::Result<()> {
        let path = crate::utils::messages_path();
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

    pub fn submit_message(&mut self) {
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
        self.user_scrolled = false;

        let messages = self.messages.clone();
        let tx = self.tx.clone();
        let debug = self.debug;
        let endpoint = self.api_endpoint.clone();
        let max_tokens = self.max_tokens;

        std::thread::spawn(move || {
            crate::llm::call_llm(messages, tx, debug, &endpoint, max_tokens);
        });
    }

    pub fn history_up(&mut self) {
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

    pub fn history_down(&mut self) {
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

    pub fn calculate_total_lines(&self) -> usize {
        let mut total_lines = 0;
        let available_width = (self.messages_rect.width.saturating_sub(4)) as usize;

        for msg in &self.messages {
            let wrapped = wrap_text(&msg.text, available_width);
            total_lines += wrapped.len() + 1;
        }

        if !self.pending_response.is_empty() {
            let wrapped = wrap_text(&self.pending_response, available_width);
            total_lines += wrapped.len();
        }

        total_lines
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
        self.user_scrolled = true;
    }

    pub fn scroll_down(&mut self) {
        let total_lines = self.calculate_total_lines();
        let max_scroll = total_lines.saturating_sub(self.messages_rect.height as usize);
        self.scroll_offset = (self.scroll_offset + 3).min(max_scroll);
        if self.scroll_offset >= max_scroll {
            self.user_scrolled = false;
        }
    }

    pub fn calculate_scroll_info(&self) -> (bool, usize) {
        let total_lines = self.calculate_total_lines();
        let max_scroll = total_lines.saturating_sub(self.messages_rect.height as usize);
        let at_bottom = self.scroll_offset >= max_scroll;

        (at_bottom, total_lines)
    }

    pub fn handle_scrollbar_click(&mut self, mouse_y: u16) {
        let (_at_bottom, total_lines) = self.calculate_scroll_info();
        if total_lines as u16 <= self.messages_rect.height {
            return;
        }

        let scrollbar_height = (self.messages_rect.height as f64 * self.messages_rect.height as f64 / total_lines as f64).max(1.0) as u16;
        let scrollable_height = self.messages_rect.height.saturating_sub(scrollbar_height);
        let scrollable_lines = total_lines.saturating_sub(self.messages_rect.height as usize);

        let click_offset = mouse_y.saturating_sub(self.messages_rect.y).min(scrollable_height);

        if scrollable_height > 0 {
            let proportion = click_offset as f64 / scrollable_height as f64;
            self.scroll_offset = (proportion * scrollable_lines as f64) as usize;
        } else {
            self.scroll_offset = 0;
        }

        self.user_scrolled = true;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_char(&mut self) {
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

    pub fn move_cursor_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_to_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    pub fn kill_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let input_chars: Vec<char> = self.input.chars().collect();
        let mut pos = self.cursor_pos.saturating_sub(1);

        while pos > 0 && input_chars[pos].is_whitespace() {
            pos = pos.saturating_sub(1);
        }

        while pos > 0 && !input_chars[pos - 1].is_whitespace() {
            pos = pos.saturating_sub(1);
        }

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

    pub fn kill_line(&mut self) {
        let line_start = self.input[..self.cursor_pos]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        self.input.drain(line_start..self.cursor_pos);
        self.cursor_pos = line_start;
    }
}
