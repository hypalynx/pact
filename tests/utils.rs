use pact::utils::{extract_and_format_model_name, format_tokens, get_git_branch, get_pwd_display};

#[test]
fn test_format_tokens_under_1000() {
    assert_eq!(format_tokens(0), "0");
    assert_eq!(format_tokens(1), "1");
    assert_eq!(format_tokens(999), "999");
}

#[test]
fn test_format_tokens_1000_and_above() {
    assert_eq!(format_tokens(1000), "1k");
    assert_eq!(format_tokens(1500), "2k");
    assert_eq!(format_tokens(5000), "5k");
    assert_eq!(format_tokens(10000), "10k");
    assert_eq!(format_tokens(999999), "1000k");
}

#[test]
fn test_format_tokens_rounding() {
    // Test rounding behavior
    assert_eq!(format_tokens(1400), "1k");
    assert_eq!(format_tokens(1500), "2k");
    assert_eq!(format_tokens(2400), "2k");
    assert_eq!(format_tokens(2500), "2k"); // 2.5 rounds to 2
}

#[test]
fn test_format_tokens_edge_cases() {
    assert_eq!(format_tokens(950), "950");
    assert_eq!(format_tokens(1049), "1k");
    assert_eq!(format_tokens(1050), "1k");
}

#[test]
fn test_get_pwd_display() {
    // This function depends on current_dir, so we just verify it returns a string
    let pwd = get_pwd_display();
    assert!(!pwd.is_empty());
    // It should return just the directory name (no path separators)
    assert!(!pwd.contains('/'));
}

#[test]
fn test_get_pwd_display_not_error() {
    // Ensure it doesn't panic and returns something
    let result = std::panic::catch_unwind(|| get_pwd_display());
    assert!(result.is_ok());
}

#[test]
fn test_db_path() {
    use pact::utils::db_path;
    let path = db_path();
    assert!(path.ends_with("pact.db"));
    // Path should contain pact in the path
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("pact"));
}

#[test]
fn test_db_path_creates_directory() {
    use pact::utils::db_path;
    let path = db_path();
    // The directory should exist after calling db_path
    let parent = path.parent().unwrap();
    assert!(parent.exists());
}

#[test]
fn test_get_git_branch() {
    // This function depends on git being available
    // It may return Some or None depending on the environment
    let branch = get_git_branch();
    // If we're in a git repo, we should get Some("main") or similar
    // If not in a git repo, we get None
    // Both are valid, we just verify it doesn't panic
    match branch {
        Some(b) => assert!(!b.is_empty()),
        None => {} // Also valid if not in a git repo
    }
}

#[test]
fn test_fetch_server_info_default_on_error() {
    // Test with invalid endpoint to trigger error handling
    use pact::utils::fetch_server_info;
    let info = fetch_server_info("http://invalid-endpoint-that-does-not-exist:7777");
    // Should return defaults on error
    assert_eq!(info.model_name, "unknown");
    assert_eq!(info.context_window, 65535);
}

#[test]
fn test_extract_model_name_with_gguf_and_quant() {
    // Test GGUF with quantization suffix
    assert_eq!(
        extract_and_format_model_name("Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf"),
        "qwen3-coder-30b-a3b-instruct"
    );
}

#[test]
fn test_extract_model_name_with_gguf_different_quant() {
    // Test different quantization format (multi-part: q6-k)
    assert_eq!(
        extract_and_format_model_name("llama-2-13b-q6-k.gguf"),
        "llama-2-13b"
    );
}

#[test]
fn test_extract_model_name_with_iq_quant() {
    // Test IQ quantization format
    assert_eq!(
        extract_and_format_model_name("mistral-7b-IQ3_XS.gguf"),
        "mistral-7b"
    );
}

#[test]
fn test_extract_model_name_without_gguf() {
    // Test non-GGUF model names (e.g., cloud providers)
    assert_eq!(extract_and_format_model_name("gpt-4-turbo"), "gpt-4-turbo");
    assert_eq!(
        extract_and_format_model_name("moonshot-v1-8k"),
        "moonshot-v1-8k"
    );
}

#[test]
fn test_extract_model_name_kebab_case_conversion() {
    // Test underscore to kebab-case conversion
    assert_eq!(
        extract_and_format_model_name("my_model_name"),
        "my-model-name"
    );
}

#[test]
fn test_extract_model_name_uppercase_to_lowercase() {
    // Test case conversion
    assert_eq!(extract_and_format_model_name("MyModelName"), "mymodelname");
}
