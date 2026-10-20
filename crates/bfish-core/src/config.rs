use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::prompts;

pub const DEFAULT_MCP_PORT: u16 = 34871;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl ProviderConfig {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            api_key: String::new(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    pub fn is_usable(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub default_base_url: &'static str,
    pub default_model: &'static str,
    pub key_url: &'static str,
    pub requires_key: bool,
    pub models: &'static [&'static str],
    pub note: &'static str,
}

pub fn provider_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            id: "openai",
            label: "OpenAI",
            default_base_url: "https://api.openai.com/v1",
            default_model: "gpt-4.1",
            key_url: "https://platform.openai.com/api-keys",
            requires_key: true,
            models: &["gpt-4.1", "gpt-4.1-mini", "o3", "o4-mini", "gpt-4o"],
            note: "Official OpenAI Chat Completions API.",
        },
        ProviderPreset {
            id: "anthropic",
            label: "Claude",
            default_base_url: "https://api.anthropic.com/v1",
            default_model: "claude-sonnet-4-5",
            key_url: "https://console.anthropic.com/settings/keys",
            requires_key: true,
            models: &[
                "claude-sonnet-4-5",
                "claude-opus-4-1",
                "claude-haiku-4-5",
                "claude-3-7-sonnet-latest",
            ],
            note: "Anthropic Messages API with native tool use.",
        },
        ProviderPreset {
            id: "gemini",
            label: "Gemini",
            default_base_url: "https://generativelanguage.googleapis.com/v1beta",
            default_model: "gemini-2.5-pro",
            key_url: "https://aistudio.google.com/app/apikey",
            requires_key: true,
            models: &["gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.0-flash"],
            note: "Google AI Studio keys. Streaming function calls supported.",
        },
        ProviderPreset {
            id: "deepseek",
            label: "DeepSeek",
            default_base_url: "https://api.deepseek.com/v1",
            default_model: "deepseek-chat",
            key_url: "https://platform.deepseek.com/api_keys",
            requires_key: true,
            models: &["deepseek-chat", "deepseek-reasoner"],
            note: "Official DeepSeek API. Use deepseek-chat for tools; deepseek-reasoner streams separate reasoning.",
        },
        ProviderPreset {
            id: "xai",
            label: "xAI",
            default_base_url: "https://api.x.ai/v1",
            default_model: "grok-4",
            key_url: "https://console.x.ai",
            requires_key: true,
            models: &["grok-4", "grok-3", "grok-3-mini"],
            note: "xAI's OpenAI-compatible endpoint.",
        },
        ProviderPreset {
            id: "openrouter",
            label: "OpenRouter",
            default_base_url: "https://openrouter.ai/api/v1",
            default_model: "anthropic/claude-sonnet-4.5",
            key_url: "https://openrouter.ai/keys",
            requires_key: true,
            models: &[
                "anthropic/claude-sonnet-4.5",
                "openai/gpt-4.1",
                "google/gemini-2.5-pro",
                "x-ai/grok-4",
                "deepseek/deepseek-chat",
            ],
            note: "One key for hundreds of models. BFish sends attribution headers.",
        },
        ProviderPreset {
            id: "ollama",
            label: "Ollama",
            default_base_url: "http://127.0.0.1:11434/v1",
            default_model: "qwen3:8b",
            key_url: "https://ollama.com/download",
            requires_key: false,
            models: &["qwen3:8b", "llama3.1:8b", "deepseek-r1:8b", "gemma3:4b"],
            note: "Local models. Start Ollama before chatting; no API key needed.",
        },
    ]
}

pub fn preset(id: &str) -> Option<ProviderPreset> {
    provider_presets().into_iter().find(|p| p.id == id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub system_prompt: String,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub mcp_port: u16,
    pub auto_install_plugin: bool,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let providers = provider_presets()
            .into_iter()
            .map(|p| {
                (
                    p.id.to_string(),
                    ProviderConfig::new(p.default_base_url, p.default_model),
                )
            })
            .collect::<BTreeMap<_, _>>();

        Self {
            provider: "openai".to_string(),
            model: "gpt-4.1".to_string(),
            temperature: 0.4,
            max_tokens: 8192,
            system_prompt: prompts::default_prompt().to_string(),
            providers,
            mcp_port: DEFAULT_MCP_PORT,
            auto_install_plugin: false,
            onboarded: false,
        }
    }
}

impl Settings {
    pub fn provider_config(&self) -> ProviderConfig {
        self.providers
            .get(&self.provider)
            .cloned()
            .unwrap_or_else(|| ProviderConfig::new("", ""))
    }

    /// The model shown in the UI. Falls back to the provider's saved model.
    pub fn effective_model(&self) -> String {
        if self.model.trim().is_empty() {
            self.provider_config().model
        } else {
            self.model.clone()
        }
    }

    /// Fill in anything a hand-edited or older settings file omitted.
    fn normalize(&mut self) {
        if self.temperature.is_nan() || !(0.0..=2.0).contains(&self.temperature) {
            self.temperature = 0.4;
        }
        if self.max_tokens < 256 {
            self.max_tokens = 8192;
        }
        if self.system_prompt.trim().is_empty() {
            self.system_prompt = prompts::default_prompt().to_string();
        }
        if self.mcp_port == 0 {
            self.mcp_port = DEFAULT_MCP_PORT;
        }
        for p in provider_presets() {
            let entry = self.providers.entry(p.id.to_string()).or_default();
            if entry.base_url.trim().is_empty() {
                entry.base_url = p.default_base_url.to_string();
            }
            if entry.model.trim().is_empty() {
                entry.model = p.default_model.to_string();
            }
        }
        if !self.providers.contains_key(&self.provider) {
            self.provider = "openai".to_string();
        }
        if self.model.trim().is_empty() {
            self.model = self
                .providers
                .get(&self.provider)
                .map(|p| p.model.clone())
                .unwrap_or_else(|| "gpt-4.1".to_string());
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub dir: PathBuf,
}

impl SettingsStore {
    pub fn discover() -> Self {
        Self { dir: bfish_home() }
    }

    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.dir.join("sessions")
    }

    pub fn load(&self) -> Settings {
        let mut settings = fs::read_to_string(self.path())
            .ok()
            .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
            .unwrap_or_default();
        settings.normalize();
        settings
    }

    pub fn save(&self, settings: &Settings) -> Result<()> {
        let mut settings = settings.clone();
        settings.normalize();
        if let Some(parent) = self.path().parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }
        write_private(&self.path(), &serde_json::to_vec_pretty(&settings)?)?;
        Ok(())
    }
}

pub fn bfish_home() -> PathBuf {
    if let Ok(custom) = std::env::var("BFISH_HOME") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BFish")
}

pub(crate) fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    if path.exists() {
        // Windows rename does not replace an existing file.
        fs::remove_file(path).ok();
    }
    fs::rename(&tmp, path).with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod config_tests;
