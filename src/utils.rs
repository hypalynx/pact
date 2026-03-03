use std::path::PathBuf;

pub fn get_pwd_display() -> String {
    match std::env::current_dir() {
        Ok(path) => {
            // Show just the current directory name
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string())
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

pub fn extract_and_format_model_name(raw_name: &str) -> String {
    let mut name = raw_name.to_string();

    // Remove .gguf extension if present
    if name.ends_with(".gguf") {
        name = name[..name.len() - 5].to_string();

        // Remove quantization suffix (e.g., Q4_K_M, IQ3_XS, q6-k, etc.)
        // Look for -Q<digits> or -IQ<digits> pattern (case-insensitive)
        let name_lower = name.to_lowercase();
        for (i, _) in name_lower.match_indices('-') {
            let after_dash = &name_lower[i + 1..];
            // Check if this dash starts a quant suffix: Q/IQ followed by digit
            let is_quant = if after_dash.starts_with("iq") {
                after_dash
                    .chars()
                    .nth(2)
                    .is_some_and(|c| c.is_ascii_digit())
            } else if after_dash.starts_with('q') {
                after_dash
                    .chars()
                    .nth(1)
                    .is_some_and(|c| c.is_ascii_digit())
            } else {
                false
            };
            if is_quant {
                name = name[..i].to_string();
                break;
            }
        }
    }

    // Convert to kebab-case: lowercase and replace underscores with hyphens
    name.to_lowercase().replace('_', "-")
}

pub fn fetch_server_info(endpoint: &str) -> ServerInfo {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    // Trim trailing /v1 from endpoint to avoid /v1/v1/models
    // But keep /inference as it's part of Fireworks base URL
    let base = endpoint.trim_end_matches("/v1");

    if let Ok(response) = client.get(format!("{}/v1/models", base)).send()
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
                .map(extract_and_format_model_name)
                .unwrap_or_else(|| "unknown".to_string());
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
            .map(extract_and_format_model_name)
            .unwrap_or_else(|| "unknown".to_string());
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

/// Fetch all available models from the provider's /v1/models endpoint
/// Returns a list of model IDs that can be used for the /model command
pub fn fetch_available_models(endpoint: &str, api_key: Option<&str>) -> Vec<String> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    // Trim trailing /v1 from endpoint if present to avoid /v1/v1/models
    // But keep /inference as it's part of Fireworks base URL
    let base = endpoint.trim_end_matches("/v1");
    let url = format!("{}/v1/models", base);

    let mut request = client.get(&url);

    // Add Authorization header if API key is provided
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    if let Ok(response) = request.send()
        && let Ok(text) = response.text()
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
    {
        // Standard OpenAI format: { "data": [{ "id": "..." }] }
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            return data
                .iter()
                .filter_map(|model| {
                    model
                        .get("id")
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
        }

        // Try single model response format (llama.cpp server)
        if let Some(id) = json.get("id").and_then(|id| id.as_str()) {
            return vec![id.to_string()];
        }
    }

    // Return empty list - no fallback
    Vec::new()
}

pub fn db_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".local/share/pact");
    std::fs::create_dir_all(&path).ok();
    path.push("pact.db");
    path
}

/// Generate a short 7-character hex session ID
pub fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!("{:07x}", timestamp % 0x10000000)
}
