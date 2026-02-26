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

pub fn format_tokens(tokens: usize) -> String {
    if tokens >= 1000 {
        format!("{:.0}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    }
}

pub fn fetch_context_window(endpoint: &str) -> usize {
    let client = reqwest::blocking::Client::new();

    if let Ok(response) = client.get(&format!("{}/v1/models", endpoint)).send() {
        if let Ok(text) = response.text() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if let Some(first_model) = data.first() {
                        if let Some(max_tokens) = first_model.get("max_tokens").and_then(|m| m.as_u64()) {
                            return max_tokens as usize;
                        }
                    }
                }

                if let Some(max_tokens) = json.get("max_tokens").and_then(|m| m.as_u64()) {
                    return max_tokens as usize;
                }
            }
        }
    }

    65535
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
