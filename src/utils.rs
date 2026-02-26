use std::path::PathBuf;

pub fn get_pwd_display() -> String {
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

pub fn get_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
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

pub fn format_tokens(tokens: usize) -> String {
    if tokens >= 1000 {
        format!("{:.0}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    }
}

pub struct ServerInfo {
    pub model_name: String,
    pub context_window: usize,
}

pub fn fetch_server_info(endpoint: &str) -> ServerInfo {
    let client = reqwest::blocking::Client::new();

    if let Ok(response) = client.get(format!("{}/v1/models", endpoint)).send()
        && let Ok(text) = response.text()
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
    {
        // Try data array format
        if let Some(data) = json.get("data").and_then(|d| d.as_array())
            && let Some(first_model) = data.first()
        {
            let model_name = first_model
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown")
                .to_string();
            let context_window = first_model
                .get("max_tokens")
                .and_then(|m| m.as_u64())
                .unwrap_or(65535) as usize;
            return ServerInfo {
                model_name,
                context_window,
            };
        }

        // Try single model response format
        let model_name = json
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("unknown")
            .to_string();
        let context_window = json
            .get("max_tokens")
            .and_then(|m| m.as_u64())
            .unwrap_or(65535) as usize;
        return ServerInfo {
            model_name,
            context_window,
        };
    }

    ServerInfo {
        model_name: "unknown".to_string(),
        context_window: 65535,
    }
}

pub fn messages_path() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| {
        let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.push(".local/share");
        home
    });
    path.push("pact");
    std::fs::create_dir_all(&path).ok();
    path.push("messages.json");
    path
}
