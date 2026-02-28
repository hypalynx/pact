use pact::utils::{format_tokens, get_git_branch, get_pwd_display};

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
    // It should either be ".", a path, or contain "~" if in home
    assert!(pwd == "." || pwd.starts_with("/") || pwd.contains("~"));
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
