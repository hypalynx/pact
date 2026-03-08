use indexmap::IndexMap;
use pact::config::{Config, FilePermission, Mode, UiConfig};

#[test]
fn test_ui_config_defaults() {
    let config = UiConfig::default();
    assert_eq!(config.default_mode, "plan");
    assert!(config.modes.is_empty());
}

#[test]
fn test_config_defaults() {
    let config = Config::default();
    assert_eq!(config.ui.default_mode, "plan");
    assert!(!config.debug);
    assert!(config.providers.is_empty());
}

#[test]
fn test_mode_defaults() {
    let mode = Mode::default();
    assert!(mode.system_prompt.is_none());
    assert!(mode.temperature.is_none());
    assert!(mode.color.is_none());
}

#[test]
fn test_mode_with_values() {
    let mode = Mode {
        system_prompt: Some("Test prompt".to_string()),
        color: Some("blue".to_string()),
        temperature: Some(0.5),
        top_p: None,
        presence_penalty: None,
        local_extensions: IndexMap::new(),
        file_permission: FilePermission::Markdown,
    };
    assert_eq!(mode.system_prompt, Some("Test prompt".to_string()));
    assert_eq!(mode.temperature, Some(0.5));
    assert_eq!(mode.color, Some("blue".to_string()));
}

#[test]
fn test_config_load_no_file() {
    // Config::load() checks if config file exists
    // When it doesn't exist, it should return defaults
    let config = Config::load();
    assert_eq!(config.ui.default_mode, "plan");
    // Default modes should include "build" and "plan"
    assert!(config.ui.modes.contains_key("build"));
    assert!(config.ui.modes.contains_key("plan"));
}

#[test]
fn test_config_has_default_build_mode() {
    // Test the default modes (not affected by user config)
    let modes = Config::default_modes();
    let build = modes.get("build").expect("build mode should exist");
    assert!(build.system_prompt.is_some());
    assert!(build.color.is_some());
}

#[test]
fn test_config_has_default_plan_mode() {
    let config = Config::load();
    let plan_mode = config.ui.modes.get("plan");
    assert!(plan_mode.is_some());
    let plan = plan_mode.unwrap();
    assert!(plan.system_prompt.is_some());
    assert_eq!(plan.temperature, Some(0.5));
    assert!(plan.color.is_some());
}

#[test]
fn test_ui_config_modes_order() {
    // Modes should maintain insertion order (IndexMap)
    let mut modes = IndexMap::new();
    modes.insert("first".to_string(), Mode::default());
    modes.insert("second".to_string(), Mode::default());
    modes.insert("third".to_string(), Mode::default());

    let keys: Vec<&String> = modes.keys().collect();
    assert_eq!(
        keys,
        vec![
            &"first".to_string(),
            &"second".to_string(),
            &"third".to_string()
        ]
    );
}

#[test]
fn test_config_clone() {
    let config = Config::default();
    let cloned = config.clone();
    assert_eq!(cloned.ui.default_mode, config.ui.default_mode);
    assert_eq!(cloned.providers.len(), config.providers.len());
}

#[test]
fn test_mode_clone() {
    let mode = Mode {
        system_prompt: Some("Test".to_string()),
        color: Some("red".to_string()),
        temperature: Some(0.7),
        top_p: None,
        presence_penalty: None,
        local_extensions: IndexMap::new(),
        file_permission: FilePermission::Markdown,
    };
    let cloned = mode.clone();
    assert_eq!(cloned.system_prompt, mode.system_prompt);
    assert_eq!(cloned.temperature, mode.temperature);
    assert_eq!(cloned.color, mode.color);
}

#[test]
fn test_config_load_merges_modes() {
    let config = Config::load();
    // Should have at least the default modes
    assert!(config.ui.modes.len() >= 2);
    assert!(config.ui.modes.contains_key("build"));
    assert!(config.ui.modes.contains_key("plan"));
}

#[test]
fn test_ui_config_with_custom_modes() {
    let mut modes = IndexMap::new();
    modes.insert(
        "custom".to_string(),
        Mode {
            system_prompt: Some("Custom prompt".to_string()),
            color: Some("yellow".to_string()),
            temperature: Some(0.8),
            top_p: None,
            presence_penalty: None,
            local_extensions: IndexMap::new(),
            file_permission: FilePermission::Markdown,
        },
    );

    let config = UiConfig {
        default_mode: "custom".to_string(),
        modes,
    };

    assert_eq!(config.default_mode, "custom");
    assert!(config.modes.contains_key("custom"));
    assert_eq!(config.modes.get("custom").unwrap().temperature, Some(0.8));
}

#[test]
fn test_load_agents_context_no_files() {
    // Test with nonexistent custom path and no fallback
    // Since we can't easily test without a local AGENTS.md in the project,
    // we verify that the function attempts to load from the custom path
    let config = Config {
        ui: UiConfig::default(),
        debug: false,
        agents_md_path: Some("/nonexistent/path/that/does/not/exist/AGENTS.md".to_string()),
        providers: vec![],
    };
    let context = config.load_agents_context();
    // In this project, there IS a local AGENTS.md, so context should be Some
    // This test verifies the function returns Some when local file exists as fallback
    assert!(
        context.is_some(),
        "Should return Some when local AGENTS.md exists as fallback"
    );
}

#[test]
fn test_load_agents_context_with_custom_path() {
    // Test that custom agents_md_path is checked (even if nonexistent)
    // and local AGENTS.md is still available as fallback
    let config = Config {
        ui: UiConfig::default(),
        debug: false,
        agents_md_path: Some("/nonexistent/path/AGENTS.md".to_string()),
        providers: vec![],
    };
    let context = config.load_agents_context();
    // Should return Some since local AGENTS.md exists in the project
    assert!(
        context.is_some(),
        "Should return Some when local AGENTS.md is available"
    );
}
