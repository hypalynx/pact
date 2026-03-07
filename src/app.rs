use crate::db::Db;
use crate::llm::{LlmEvent, Message};
use indexmap::IndexMap;
use ratatui::layout::Rect;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelState {
    None,
    ControlPanel,
    Debug,
}

pub struct PendingBashConfirm {
    pub tool_id: String,
    pub command: String,
    pub reason: String,
    pub args: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
    pub blocks: Vec<u32>,
    pub blocked_by: Vec<u32>,
}

pub struct FilePicker {
    pub query: String,
    pub at_start: usize, // byte offset in `input` where @ was typed
    pub all_entries: Vec<String>,
    pub filtered: Vec<String>,
    pub selected: usize,
}

#[derive(Clone, PartialEq)]
pub enum SlashCommand {
    Model,
    Connect,
    New,
    Clear,
}

pub struct SlashCommandPicker {
    pub command: SlashCommand,
    pub query: String,
    pub slash_start: usize, // byte offset in `input` where / was typed
    pub all_entries: Vec<String>,
    pub filtered: Vec<String>,
    pub selected: usize,
}

fn scan_project_files() -> Vec<String> {
    let mut files = Vec::new();
    walk_dir(std::path::Path::new("."), &mut files, 0);
    files
}

fn walk_dir(dir: &std::path::Path, out: &mut Vec<String>, depth: usize) {
    if depth > 6 {
        return;
    }
    let skip = [
        ".git",
        "node_modules",
        "target",
        "dist",
        ".venv",
        "__pycache__",
        ".next",
    ];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if skip.contains(&&*name_str) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out, depth + 1);
        } else {
            let rel = path.to_string_lossy().trim_start_matches("./").to_string();
            out.push(rel);
        }
    }
}

pub(crate) const DEFAULT_MAX_TOKENS: usize = 8192;
const SCROLL_STEP: usize = 8;

pub struct App {
    pub db: Option<Db>,
    pub messages: Vec<Message>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub input: String,
    pub cursor_pos: usize,
    pub unsent_draft: String, // Preserves unsent input when navigating history
    pub unsent_cursor_pos: usize, // Preserves cursor pos for unsent input
    pub input_rect: Rect,
    pub messages_rect: Rect,
    pub rx: mpsc::Receiver<LlmEvent>,
    pub tx: mpsc::Sender<LlmEvent>,
    pub active_llm_calls: usize,
    pub pending_response: String,
    pub pending_thinking: String,
    pub debug: bool,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub rendered_line_count: usize,
    pub dragging_scrollbar: bool,
    pub default_mode_name: String,
    pub mode_name: String,
    pub available_modes: Vec<String>,
    pub modes_config: IndexMap<String, crate::config::Mode>,
    pub context_window: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub last_output_tokens: usize,
    pub frame_count: u32,
    pub last_server_check: u32,
    pub api_endpoint: String,
    pub temperature: Option<f32>,
    pub mode_color: Option<String>,
    pub model_name: String,
    pub selection_start: Option<(u16, u16)>,
    pub selection_end: Option<(u16, u16)>,
    pub last_copy_frame: u32,
    pub input_scroll_offset: usize,
    pub status_message: Option<(String, StatusLevel)>,
    pub last_status_frame: u32,
    pub exit_confirm_frame: u32,
    pub cancel_confirm_frame: u32,
    pub last_cancel_frame: u32,
    pub panel_state: PanelState,
    pub cancel_flag: Arc<AtomicBool>,
    pub debug_scroll: usize,
    pub debug_filter_errors: bool,
    pub debug_logs: Vec<crate::db::ApiLogEntry>,
    pub debug_selected_row: usize,
    pub debug_expanded_row: Option<usize>,
    pub debug_expand_scroll: usize,
    pub debug_expand_scroll_x: usize,

    pub all_line_texts: Vec<String>,
    pub agents_context: Option<String>,
    pub file_picker: Option<FilePicker>,
    pub file_picker_map: IndexMap<String, String>, // Map filename -> full relative path

    // Provider management
    pub providers: Vec<crate::db::Provider>,
    pub active_provider: Option<crate::db::Provider>,

    // Slash command picker
    pub slash_picker: Option<SlashCommandPicker>,

    // API key input mode
    pub api_key_input: Option<String>, // When Some, we're in API key input mode

    // Call tracking
    pub call_counter: u64,
    pub active_call_id: Option<u64>,
    pub pending_tool_count: usize,

    // Pending bash confirmation
    pub pending_bash_confirm: Option<PendingBashConfirm>,

    // Session management
    pub session_id: String,
    pub working_directory: String,

    // Retry handling for invalid tool calls
    pub needs_retry: bool,

    // Queue user messages submitted while LLM is generating
    pub pending_user_messages: Vec<Message>,

    // Task management
    pub tasks: Vec<Task>,
    pub task_id_counter: u32,

    // Ctrl+X prefix key state
    pub pending_ctrl_x: bool,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        debug: bool,
        temperature: Option<f32>,
        mode_name: String,
        modes_config: IndexMap<String, crate::config::Mode>,
        agents_context: Option<String>,
        session_id: String,
        working_directory: String,
        messages: Vec<Message>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let available_modes: Vec<String> = modes_config.keys().cloned().collect();
        let mode_color = modes_config.get(&mode_name).and_then(|m| m.color.clone());

        // Initialize database (graceful failure)
        // Note: migrations are run earlier in main.rs, so we just need to open the DB
        let (db, db_error) = match Db::open() {
            Ok(db) => (Some(db), None),
            Err(e) => (None, Some(format!("Database error: {}", e))),
        };

        Self {
            db,
            messages,
            history: Vec::new(),
            history_index: None,
            input: String::new(),
            cursor_pos: 0,
            unsent_draft: String::new(),
            unsent_cursor_pos: 0,
            input_rect: Rect::default(),
            messages_rect: Rect::default(),
            rx,
            tx,
            active_llm_calls: 0,
            pending_response: String::new(),
            pending_thinking: String::new(),
            debug,
            scroll_offset: 0,
            auto_scroll: true,
            rendered_line_count: 0,
            dragging_scrollbar: false,
            default_mode_name: mode_name.clone(),
            mode_name,
            available_modes,
            modes_config,
            context_window: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            last_output_tokens: 0,
            frame_count: 0,
            last_server_check: 0,
            api_endpoint: String::new(),
            temperature,
            mode_color,
            model_name: String::new(),
            selection_start: None,
            selection_end: None,
            last_copy_frame: u32::MAX, // Initialize to max so it's never "recent" on startup
            input_scroll_offset: 0,
            status_message: db_error.map(|e| (e, StatusLevel::Error)),
            last_status_frame: u32::MAX,
            exit_confirm_frame: u32::MAX,
            cancel_confirm_frame: u32::MAX,
            last_cancel_frame: u32::MAX,
            panel_state: PanelState::None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            debug_scroll: 0,
            debug_filter_errors: false,
            debug_logs: Vec::new(),
            debug_selected_row: 0,
            debug_expanded_row: None,
            debug_expand_scroll: 0,
            debug_expand_scroll_x: 0,

            all_line_texts: Vec::new(),
            agents_context,
            file_picker: None,
            file_picker_map: IndexMap::new(),
            providers: Vec::new(),
            active_provider: None,
            slash_picker: None,
            api_key_input: None,
            call_counter: 0,
            active_call_id: None,
            pending_tool_count: 0,
            pending_bash_confirm: None,
            session_id,
            working_directory,
            needs_retry: false,
            pending_user_messages: Vec::new(),
            tasks: Vec::new(),
            task_id_counter: 1,
            pending_ctrl_x: false,
        }
    }

    pub fn load_providers_from_db(&mut self) {
        if let Some(db) = &self.db {
            if let Ok(providers) = db.get_providers() {
                self.providers = providers;
            }
            if let Ok(Some(active)) = db.get_active_provider() {
                self.active_provider = Some(active.clone());
                self.api_endpoint = active.endpoint.clone();
            }
        }
    }

    pub fn cycle_provider(&mut self) {
        if self.providers.is_empty() {
            return;
        }

        // Find current provider index
        let current_idx = self
            .active_provider
            .as_ref()
            .and_then(|ap| self.providers.iter().position(|p| p.name == ap.name))
            .unwrap_or(0);

        // Get next provider
        let next_idx = (current_idx + 1) % self.providers.len();
        let next_provider = self.providers[next_idx].clone();

        // Update active provider in DB and app
        if let Some(db) = &self.db {
            let _ = db.set_active_provider(&next_provider.name);
        }

        self.active_provider = Some(next_provider.clone());
        self.api_endpoint = next_provider.endpoint.clone();
        self.set_status(
            format!("Switched to provider: {}", next_provider.name),
            StatusLevel::Info,
        );
    }

    pub fn submit_message(&mut self) {
        if self.input.trim().is_empty() {
            return;
        }
        let mut text = std::mem::take(&mut self.input);
        // Resolve file picker references (@filename -> ./relative/path)
        text = self.resolve_file_picker_refs(&text);
        self.history.push(text.clone());
        let msg = Message {
            role: "user".to_string(),
            text: text.clone(),
            is_tool_result: false,
            thinking: None,
            tool_result_content: None,
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        };

        // If LLM is actively responding, queue the message to send later
        if self.active_llm_calls > 0 {
            self.pending_user_messages.push(msg);
        } else {
            // LLM is idle, add message to history and send immediately
            self.messages.push(msg.clone());
            // Save user message to database if available
            if let Some(db) = &self.db {
                let is_first_user_message = self
                    .messages
                    .iter()
                    .filter(|m| m.role == "user" && !m.is_tool_result)
                    .count()
                    == 1;
                if is_first_user_message {
                    let preview = text.chars().take(60).collect::<String>();
                    let _ = db.update_session_first_prompt(&self.session_id, &preview);
                }
                let _ =
                    db.save_message_with_session(&msg, &self.session_id, &self.working_directory);
            }
            self.send_to_llm();
        }

        self.history_index = None;
        self.unsent_draft.clear();
        self.unsent_cursor_pos = 0;
        self.input = String::new();
        self.cursor_pos = 0;
        self.auto_scroll = true;
    }

    fn resolve_file_picker_refs(&self, text: &str) -> String {
        let mut result = text.to_string();
        // Replace @filename patterns with actual paths from the mapping
        for (filename, path) in &self.file_picker_map {
            let pattern = format!("@{}", filename);
            if result.contains(&pattern) {
                result = result.replace(&pattern, path);
            }
        }
        result
    }

    pub fn has_pending_messages(&self) -> bool {
        !self.pending_user_messages.is_empty()
    }

    pub fn send_to_llm(&mut self) {
        // Safety net: reject if call already active
        if self.active_llm_calls > 0 {
            return;
        }

        self.active_llm_calls += 1;
        self.pending_response.clear();
        self.pending_thinking.clear();

        // Generate call ID
        let call_id = self.call_counter;
        self.call_counter += 1;
        self.active_call_id = Some(call_id);

        // Reset cancellation flag for new call
        self.cancel_flag.store(false, Ordering::SeqCst);
        let cancel_flag = Arc::clone(&self.cancel_flag);

        let messages = self.messages.clone();
        let tx = self.tx.clone();
        let debug = self.debug;
        let endpoint = self.api_endpoint.clone();
        let max_tokens = DEFAULT_MAX_TOKENS;
        let temperature = self.temperature;

        // Get provider info (API key and model) from active provider if available
        let api_key = self
            .active_provider
            .as_ref()
            .and_then(|p| p.api_key.clone());
        let provider_name = self.active_provider.as_ref().map(|p| p.name.clone());

        // Get model ID from active provider's default_model, or use "local" as fallback
        let model_id = self
            .active_provider
            .as_ref()
            .and_then(|p| p.default_model.clone())
            .unwrap_or_else(|| "local".to_string());

        // Combine mode prompt with agents_context
        let mode_prompt = self
            .modes_config
            .get(&self.mode_name)
            .and_then(|m| m.system_prompt.clone());
        let system_prompt = match (mode_prompt, &self.agents_context) {
            (Some(mode), Some(agents)) => Some(format!("{}\n\n{}", mode, agents)),
            (Some(mode), None) => Some(mode),
            (None, Some(agents)) => Some(agents.clone()),
            (None, None) => None,
        };

        std::thread::spawn(move || {
            crate::llm::call_llm(
                messages,
                tx,
                debug,
                &endpoint,
                api_key.as_deref(),
                max_tokens,
                temperature,
                system_prompt,
                model_id,
                provider_name,
                cancel_flag,
                call_id,
            );
        });
    }

    /// Get the available width for input text wrapping
    fn get_input_width(&self) -> usize {
        const INPUT_HORIZONTAL_MARGIN: u16 = 3;
        (self
            .input_rect
            .width
            .saturating_sub(INPUT_HORIZONTAL_MARGIN * 2)) as usize
    }

    /// Get visual cursor position (col, row) and total wrapped lines
    fn get_cursor_visual_position(&self) -> (usize, usize, usize) {
        let width = self.get_input_width();
        let (col, row) = crate::text::cursor_position(&self.input, self.cursor_pos, width);
        let total_lines = crate::text::wrap_text(&self.input, width).len().max(1);
        (col, row, total_lines)
    }

    /// Move cursor up within the input (without changing history)
    fn move_cursor_up_in_input(&mut self) {
        let (col, row, _total_lines) = self.get_cursor_visual_position();

        // Only move if not on first line
        if row == 0 {
            return;
        }

        self.move_cursor_to_visual_row(row - 1, col);
    }

    /// Move cursor down within the input (without changing history)
    fn move_cursor_down_in_input(&mut self) {
        let (col, row, total_lines) = self.get_cursor_visual_position();

        // Only move if not on last line
        if row >= total_lines - 1 {
            return;
        }

        self.move_cursor_to_visual_row(row + 1, col);
    }

    /// Move cursor to a specific visual row, attempting to maintain column position
    fn move_cursor_to_visual_row(&mut self, target_row: usize, target_col: usize) {
        let width = self.get_input_width();

        // Find the first byte position in the target row
        // Then move col characters into that row
        let mut found_row_start = None;

        for byte_offset in 0..=self.input.len() {
            let (_, visual_row) = crate::text::cursor_position(&self.input, byte_offset, width);

            if visual_row == target_row {
                found_row_start = Some(byte_offset);
                break;
            }
        }

        if let Some(row_start) = found_row_start {
            // Found the start of target row, now move target_col characters in
            for byte_offset in row_start..=self.input.len() {
                let (col, row) = crate::text::cursor_position(&self.input, byte_offset, width);

                // Stop if we've gone past the target row
                if row != target_row {
                    // Use the previous valid position
                    self.cursor_pos = byte_offset.saturating_sub(1);
                    return;
                }

                // Stop if we've reached or passed the target column
                if col >= target_col {
                    self.cursor_pos = byte_offset;
                    return;
                }
            }

            // If we've exhausted the input, place cursor at end of target row
            self.cursor_pos = self.input.len();
        }
    }

    /// Handle up arrow: move cursor up in input if possible, otherwise navigate history
    pub fn handle_up_key(&mut self) {
        let (_col, row, _total_lines) = self.get_cursor_visual_position();

        // If not on first line, move up within input
        if row > 0 {
            self.move_cursor_up_in_input();
        } else {
            // On first line, navigate history
            self.history_up();
        }
    }

    /// Handle down arrow: move cursor down in input if possible, otherwise navigate history
    pub fn handle_down_key(&mut self) {
        let (_col, row, total_lines) = self.get_cursor_visual_position();

        // If not on last line, move down within input
        if row < total_lines - 1 {
            self.move_cursor_down_in_input();
        } else {
            // On last line, navigate history
            self.history_down();
        }
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        // Save current input as draft if this is the first navigation
        if self.history_index.is_none() {
            self.unsent_draft = self.input.clone();
            self.unsent_cursor_pos = self.cursor_pos;
        }

        let new_index = match self.history_index {
            None => self.history.len() - 1,
            Some(0) => return,
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
                // Restore the unsent draft
                self.input = self.unsent_draft.clone();
                self.cursor_pos = self.unsent_cursor_pos;
                return;
            }
            Some(i) => i + 1,
        };
        self.history_index = Some(new_index);
        self.input = self.history[new_index].clone();
        self.cursor_pos = self.input.len();
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(SCROLL_STEP);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self
            .rendered_line_count
            .saturating_sub(self.messages_rect.height as usize);
        self.scroll_offset = (self.scroll_offset + SCROLL_STEP).min(max_scroll);
        if self.scroll_offset >= max_scroll {
            self.auto_scroll = true;
        }
    }

    pub fn handle_scrollbar_click(&mut self, mouse_y: u16) {
        let total_lines = self.rendered_line_count;
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

        self.auto_scroll = false;
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
            self.last_server_check = self.frame_count;

            let endpoint = self.api_endpoint.clone();
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let server_info = crate::utils::fetch_server_info(&endpoint);
                let _ = tx.send(LlmEvent::ServerInfo {
                    model_name: server_info.model_name,
                    context_window: server_info.context_window,
                    call_id: 0, // Background server check, not part of a call
                });
            });
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
            if let Some(text) = self.extract_selected_text()
                && let Ok(mut clipboard) = arboard::Clipboard::new()
                && clipboard.set_text(text).is_ok()
            {
                self.last_copy_frame = self.frame_count;
                // Keep clipboard object alive for a bit longer to ensure clipboard managers have time to read
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            // Silently fail on clipboard errors - don't corrupt TUI with stderr
            self.selection_start = None;
            self.selection_end = None;
        }
    }

    fn extract_selected_text(&self) -> Option<String> {
        let (start, end) = match (self.selection_start, self.selection_end) {
            (Some(s), Some(e)) => {
                if s.1 < e.1 || (s.1 == e.1 && s.0 <= e.0) {
                    (s, e)
                } else {
                    (e, s)
                }
            }
            _ => return None,
        };

        if self.all_line_texts.is_empty() {
            return None;
        }

        // Compute scroll offset → full array index mapping
        let line_count = self.all_line_texts.len() as u16;
        let max_scroll = line_count.saturating_sub(self.messages_rect.height);
        let start_line = (self.scroll_offset as u16).min(max_scroll) as usize;

        // Screen row → full array index
        let vis_start = start.1.saturating_sub(self.messages_rect.y) as usize;
        let vis_end = end.1.saturating_sub(self.messages_rect.y) as usize;
        let idx_start = (vis_start + start_line).min(self.all_line_texts.len().saturating_sub(1));
        let idx_end = (vis_end + start_line).min(self.all_line_texts.len().saturating_sub(1));

        // Column → text char offset (subtract left padding of 2)
        let text_x = self.messages_rect.x;
        let pad = 2usize;
        let col_start = (start.0 as usize).saturating_sub(text_x as usize + pad);
        let col_end = (end.0 as usize).saturating_sub(text_x as usize + pad);

        let mut parts = Vec::new();
        for idx in idx_start..=idx_end {
            let line = &self.all_line_texts[idx];
            let char_count = line.chars().count();

            let (cs, ce) = if idx == idx_start && idx == idx_end {
                (col_start.min(char_count), col_end.min(char_count))
            } else if idx == idx_start {
                (col_start.min(char_count), char_count)
            } else if idx == idx_end {
                (0, col_end.min(char_count))
            } else {
                (0, char_count)
            };

            // Slice by char index (safe for multibyte chars)
            let sliced: String = line.chars().skip(cs).take(ce.saturating_sub(cs)).collect();
            parts.push(sliced);
        }

        let selected = parts.join("\n");
        if selected.trim().is_empty() {
            None
        } else {
            Some(selected)
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

    pub fn set_status(&mut self, msg: impl Into<String>, level: StatusLevel) {
        self.status_message = Some((msg.into(), level));
        self.last_status_frame = self.frame_count;
    }

    pub fn has_status(&self) -> bool {
        // Show status for ~2 seconds (roughly 125 frames at 16ms)
        // Don't show if we've never set status (last_status_frame is u32::MAX)
        if self.last_status_frame == u32::MAX {
            return false;
        }
        self.frame_count.saturating_sub(self.last_status_frame) < 125
    }

    pub fn get_status_level(&self) -> Option<StatusLevel> {
        if self.has_status() {
            self.status_message.as_ref().map(|(_, level)| *level)
        } else {
            None
        }
    }

    #[deprecated(note = "Use set_status(msg, StatusLevel::Error) instead")]
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.set_status(msg, StatusLevel::Error);
    }

    #[deprecated(note = "Use has_status() instead")]
    pub fn has_error(&self) -> bool {
        self.has_status()
    }

    pub fn is_exit_confirming(&self) -> bool {
        // Show confirmation for ~2 seconds (roughly 125 frames at 16ms)
        // Don't show if we've never confirmed (exit_confirm_frame is u32::MAX)
        if self.exit_confirm_frame == u32::MAX {
            return false;
        }
        self.frame_count.saturating_sub(self.exit_confirm_frame) < 125
    }

    pub fn set_exit_confirmation(&mut self) {
        self.exit_confirm_frame = self.frame_count;
    }

    pub fn reset_exit_confirmation(&mut self) {
        self.exit_confirm_frame = u32::MAX;
    }

    pub fn is_cancel_confirming(&self) -> bool {
        // Show confirmation for ~2 seconds (roughly 125 frames at 16ms)
        // Don't show if we've never confirmed (cancel_confirm_frame is u32::MAX)
        if self.cancel_confirm_frame == u32::MAX {
            return false;
        }
        self.frame_count.saturating_sub(self.cancel_confirm_frame) < 125
    }

    pub fn set_cancel_confirmation(&mut self) {
        self.cancel_confirm_frame = self.frame_count;
    }

    pub fn reset_cancel_confirmation(&mut self) {
        self.cancel_confirm_frame = u32::MAX;
        // Don't reset last_cancel_frame here - that tracks when we actually cancelled
    }

    pub fn cancel_current_call(&mut self) {
        // Set cancellation flag to stop the LLM thread
        // Decrement counter immediately to stop loading animation
        if self.active_llm_calls > 0 {
            self.cancel_flag.store(true, Ordering::SeqCst);
            self.active_llm_calls = self.active_llm_calls.saturating_sub(1);
            // Track when we cancelled to show "Call cancelled" status
            self.last_cancel_frame = self.frame_count;
            // Push an assistant message about the cancellation
            // Using assistant role with special marker to indicate it was cancelled
            let cancel_msg = Message {
                role: "assistant".to_string(),
                text: "_Call cancelled by user_".to_string(),
                is_tool_result: false,
                thinking: None,
                tool_result_content: None,
                tool_call_id: None,
                tool_name: None,
                tool_calls: None,
            };
            self.messages.push(cancel_msg.clone());
            if let Some(db) = &self.db {
                let _ = db.save_message_with_session(
                    &cancel_msg,
                    &self.session_id,
                    &self.working_directory,
                );
            }
        }
    }

    pub fn was_just_cancelled(&self) -> bool {
        // Show "Call cancelled" for ~2 seconds
        if self.last_cancel_frame == u32::MAX {
            return false;
        }
        self.frame_count.saturating_sub(self.last_cancel_frame) < 125
    }

    pub fn refresh_debug_logs(&mut self) {
        if let Some(db) = &self.db {
            self.debug_logs = db.recent_api_logs(100).unwrap_or_default();
        }
    }

    pub fn load_history_from_db(&mut self) {
        if let Some(db) = &self.db
            && let Ok(msgs) = db.load_messages()
        {
            // Only load user message texts for history (for up/down arrow navigation)
            // Don't load messages into the chat display
            self.history = msgs
                .iter()
                .filter(|m| m.role == "user" && !m.is_tool_result)
                .map(|m| m.text.clone())
                .collect();
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
            self.debug_expand_scroll_x = 0;
        } else {
            self.debug_expanded_row = Some(row_idx);
            self.debug_expand_scroll = 0;
            self.debug_expand_scroll_x = 0;
        }
    }

    pub fn start_file_picker(&mut self) {
        let all_entries = scan_project_files();
        let at_start = self.cursor_pos.saturating_sub(1);
        self.file_picker = Some(FilePicker {
            query: String::new(),
            at_start,
            all_entries,
            filtered: Vec::new(),
            selected: 0,
        });
        self.file_picker_update_filter();
    }

    fn file_picker_update_filter(&mut self) {
        if let Some(picker) = &mut self.file_picker {
            let query_lower = picker.query.to_lowercase();
            picker.filtered = picker
                .all_entries
                .iter()
                .filter(|entry| entry.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
            picker.selected = 0;
        }
    }

    pub fn file_picker_type(&mut self, c: char) {
        if let Some(picker) = &mut self.file_picker {
            picker.query.push(c);
        }
        self.file_picker_update_filter();
        // Auto-close if no matches: keep @+query as plain text in input
        if self
            .file_picker
            .as_ref()
            .is_some_and(|p| p.filtered.is_empty())
            && let Some(picker) = self.file_picker.take()
        {
            for ch in picker.query.chars() {
                self.input.insert(self.cursor_pos, ch);
                self.cursor_pos += ch.len_utf8();
            }
        }
    }

    pub fn file_picker_backspace(&mut self) -> bool {
        if let Some(picker) = &mut self.file_picker {
            if picker.query.is_empty() {
                return false; // Signal to close picker
            }
            picker.query.pop();
        } else {
            return true;
        }
        self.file_picker_update_filter();
        true
    }

    pub fn file_picker_up(&mut self) {
        if let Some(picker) = &mut self.file_picker {
            picker.selected = picker.selected.saturating_sub(1);
        }
    }

    pub fn file_picker_down(&mut self) {
        if let Some(picker) = &mut self.file_picker
            && picker.selected < picker.filtered.len().saturating_sub(1)
        {
            picker.selected += 1;
        }
    }

    pub fn file_picker_select(&mut self) {
        if let Some(picker) = self.file_picker.take()
            && let Some(path) = picker.filtered.get(picker.selected)
            && picker.at_start <= self.input.len()
        {
            // Get the filename (for display) and full relative path (for actual use)
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);

            // Format: "@filename" to display nicely
            let display_text = format!("@{}", filename);

            // Store mapping from filename to full path
            self.file_picker_map
                .insert(filename.to_string(), path.clone());

            // Replace from at_start to cursor_pos with display text
            let at_start = picker.at_start;
            let cursor_pos = self.cursor_pos;

            self.input.drain(at_start..cursor_pos);
            self.input.insert_str(at_start, &display_text);
            self.cursor_pos = at_start + display_text.len();
        }
    }

    // Slash command picker methods

    pub fn start_slash_command_help(&mut self) {
        let slash_start = self.cursor_pos.saturating_sub(1);

        // Show help with available commands
        self.slash_picker = Some(SlashCommandPicker {
            command: SlashCommand::Model, // Default, will switch
            query: String::new(),
            slash_start,
            all_entries: vec![
                "/model - Select a model".to_string(),
                "/connect - Set API key".to_string(),
                "/new - Start a new session".to_string(),
                "/clear - Clear current session".to_string(),
            ],
            filtered: vec![
                "/model - Select a model".to_string(),
                "/connect - Set API key".to_string(),
                "/new - Start a new session".to_string(),
                "/clear - Clear current session".to_string(),
            ],
            selected: 0,
        });
    }

    pub fn start_slash_picker(&mut self, command: SlashCommand, initial_text: &str) {
        let slash_start = self.cursor_pos.saturating_sub(1 + initial_text.len());

        let all_entries = match command {
            SlashCommand::Model => {
                // For Model, start with placeholder and fetch in background
                vec!["Loading models...".to_string()]
            }
            SlashCommand::Connect => vec!["Enter API key for current provider".to_string()],
            SlashCommand::New => vec!["Start a new session (clears current context)".to_string()],
            SlashCommand::Clear => vec!["Clear current session context".to_string()],
        };

        self.slash_picker = Some(SlashCommandPicker {
            command: command.clone(),
            query: initial_text.to_string(),
            slash_start,
            all_entries,
            filtered: Vec::new(),
            selected: 0,
        });
        self.slash_picker_update_filter();

        // Fetch models in background for Model command (don't block UI)
        if command == SlashCommand::Model {
            let tx = self.tx.clone();
            let provider = self.active_provider.clone();
            // Get models from DB now before spawning thread (DB can't be cloned)
            let db_models = if let Some(provider) = &provider {
                if let Some(db) = &self.db {
                    db.get_provider_models(&provider.name)
                        .ok()
                        .filter(|m| !m.is_empty())
                } else {
                    None
                }
            } else {
                None
            };

            std::thread::spawn(move || {
                let models = if let Some(models) = db_models {
                    models
                } else if let Some(provider) = &provider {
                    // Fetch from API if not in DB
                    crate::utils::fetch_available_models(
                        &provider.endpoint,
                        provider.api_key.as_deref(),
                    )
                } else {
                    Vec::new()
                };

                let _ = tx.send(LlmEvent::ModelsLoaded { models });
            });
        }
    }

    pub fn slash_picker_update_filter(&mut self) {
        if let Some(picker) = &mut self.slash_picker {
            let query_lower = picker.query.to_lowercase();
            picker.filtered = picker
                .all_entries
                .iter()
                .filter(|entry| entry.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
            picker.selected = 0;
        }
    }

    pub fn slash_picker_type(&mut self, c: char) {
        if let Some(picker) = &mut self.slash_picker {
            picker.query.push(c);
        }
        // Also insert into main input buffer so user sees what they're typing
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.slash_picker_update_filter();
    }

    pub fn slash_picker_backspace(&mut self) -> bool {
        if let Some(picker) = &mut self.slash_picker {
            if picker.query.is_empty() {
                return false; // Signal to close picker
            }
            picker.query.pop();
            // Also delete from main input buffer
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
        } else {
            return true;
        }
        self.slash_picker_update_filter();
        true
    }

    pub fn slash_picker_up(&mut self) {
        if let Some(picker) = &mut self.slash_picker {
            picker.selected = picker.selected.saturating_sub(1);
        }
    }

    pub fn slash_picker_down(&mut self) {
        if let Some(picker) = &mut self.slash_picker
            && picker.selected < picker.filtered.len().saturating_sub(1)
        {
            picker.selected += 1;
        }
    }

    pub fn slash_picker_select(&mut self) {
        if let Some(picker) = self.slash_picker.take()
            && picker.slash_start <= self.input.len()
        {
            // Check if we're in help mode (entries contain help text)
            if let Some(entry) = picker.filtered.get(picker.selected) {
                if entry.starts_with("/model -") {
                    // User selected /model from help - start model picker
                    self.start_slash_picker(SlashCommand::Model, &picker.query);
                    return;
                } else if entry.starts_with("/connect -") {
                    // User selected /connect from help
                    self.api_key_input = Some(String::new());
                    // Remove the slash command from input
                    let slash_start = picker.slash_start;
                    self.input.drain(slash_start..self.cursor_pos);
                    self.cursor_pos = slash_start;
                    self.set_status("Enter API key (press Enter when done)", StatusLevel::Info);
                    return;
                } else if entry.starts_with("/new -") {
                    // User selected /new from help
                    self.start_slash_picker(SlashCommand::New, &picker.query);
                    return;
                } else if entry.starts_with("/clear -") {
                    // User selected /clear from help
                    self.start_slash_picker(SlashCommand::Clear, &picker.query);
                    return;
                }
            }

            // Regular command handling
            match picker.command {
                SlashCommand::Model => {
                    // Get model from selection or use typed query as manual entry
                    let model = picker.filtered.get(picker.selected).cloned().or_else(|| {
                        // If nothing selected but user typed something, use that as manual model
                        if !picker.query.is_empty() {
                            Some(picker.query.clone())
                        } else {
                            None
                        }
                    });

                    if let Some(model) = model {
                        // Expand short form "provider/model" to full ID if needed
                        let full_model_id =
                            if model.contains('/') && !model.starts_with("accounts/") {
                                // User entered "fireworks/kimi-k2p5" format
                                format!("accounts/{}", model)
                            } else {
                                model
                            };

                        // Update the provider's default_model in memory
                        if let Some(provider) = &mut self.active_provider {
                            provider.default_model = Some(full_model_id.clone());
                            // Update in database
                            if let Some(db) = &self.db {
                                let _ = db.update_provider_model(&provider.name, &full_model_id);
                            }
                        }
                        // Remove the slash command and everything typed after it from input
                        self.input.drain(picker.slash_start..);
                        self.cursor_pos = picker.slash_start;
                        self.set_status(
                            format!("Switched to model: {}", full_model_id),
                            StatusLevel::Info,
                        );
                    }
                }
                SlashCommand::Connect => {
                    // Enter API key input mode
                    self.api_key_input = Some(String::new());
                    // Remove the slash command from input
                    let slash_start = picker.slash_start;
                    self.input.drain(slash_start..self.cursor_pos);
                    self.cursor_pos = slash_start;
                    self.set_status("Enter API key (press Enter when done)", StatusLevel::Info);
                }
                SlashCommand::New => {
                    // Start a new session: generate new session ID, clear messages but keep history for navigation
                    self.messages.clear();
                    self.unsent_draft.clear();
                    self.unsent_cursor_pos = 0;
                    self.auto_scroll = true;
                    // Reset token counts when starting a new session
                    self.total_input_tokens = 0;
                    self.total_output_tokens = 0;
                    self.last_output_tokens = 0;
                    let _old_session_id = std::mem::replace(
                        &mut self.session_id,
                        crate::utils::generate_session_id(),
                    );
                    if let Some(db) = &self.db {
                        let _ = db.create_session(&self.session_id, &self.working_directory, None);
                    }
                    // Reset mode to default
                    self.mode_name = self.default_mode_name.clone();
                    if let Some(mode_config) = self.modes_config.get(&self.mode_name) {
                        self.temperature = mode_config.temperature;
                        self.mode_color = mode_config.color.clone();
                    }
                    // Remove the slash command from input
                    let slash_start = picker.slash_start;
                    self.input.drain(slash_start..self.cursor_pos);
                    self.cursor_pos = slash_start;
                    self.set_status(
                        format!("Started new session: {}", self.session_id),
                        StatusLevel::Info,
                    );
                }
                SlashCommand::Clear => {
                    // Clear current session context but keep the session ID
                    // Note: We intentionally do NOT clear history here - /clear is an alias
                    // for /new, and both should preserve input history for navigation
                    self.messages.clear();
                    self.unsent_draft.clear();
                    self.unsent_cursor_pos = 0;
                    self.auto_scroll = true;
                    if let Some(db) = &self.db {
                        let _ = db.clear_session_messages(&self.session_id);
                    }
                    // Reset mode to default
                    self.mode_name = self.default_mode_name.clone();
                    if let Some(mode_config) = self.modes_config.get(&self.mode_name) {
                        self.temperature = mode_config.temperature;
                        self.mode_color = mode_config.color.clone();
                    }
                    // Remove the slash command from input
                    let slash_start = picker.slash_start;
                    self.input.drain(slash_start..self.cursor_pos);
                    self.cursor_pos = slash_start;
                    self.set_status("Cleared current session", StatusLevel::Info);
                }
            }
        }
    }

    pub fn handle_api_key_input(&mut self, c: char) {
        if let Some(key) = &mut self.api_key_input {
            key.push(c);
        }
    }

    pub fn handle_api_key_backspace(&mut self) -> bool {
        if let Some(key) = &mut self.api_key_input {
            if key.is_empty() {
                self.api_key_input = None;
                return false;
            }
            key.pop();
            return true;
        }
        false
    }

    pub fn submit_api_key(&mut self) {
        if let Some(key) = self.api_key_input.take()
            && let Some(provider) = self.active_provider.as_ref()
        {
            let provider_name = provider.name.clone();

            // Update the provider's API key in memory
            let mut updated_provider = provider.clone();
            updated_provider.api_key = Some(key.clone());
            self.active_provider = Some(updated_provider);

            // Update in database
            if let Some(db) = &self.db {
                let _ = db.update_provider_api_key(&provider_name, &key);
            }

            self.set_status(
                format!("API key set for {}", provider_name),
                StatusLevel::Info,
            );
        }
    }
}
