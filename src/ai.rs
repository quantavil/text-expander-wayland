use std::process::Command;
use std::thread;
use std::time::Duration;
use crate::config::AiConfig;
use crate::inject::{copy_to_clipboard, clear_clipboard, simulate_copy, type_expansion};

const CLIPBOARD_POLL_RETRIES: usize = 25;
const CLIPBOARD_POLL_INTERVAL_MS: u64 = 20;
const CLIPBOARD_RESTORE_DELAY_MS: u64 = 800;
const CURL_MAX_TIME_SEC: &str = "10";

pub fn trigger_ai_fix(prompt: &str, ai_config: &AiConfig) -> Result<(), Box<dyn std::error::Error>> {
    let original_clipboard = crate::config::run_command("wl-paste", &["-n"]);

    for _ in 0..50 {
        if crate::input::MODIFIERS_DOWN.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    clear_clipboard();
    simulate_copy();

    let mut selected_text = String::new();
    for _ in 0..CLIPBOARD_POLL_RETRIES {
        thread::sleep(Duration::from_millis(CLIPBOARD_POLL_INTERVAL_MS));
        let text = crate::config::run_command("wl-paste", &["-n"]);
        if !text.is_empty() {
            selected_text = text;
            break;
        }
    }

    if selected_text.is_empty() {
        copy_to_clipboard(&original_clipboard);
        return Ok(());
    }

    let api_key = match &ai_config.api_key {
        Some(key) if !key.trim().is_empty() => key.trim().to_string(),
        _ => {
            copy_to_clipboard(&original_clipboard);
            return Err("AI API key is missing. Please configure 'ai.api_key' in base.yml.".into());
        }
    };

    let endpoint = ai_config.endpoint.as_deref()
        .map(|s| s.trim())
        .unwrap_or("https://generativelanguage.googleapis.com/v1beta/openai/chat/completions");
    
    let model = ai_config.model.as_deref()
        .map(|s| s.trim())
        .unwrap_or("gemini-3.1-flash-lite");

    let payload = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": prompt
            },
            {
                "role": "user",
                "content": selected_text
            }
        ]
    });

    let mut cmd = Command::new("curl");
    cmd.args(["-s", "-X", "POST", "-K", "-", "--max-time", CURL_MAX_TIME_SEC]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    let payload_str = payload.to_string();
    let escaped_payload = payload_str.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_api_key = api_key.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_endpoint = endpoint.replace('\\', "\\\\").replace('"', "\\\"");

    let config_input = format!(
        "url = \"{}\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json\"\ndata = \"{}\"\n",
        escaped_endpoint,
        escaped_api_key,
        escaped_payload
    );

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(config_input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
        copy_to_clipboard(&original_clipboard);
        return Err(format!("Curl process failed: {}", err_msg).into());
    }

    let response_str = String::from_utf8(output.stdout)?;
    let response: serde_json::Value = serde_json::from_str(&response_str)?;

    if let Some(err) = response.get("error") {
        if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
            copy_to_clipboard(&original_clipboard);
            return Err(format!("API error response: {}", msg).into());
        }
    }

    let corrected_text = response["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("Invalid response format: 'choices[0].message.content' not found.")?
        .trim();

    if corrected_text.is_empty() {
        copy_to_clipboard(&original_clipboard);
        return Err("API returned empty text selection.".into());
    }

    copy_to_clipboard(corrected_text);
    type_expansion(0, corrected_text, None, true);

    let saved = original_clipboard;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS));
        copy_to_clipboard(&saved);
    });

    Ok(())
}
