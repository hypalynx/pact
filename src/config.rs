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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_mode")]
    pub default_mode: String,
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

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            endpoint: default_endpoint(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_mode: default_mode(),
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
        if !path.exists() {
            return Config::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_yaml::from_str(&content).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }
}
