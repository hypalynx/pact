use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mode {
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default)]
    pub modes: IndexMap<String, Mode>,
}

fn default_endpoint() -> String {
    "http://127.0.0.1:7777".to_string()
}

fn default_max_tokens() -> usize {
    1024
}

fn default_mode() -> String {
    "build".to_string()
}

impl Default for Mode {
    fn default() -> Self {
        Self {
            system_prompt: None,
            temperature: None,
            color: None,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            endpoint: default_endpoint(),
            max_tokens: default_max_tokens(),
            api_key: None,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_mode: default_mode(),
            modes: IndexMap::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Config {
    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| {
            let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            home.push(".config");
            home
        });
        path.push("pact");
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

    fn default_modes() -> IndexMap<String, Mode> {
        let mut modes = IndexMap::new();
        modes.insert(
            "build".to_string(),
            Mode {
                system_prompt: Some("You are a helpful coding assistant...".to_string()),
                temperature: None,
                color: Some("cyan".to_string()),
            },
        );
        modes.insert(
            "plan".to_string(),
            Mode {
                system_prompt: Some("You are an expert at planning implementations...".to_string()),
                temperature: Some(0.5),
                color: Some("green".to_string()),
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
}
