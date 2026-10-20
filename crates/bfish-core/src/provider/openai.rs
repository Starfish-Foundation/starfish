use anyhow::{bail, Result};
use futures_util::stream::BoxStream;
use serde_json::{json, Value};

use super::{
    absorb_openai_tool_calls, http_client, openai_messages, openai_tools, openai_usage,
    provider_label, require_key, truncate_for_error, PartialToolCall,
};
use crate::config::ProviderConfig;
use crate::models::{ChatRequest, StreamEvent, Usage};

pub async fn stream(
    kind: &str,
    cfg: &ProviderConfig,
    req: ChatRequest,
) -> Result<BoxStream<'static, Result<StreamEvent>>> {
    require_key(kind, cfg)?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");
    let label = provider_label(kind);

    let reasoning_model = (kind == "openai"
        && (req.model.starts_with("o1")
            || req.model.starts_with("o3")
            || req.model.starts_with("o4")
            || req.model.starts_with("gpt-5")))
        || (kind == "deepseek" && req.model.starts_with("deepseek-reasoner"));

    let mut body = json!({
        "model": req.model,
        "messages": openai_messages(&req.messages),
        "stream": true,
    });

    if !reasoning_model {
        body["temperature"] = json!(req.temperature);
    }
    if kind == "openai" {
        if reasoning_model {
            body["max_completion_tokens"] = json!(req.max_tokens);
        } else {
            body["max_tokens"] = json!(req.max_tokens);
        }
    } else {
        body["max_tokens"] = json!(req.max_tokens);
    }
    if !req.tools.is_empty() {
        body["tools"] = openai_tools(&req.tools);
        body["tool_choice"] = json!("auto");
    }

    let mut builder = http_client().post(&url).json(&body);
    if !cfg.api_key.trim().is_empty() {
        builder = builder.bearer_auth(cfg.api_key.trim());
    }
    if kind == "openrouter" {
        builder = builder
            .header("HTTP-Referer", "https://github.com/bfish-tools/bfish")
            .header("X-Title", "BFish");
    }

    let resp = builder.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!(
            "{label} API error ({status}): {}",
            truncate_for_error(&text)
        );
    }

    let mut events = Box::pin(super::sse::sse_data(resp));
    let stream = async_stream::stream! {
        use futures_util::StreamExt;
        let mut calls: std::collections::BTreeMap<usize, PartialToolCall> = Default::default();
        let mut usage = Usage::default();

        while let Some(item) = events.next().await {
            let data = match item {
                Ok(data) => data,
                Err(err) => {
                    yield Err(err);
                    return;
                }
            };
            if data.trim() == "[DONE]" {
                break;
            }
            let value: Value = match serde_json::from_str(&data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(err) = value.get("error") {
                let message = err["message"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| err.to_string());
                yield Err(anyhow::anyhow!("{label} API error: {message}"));
                return;
            }
            if value.get("usage").is_some() {
                usage = openai_usage(&value["usage"]);
            }
            let Some(choice) = value["choices"].get(0) else {
                continue;
            };
            let delta = &choice["delta"];
            if let Some(text) = delta["content"].as_str() {
                if !text.is_empty() {
                    yield Ok(StreamEvent::Text(text.to_string()));
                }
            }
            if delta.get("tool_calls").is_some() {
                absorb_openai_tool_calls(&mut calls, &delta["tool_calls"]);
            }
        }

        for (index, call) in std::mem::take(&mut calls) {
            yield Ok(StreamEvent::ToolCall(call.finish(index)));
        }
        yield Ok(StreamEvent::Done(usage));
    };

    Ok(Box::pin(stream))
}

pub async fn list_models(kind: &str, cfg: &ProviderConfig) -> Result<Vec<String>> {
    require_key(kind, cfg)?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/models");
    let mut builder = http_client().get(&url);
    if !cfg.api_key.trim().is_empty() {
        builder = builder.bearer_auth(cfg.api_key.trim());
    }
    if kind == "openrouter" {
        builder = builder
            .header("HTTP-Referer", "https://github.com/bfish-tools/bfish")
            .header("X-Title", "BFish");
    }
    let resp = builder.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!(
            "{} API error ({status}): {}",
            provider_label(kind),
            truncate_for_error(&text)
        );
    }
    let value: Value = resp.json().await?;
    let mut models = value["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item["id"]
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| item["name"].as_str().map(str::to_string))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    models.sort_by_key(|m| m.to_lowercase());
    models.dedup();
    Ok(models)
}
