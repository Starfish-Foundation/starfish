#[cfg(test)]
mod tests {
    use crate::config::*;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("bfish-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn defaults_include_all_six_providers() {
        let settings = Settings::default();
        assert_eq!(settings.providers.len(), 6);
        for preset in provider_presets() {
            assert!(
                settings.providers.contains_key(preset.id),
                "missing provider {}",
                preset.id
            );
        }
        assert_eq!(settings.mcp_port, DEFAULT_MCP_PORT);
        assert!(!settings.system_prompt.is_empty());
    }

    #[test]
    fn settings_round_trip() {
        let dir = temp_dir();
        let store = SettingsStore::new(&dir);
        let mut settings = Settings::default();
        settings.provider = "ollama".to_string();
        settings.model = "qwen3:8b".to_string();
        settings
            .providers
            .get_mut("ollama")
            .expect("ollama preset")
            .api_key = "local".to_string();
        settings.temperature = 0.7;

        store.save(&settings).expect("save");
        let loaded = store.load();
        assert_eq!(loaded.provider, "ollama");
        assert_eq!(loaded.model, "qwen3:8b");
        assert_eq!(loaded.temperature, 0.7);
        assert_eq!(loaded.providers["ollama"].api_key, "local");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_provider_entries_are_repaired() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"provider":"gemini","model":"","providers":{},"mcpPort":0}"#,
        )
        .unwrap();
        let store = SettingsStore::new(&dir);
        let settings = store.load();
        assert_eq!(settings.providers.len(), 6);
        assert_eq!(settings.provider, "gemini");
        assert!(!settings.model.is_empty());
        assert_eq!(settings.mcp_port, DEFAULT_MCP_PORT);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn presets_have_models_and_urls() {
        for preset in provider_presets() {
            assert!(!preset.default_model.is_empty());
            assert!(preset.default_base_url.starts_with("http"));
            assert!(!preset.models.is_empty());
        }
    }
}
