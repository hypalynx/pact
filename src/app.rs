use crate::db::Db;
use crate::llm::{LlmEvent, Message};
use crate::text::wrap_text;
use indexmap::IndexMap;
use ratatui::layout::Rect;
use std::fs;
use std::io;
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelState {
    None,
    ControlPanel,
    Debug,
}

pub struct App {
    pub db: Option<Db>,
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
    pub pending_thinking: String,
    pub debug: bool,
    pub scroll_offset: usize,
    pub user_scrolled: bool,
    pub was_at_bottom: bool,
    pub dragging_scrollbar: bool,
    pub mode_name: String,
    pub available_modes: Vec<String>,
    pub modes_config: IndexMap<String, crate::config::Mode>,
    pub context_window: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub frame_count: u32,
    pub last_server_check: u32,
    pub api_endpoint: String,
    pub max_tokens: usize,
    pub temperature: Option<f32>,
    pub mode_color: Option<String>,
    pub model_name: String,
    pub selection_start: Option<(u16, u16)>,
    pub selection_end: Option<(u16, u16)>,
    pub last_copy_frame: u32,
    pub error_message: Option<String>,
    pub error_frame: u32,
    pub panel_state: PanelState,
    pub debug_scroll: usize,
    pub debug_filter_errors: bool,
    pub debug_logs: Vec<crate::db::ApiLogEntry>,
    pub debug_selected_row: usize,
    pub debug_expanded_row: Option<usize>,
    pub debug_expand_scroll: usize,
}

impl App {
    pub fn new(
        debug: bool,
        api_endpoint: String,
        max_tokens: usize,
        temperature: Option<f32>,
        mode_name: String,
        modes_config: IndexMap<String, crate::config::Mode>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let available_modes: Vec<String> = modes_config.keys().cloned().collect();
        let mode_color = modes_config.get(&mode_name).and_then(|m| m.color.clone());

        // Initialize database (graceful failure)
        let (db, db_error) = match Db::open().and_then(|db| db.init_schema().map(|_| db)) {
            Ok(db) => (Some(db), None),
            Err(e) => (None, Some(format!("Database error: {}", e))),
        };

        Self {
            db,
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
            pending_thinking: String::new(),
            debug,
            scroll_offset: 0,
            user_scrolled: false,
            was_at_bottom: true,
            dragging_scrollbar: false,
            mode_name,
            available_modes,
            modes_config,
            context_window: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            frame_count: 0,
            last_server_check: 0,
            api_endpoint,
            max_tokens,
            temperature,
            mode_color,
            model_name: String::new(),
            selection_start: None,
            selection_end: None,
            last_copy_frame: u32::MAX, // Initialize to max so it's never "recent" on startup
            error_message: db_error,
            error_frame: 0,
            panel_state: PanelState::None,
            debug_scroll: 0,
            debug_filter_errors: false,
            debug_logs: Vec::new(),
            debug_selected_row: 0,
            debug_expanded_row: None,
            debug_expand_scroll: 0,
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
            .filter(|m| m.role == "user" && !m.is_tool_result)
            .map(|m| Message {
                role: m.role.clone(),
                text: m.text.clone(),
                is_tool_result: false,
                thinking: None,
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
        let msg = Message {
            role: "user".to_string(),
            text: text.clone(),
            is_tool_result: false,
            thinking: None,
        };
        self.messages.push(msg.clone());
        self.history_index = None;
        // Save user message to database if available
        if let Some(db) = &self.db {
            let _ = db.save_message(&msg);
        }
        self.input = String::new();
        self.cursor_pos = 0;
        self.send_to_llm();
    }

    pub fn send_to_llm(&mut self) {
        self.loading = true;
        self.pending_response.clear();
        self.user_scrolled = false;

        let messages = self.messages.clone();
        let tx = self.tx.clone();
        let debug = self.debug;
        let endpoint = self.api_endpoint.clone();
        let max_tokens = self.max_tokens;
        let temperature = self.temperature;
        let system_prompt = self
            .modes_config
            .get(&self.mode_name)
            .and_then(|m| m.system_prompt.clone());

        std::thread::spawn(move || {
            crate::llm::call_llm(
                messages,
                tx,
                debug,
                &endpoint,
                max_tokens,
                temperature,
                system_prompt,
            );
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

        let scrollbar_height = (self.messages_rect.height as f64 * self.messages_rect.height as f64
            / total_lines as f64)
            .max(1.0) as u16;
        let scrollable_height = self.messages_rect.height.saturating_sub(scrollbar_height);
        let scrollable_lines = total_lines.saturating_sub(self.messages_rect.height as usize);

        let click_offset = mouse_y
            .saturating_sub(self.messages_rect.y)
            .min(scrollable_height);

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
            let byte_pos = self
                .input
                .char_indices()
                .rfind(|(i, _)| *i < self.cursor_pos)
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

    pub fn move_cursor_forward(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next_char = self.input[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            self.cursor_pos += next_char;
        }
    }

    pub fn move_cursor_backward(&mut self) {
        if self.cursor_pos > 0 {
            let byte_pos = self
                .input
                .char_indices()
                .rfind(|(i, _)| *i < self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor_pos = byte_pos;
        }
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

        let byte_pos = self
            .input
            .chars()
            .take(pos)
            .map(|c| c.len_utf8())
            .sum::<usize>();
        let byte_end = self
            .input
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

    pub fn cycle_mode(&mut self) {
        if self.available_modes.is_empty() {
            return;
        }

        let current_idx = self
            .available_modes
            .iter()
            .position(|m| m == &self.mode_name)
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % self.available_modes.len();
        self.mode_name = self.available_modes[next_idx].clone();

        // Update temperature and color from the new mode config
        if let Some(mode_config) = self.modes_config.get(&self.mode_name) {
            self.temperature = mode_config.temperature;
            self.mode_color = mode_config.color.clone();
        }
    }

    pub fn check_server_info(&mut self) {
        // Refresh server info every ~3 seconds (roughly 188 frames at 16ms per frame)
        const CHECK_INTERVAL: u32 = 188;

        if self.frame_count.saturating_sub(self.last_server_check) >= CHECK_INTERVAL {
            let server_info = crate::utils::fetch_server_info(&self.api_endpoint);
            self.model_name = server_info.model_name;
            self.context_window = server_info.context_window;
            self.last_server_check = self.frame_count;
        }
    }

    pub fn start_selection(&mut self, x: u16, y: u16) {
        self.selection_start = Some((x, y));
        self.selection_end = None;
    }

    pub fn extend_selection(&mut self, x: u16, y: u16) {
        self.selection_end = Some((x, y));
    }

    pub fn finish_selection(&mut self) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            if start == end {
                // Deselect if same position
                self.selection_start = None;
                self.selection_end = None;
                return;
            }

            // Extract text and copy to clipboard
            if let Some(text) = self.extract_selected_text() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if clipboard.set_text(text).is_ok() {
                        self.last_copy_frame = self.frame_count;
                    }
                    // Silently fail on clipboard errors - don't corrupt TUI with stderr
                }
            }
            self.selection_start = None;
            self.selection_end = None;
        }
    }

    fn extract_selected_text(&self) -> Option<String> {
        let (start, _end) = match (self.selection_start, self.selection_end) {
            (Some(s), Some(e)) => {
                if s <= e {
                    (s, e)
                } else {
                    (e, s)
                }
            }
            _ => return None,
        };

        // Simple extraction: collect all message text
        let mut all_text = String::new();
        for msg in &self.messages {
            all_text.push_str(&msg.text);
            all_text.push('\n');
        }
        if !self.pending_response.is_empty() {
            all_text.push_str(&self.pending_response);
        }

        // Rough approximation: extract by line
        // This is a simplified version - in reality we'd need to map screen coords to text
        if all_text.len() > (start.0 as usize) {
            Some(all_text)
        } else {
            None
        }
    }

    pub fn is_copying(&self) -> bool {
        // Show notification for ~2 seconds (roughly 125 frames at 16ms)
        // Don't show if we've never copied (last_copy_frame is u32::MAX)
        if self.last_copy_frame == u32::MAX {
            return false;
        }
        self.frame_count.saturating_sub(self.last_copy_frame) < 125
    }

    pub fn refresh_debug_logs(&mut self) {
        if let Some(db) = &self.db {
            self.debug_logs = db.recent_api_logs(100).unwrap_or_default();
        }
    }

    pub fn load_messages_from_db(&mut self) {
        if let Some(db) = &self.db {
            if let Ok(msgs) = db.load_messages() {
                self.history = msgs
                    .iter()
                    .filter(|m| m.role == "user" && !m.is_tool_result)
                    .map(|m| m.text.clone())
                    .collect();
                self.messages = msgs;
            }
        }
    }

    pub fn debug_filtered_logs(&self) -> Vec<&crate::db::ApiLogEntry> {
        if self.debug_filter_errors {
            self.debug_logs
                .iter()
                .filter(|log| log.error_message.is_some())
                .collect()
        } else {
            self.debug_logs.iter().collect()
        }
    }

    pub fn toggle_debug_row_expand(&mut self, row_idx: usize) {
        if self.debug_expanded_row == Some(row_idx) {
            self.debug_expanded_row = None;
            self.debug_expand_scroll = 0;
        } else {
            self.debug_expanded_row = Some(row_idx);
            self.debug_expand_scroll = 0;
        }
    }
}
