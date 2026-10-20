use anyhow::{bail, Result};
use futures_util::stream::BoxStream;
use serde_json::{json, Value};

use crate::config::{preset, ProviderConfig};
use crate::models::{ChatMessage, ChatRequest, Role, StreamEvent, ToolCall, Usage};

pub mod anthropic;
pub mod gemini;
mod openai;
mod sse;

pub use sse::truncate_for_error;

/// OpenAI-compatible vendors. OpenRouter, xAI and Ollama all speak this dialect.
pub fn is_openai_compatible(kind: &str) -> bool {
    matches!(kind, "openai" | "xai" | "openrouter" | "ollama")
}

pub fn provider_label(kind: &str) -> String {
    preset(kind)
        .map(|p| p.label.to_string())
        .unwrap_or_else(|| kind.to_string())
}

pub async fn stream_chat(
    kind: &str,
    cfg: &ProviderConfig,
    req: ChatRequest,
) -> Result<BoxStream<'static, Result<StreamEvent>>> {
    match kind {
        "anthropic" => anthropic::stream(cfg, req).await,
        "gemini" => gemini::stream(cfg, req).await,
        k if is_openai_compatible(k) => openai::stream(k, cfg, req).await,
        other => bail!("unknown provider: {other}"),
    }
}

pub async fn list_models(kind: &str, cfg: &ProviderConfig) -> Result<Vec<String>> {
    match kind {
        "anthropic" => anthropic::list_models(cfg).await,
        "gemini" => gemini::list_models(cfg).await,
        k if is_openai_compatible(k) => openai::list_models(k, cfg).await,
        other => bail!("unknown provider: {other}"),
    }
}

pub(crate) fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("BFish/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("building HTTP client")
    })
}

pub(crate) fn require_key(kind: &str, cfg: &ProviderConfig) -> Result<()> {
    if cfg.api_key.trim().is_empty() && !matches!(kind, "ollama") {
        let p = preset(kind);
        bail!(
            "{} API key is missing. Add it in BFish Settings → Providers ({}).",
            provider_label(kind),
            p.map(|p| p.key_url).unwrap_or("")
        );
    }
    Ok(())
}

/// Convert common messages into the OpenAI chat-completions shape.
pub(crate) fn openai_messages(messages: &[ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| match m.role {
            Role::User => json!({ "role": "user", "content": m.content }),
            Role::Assistant => {
                let mut value = json!({
                    "role": "assistant",
                    "content": if m.content.is_empty() { Value::Null } else { json!(m.content) }
                });
                if !m.tool_calls.is_empty() {
                    value["tool_calls"] = json!(m
                        .tool_calls
                        .iter()
                        .map(|tc| json!({
                            "id": tc.id,
                            "type": "function",
                            "function": { "name": tc.name, "arguments": tc.arguments }
                        }))
                        .collect::<Vec<_>>());
                }
                value
            }
            Role::Tool => json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                "content": m.content
            }),
        })
        .collect()
}

pub(crate) fn openai_tools(tools: &[crate::models::ToolSpec]) -> Value {
    json!(tools
        .iter()
        .map(|t| json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.input_schema
            }
        }))
        .collect::<Vec<_>>())
}

/// Shared parsing for an OpenAI `tool_calls` delta array.
pub(crate) fn absorb_openai_tool_calls(
    calls: &mut std::collections::BTreeMap<usize, PartialToolCall>,
    value: &Value,
) {
    let Some(items) = value.as_array() else {
        return;
    };
    for item in items {
        let index = item["index"].as_u64().unwrap_or(0) as usize;
        let entry = calls.entry(index).or_default();
        if let Some(id) = item["id"].as_str() {
            if !id.is_empty() {
                entry.id = id.to_string();
            }
        }
        if let Some(name) = item["function"]["name"].as_str() {
            if entry.name.is_empty() {
                entry.name = name.to_string();
            }
        }
        if let Some(args) = item["function"]["arguments"].as_str() {
            entry.arguments.push_str(args);
        } else if item["function"]["arguments"].is_object() {
            entry.arguments = item["function"]["arguments"].to_string();
        }
    }
}

#[derive(Default)]
pub(crate) struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl PartialToolCall {
    pub fn finish(self, index: usize) -> ToolCall {
        ToolCall {
            id: if self.id.is_empty() {
                format!("call_{index}")
            } else {
                self.id
            },
            name: if self.name.is_empty() {
                "tool".to_string()
            } else {
                self.name
            },
            arguments: if self.arguments.trim().is_empty() {
                "{}".to_string()
            } else {
                self.arguments
            },
        }
    }
}

pub(crate) fn openai_usage(value: &Value) -> Usage {
    Usage {
        input_tokens: value["prompt_tokens"]
            .as_u64()
            .or_else(|| value["input_tokens"].as_u64())
            .unwrap_or(0),
        output_tokens: value["completion_tokens"]
            .as_u64()
            .or_else(|| value["output_tokens"].as_u64())
            .unwrap_or(0),
    }
}
