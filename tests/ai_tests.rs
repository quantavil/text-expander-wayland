use std::process::Command;
use text_expander::ai::*;
use text_expander::config::AiConfig;

fn make_config(api_key: Option<&str>, model: Option<&str>, endpoint: Option<&str>) -> AiConfig {
    AiConfig {
        api_key: api_key.map(String::from),
        model: model.map(String::from),
        endpoint: endpoint.map(String::from),
        matches: vec![],
    }
}

// --- validate_api_key ---

#[test]
fn test_validate_api_key_present() {
    let cfg = make_config(Some("my-secret-key"), None, None);
    assert_eq!(validate_api_key(&cfg).unwrap(), "my-secret-key");
}

#[test]
fn test_validate_api_key_whitespace_trimmed() {
    let cfg = make_config(Some("  key-with-spaces  "), None, None);
    assert_eq!(validate_api_key(&cfg).unwrap(), "key-with-spaces");
}

#[test]
fn test_validate_api_key_missing() {
    let cfg = make_config(None, None, None);
    assert!(validate_api_key(&cfg).is_err());
}

#[test]
fn test_validate_api_key_empty() {
    let cfg = make_config(Some(""), None, None);
    assert!(validate_api_key(&cfg).is_err());
}

#[test]
fn test_validate_api_key_only_whitespace() {
    let cfg = make_config(Some("   "), None, None);
    assert!(validate_api_key(&cfg).is_err());
}

// --- build_payload ---

#[test]
fn test_build_payload_structure() {
    let cfg = make_config(Some("k"), None, None);
    let payload = build_payload("Fix grammar", "hello wrold", &cfg);

    assert_eq!(payload["model"], "gemini-3.1-flash-lite");
    let msgs = payload["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[0]["content"], "Fix grammar");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "hello wrold");
}

#[test]
fn test_build_payload_custom_model() {
    let cfg = make_config(Some("k"), Some("gpt-4"), None);
    let payload = build_payload("prompt", "text", &cfg);
    assert_eq!(payload["model"], "gpt-4");
}

#[test]
fn test_build_payload_model_trimmed() {
    let cfg = make_config(Some("k"), Some("  gpt-4  "), None);
    let payload = build_payload("prompt", "text", &cfg);
    assert_eq!(payload["model"], "gpt-4");
}

// --- build_curl_config ---

#[test]
fn test_build_curl_config_contains_required_fields() {
    let payload = serde_json::json!({"model": "test", "messages": []});
    let config = build_curl_config("https://example.com/api", "my-key", &payload);

    assert!(config.contains("url = \"https://example.com/api\""));
    assert!(config.contains("Authorization: Bearer my-key"));
    assert!(config.contains("Content-Type: application/json"));
    assert!(config.contains("data = \""));
}

#[test]
fn test_build_curl_config_escapes_special_chars() {
    let payload = serde_json::json!({"msg": "has \"quotes\""});
    let config = build_curl_config("https://api.example.com", "key\"with\"quotes", &payload);

    assert!(config.contains("key\\\"with\\\"quotes"));
}

// --- resolve_endpoint ---

#[test]
fn test_resolve_endpoint_default() {
    let cfg = make_config(Some("k"), None, None);
    assert_eq!(
        resolve_endpoint(&cfg),
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
    );
}

#[test]
fn test_resolve_endpoint_custom() {
    let cfg = make_config(Some("k"), None, Some("https://custom.api/v1"));
    assert_eq!(resolve_endpoint(&cfg), "https://custom.api/v1");
}

#[test]
fn test_resolve_endpoint_trimmed() {
    let cfg = make_config(Some("k"), None, Some("  https://custom.api/v1  "));
    assert_eq!(resolve_endpoint(&cfg), "https://custom.api/v1");
}

// --- parse_ai_response ---

#[test]
fn test_parse_response_valid() {
    let json = r#"{
        "choices": [{
            "message": { "content": "hello world" }
        }]
    }"#;
    assert_eq!(parse_ai_response(json).unwrap(), "hello world");
}

#[test]
fn test_parse_response_trims_whitespace() {
    let json = r#"{
        "choices": [{
            "message": { "content": "  trimmed  " }
        }]
    }"#;
    assert_eq!(parse_ai_response(json).unwrap(), "trimmed");
}

#[test]
fn test_parse_response_api_error() {
    let json = r#"{"error": {"message": "Invalid API key"}}"#;
    let err = parse_ai_response(json).unwrap_err();
    assert!(err.contains("Invalid API key"));
}

#[test]
fn test_parse_response_empty_content() {
    let json = r#"{"choices": [{"message": {"content": "  "}}]}"#;
    let err = parse_ai_response(json).unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn test_parse_response_missing_choices() {
    let json = r#"{"id": "123"}"#;
    let err = parse_ai_response(json).unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn test_parse_response_invalid_json() {
    let err = parse_ai_response("not json at all").unwrap_err();
    assert!(err.contains("Invalid JSON"));
}

// --- Live integration test (run with: cargo test -- --ignored) ---

#[test]
#[ignore]
fn test_live_gemini_api_call() {
    let api_key = std::env::var("TEXT_EXPANDER_AI_KEY")
        .expect("Set TEXT_EXPANDER_AI_KEY env var to run this test");

    let cfg = make_config(Some(&api_key), None, None);
    let endpoint = resolve_endpoint(&cfg);
    let prompt = "Reply with exactly the word 'pong' and nothing else.";
    let payload = build_payload(prompt, "ping", &cfg);
    let config_input = build_curl_config(endpoint, &api_key, &payload);

    let mut cmd = Command::new("curl");
    cmd.args(["-s", "-X", "POST", "-K", "-", "--max-time", "15"]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().expect("Failed to spawn curl");

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(config_input.as_bytes()).expect("Failed to write curl config");
    }

    let output = child.wait_with_output().expect("Curl failed");
    assert!(output.status.success(), "curl failed: {}", String::from_utf8_lossy(&output.stderr));

    let response_str = String::from_utf8(output.stdout).expect("Non-UTF8 response");
    eprintln!("API response: {}", response_str);

    let result = parse_ai_response(&response_str).expect("Failed to parse API response");
    let result_lower = result.to_lowercase();
    assert!(
        result_lower.contains("pong"),
        "Expected 'pong' in response, got: '{}'", result
    );
}

#[test]
fn test_parse_yaml_config() {
    let yaml_str = r#"
matches:
  - trigger: ";sig"
    replace: "Best regards"
ai:
  api_key: "my-key"
  endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
  model: "gemini-3.1-flash-lite"
  matches:
    - hotkey: "ctrl+alt+f"
      prompt: "Correct grammar"
"#;
    let config: text_expander::config::EspansoConfig = serde_norway::from_str(yaml_str).unwrap();
    assert_eq!(config.matches.len(), 1);
    let ai = config.ai.unwrap();
    assert_eq!(ai.api_key.as_deref(), Some("my-key"));
    assert_eq!(ai.endpoint.as_deref(), Some("https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"));
    assert_eq!(ai.model.as_deref(), Some("gemini-3.1-flash-lite"));
    assert_eq!(ai.matches.len(), 1);
    assert_eq!(ai.matches[0].hotkey, "ctrl+alt+f");
    assert_eq!(ai.matches[0].prompt, "Correct grammar");
}

