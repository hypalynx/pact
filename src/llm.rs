use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub text: String,
}

pub enum LlmEvent {
    Token(String),
    Done,
    Error(String),
    Usage { input_tokens: usize, output_tokens: usize },
}

pub fn call_llm(
    messages: Vec<Message>,
    tx: mpsc::Sender<LlmEvent>,
    debug: bool,
    endpoint: &str,
    max_tokens: usize,
    temperature: Option<f32>,
    system_prompt: Option<String>,
) {
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

    let mut msg_payload: Vec<_> = Vec::new();

    // Prepend system prompt as first user message if present
    if let Some(prompt) = system_prompt {
        msg_payload.push(json!({ "role": "user", "content": prompt }));
    }

    // Add conversation messages
    for m in messages {
        msg_payload.push(json!({ "role": m.role, "content": m.text }));
    }

    let mut body = json!({
        "model": "local",
        "max_tokens": max_tokens,
        "stream": true,
        "messages": msg_payload,
    });

    if let Some(temp) = temperature {
        body["temperature"] = json!(temp);
    }

    if debug {
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("api.log")
            .and_then(|mut file| {
                writeln!(file, "=== REQUEST {} ===", chrono::Local::now())?;
                writeln!(file, "POST {}/v1/messages", endpoint)?;
                writeln!(file, "{}", serde_json::to_string_pretty(&body).unwrap_or_default())?;
                writeln!(file, "\n=== RESPONSE ===")?;
                Ok(())
            });
    }

    let response = match client.post(&format!("{}/v1/messages", endpoint)).json(&body).send() {
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
                    if let Some(delta) = json_val
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        let _ = tx.send(LlmEvent::Token(delta.to_string()));
                    }

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
