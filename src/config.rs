use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FilePermission {
    Full,
    #[default]
    Markdown,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub debug: bool,
    pub agents_md_path: Option<String>,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub endpoint: String,
    pub default_model: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Mode {
    pub system_prompt: Option<String>,
    pub color: Option<String>,
    // OpenAI-compatible parameters
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    // Backend-specific extensions (applied only if local backend)
    #[serde(default)]
    pub local_extensions: IndexMap<String, serde_json::Value>,
    // File write permissions for this mode
    #[serde(default)]
    pub file_permission: FilePermission,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default)]
    pub modes: IndexMap<String, Mode>,
}

fn default_mode() -> String {
    "plan".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_mode: default_mode(),
            modes: IndexMap::new(),
        }
    }
}

impl Config {
    fn config_path() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".config/pact");
        fs::create_dir_all(&path).ok();
        path.push("pact.yaml");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_yaml::from_str(&content).unwrap_or_default(),
                Err(_) => Config::default(),
            }
        } else {
            Config::default()
        };

        // Merge default modes with user config
        config.ui.modes = Self::default_modes_merged(&config.ui.modes);
        config
    }

    pub fn default_modes() -> IndexMap<String, Mode> {
        let mut modes = IndexMap::new();
        modes.insert(
            "plan".to_string(),
            Mode {
                system_prompt: Some(
                    "You are in PLAN mode - for analysis, exploration, and designing solutions.

You can:
- Read any file (Read, Glob, Grep tools)
- Run shell commands to understand the project (Bash)
- Fetch web content (Webfetch)
- Write and edit markdown files (.md) for plans and notes

You CANNOT write or edit non-markdown files in this mode.
If you need to implement code, tell the user to switch to Build mode (you cannot do this yourself).

Focus on: understanding the codebase, designing solutions, writing clear plans."
                        .to_string(),
                ),
                color: Some("green".to_string()),
                temperature: Some(0.5),
                top_p: Some(0.9),
                presence_penalty: None,
                local_extensions: IndexMap::new(),
                file_permission: FilePermission::Markdown,
            },
        );
        modes.insert(
            "build".to_string(),
            Mode {
                system_prompt: Some(
                    "You are in BUILD mode - full capability implementation assistant.

All tools are available: Read, Glob, Grep, Write, Edit, Bash, Webfetch.

Focus on: implementing, debugging, and refactoring code.
Press Tab to switch to Plan mode for analysis and planning."
                        .to_string(),
                ),
                color: Some("cyan".to_string()),
                temperature: Some(0.3),
                top_p: Some(0.9),
                presence_penalty: None,
                local_extensions: IndexMap::new(),
                file_permission: FilePermission::Full,
            },
        );
        modes.insert(
            "research".to_string(),
            Mode {
                system_prompt: Some(
                    "You are in RESEARCH mode - for exploring topics and writing.

You can:
- Read any file (Read, Glob, Grep tools)
- Run shell commands to understand the project (Bash)
- Fetch web content (Webfetch)
- Write and edit markdown files (.md) for notes, documentation, and drafts

You CANNOT write or edit non-markdown files in this mode.
For code implementation, switch to Build mode.

Focus on: researching topics, exploring ideas, writing prose."
                        .to_string(),
                ),
                color: Some("blue".to_string()),
                temperature: Some(0.75),
                top_p: Some(0.92),
                presence_penalty: Some(0.4),
                local_extensions: IndexMap::new(),
                file_permission: FilePermission::Markdown,
            },
        );
        modes
    }

    fn default_modes_merged(user_modes: &IndexMap<String, Mode>) -> IndexMap<String, Mode> {
        let mut modes = Self::default_modes();
        // User config overrides defaults
        for (name, mode) in user_modes {
            modes.insert(name.clone(), mode.clone());
        }
        modes
    }

    pub fn load_agents_context(&self) -> Option<String> {
        let mut parts = Vec::new();

        // Load global AGENTS.md (or custom path)
        let global_path = if let Some(custom_path) = &self.agents_md_path {
            PathBuf::from(custom_path)
        } else {
            let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            path.push(".config/pact/AGENTS.md");
            path
        };

        if let Ok(content) = fs::read_to_string(&global_path) {
            parts.push(content);
        }

        // Load local AGENTS.md (in current directory)
        let local_path = PathBuf::from("AGENTS.md");
        if let Ok(content) = fs::read_to_string(&local_path) {
            parts.push(content);
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }
}
