use anyhow::{bail, Result};
use futures_util::stream::BoxStream;
use serde_json::{json, Value};

use super::{http_client, truncate_for_error};
use crate::config::ProviderConfig;
use crate::models::{
    parse_arguments, ChatMessage, ChatRequest, Role, StreamEvent, ToolCall, Usage,
};

fn gemini_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "additionalProperties" | "$schema" | "title" | "default" | "examples"
                    )
                })
                .map(|(key, item)| (key.clone(), gemini_schema(item)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(gemini_schema).collect()),
        other => other.clone(),
    }
}

fn gemini_contents(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();

    for message in messages {
        match message.role {
            Role::User => {
                if message.content.is_empty() {
                    continue;
                }
                out.push(json!({
                    "role": "user",
                    "parts": [{ "text": message.content }]
                }));
            }
            Role::Assistant => {
                let mut parts = Vec::new();
                if !message.content.is_empty() {
                    parts.push(json!({ "text": message.content }));
                }
                for call in &message.tool_calls {
                    let args = parse_arguments(&call.arguments);
                    parts.push(json!({
                        "functionCall": {
                            "name": call.name,
                            "args": if args.is_object() { args } else { json!({}) }
                        }
                    }));
                }
                if !parts.is_empty() {
                    out.push(json!({ "role": "model", "parts": parts }));
                }
            }
            Role::Tool => {
                let part = json!({
                    "functionResponse": {
                        "name": message.name.clone().unwrap_or_else(|| "tool".to_string()),
                        "response": { "result": message.content }
                    }
                });
                let append = out
                    .last()
                    .map(|last| {
                        last["role"] == "user"
                            && last["parts"]
                                .as_array()
                                .map(|parts| {
                                    parts.iter().any(|p| p.get("functionResponse").is_some())
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if append {
                    if let Some(parts) = out.last_mut().and_then(|m| m["parts"].as_array_mut()) {
                        parts.push(part);
                    }
                } else {
                    out.push(json!({ "role": "user", "parts": [part] }));
                }
            }
        }
    }

    out
}

pub async fn stream(
    cfg: &ProviderConfig,
    req: ChatRequest,
) -> Result<BoxStream<'static, Result<StreamEvent>>> {
    if cfg.api_key.trim().is_empty() {
        bail!("Gemini API key is missing. Add it in BFish Settings → Providers.");
    }
    let base = cfg.base_url.trim_end_matches('/');
    let model = req.model.trim().trim_start_matches("models/");
    let mut url = reqwest::Url::parse(&format!("{base}/models/{model}:streamGenerateContent"))?;
    url.query_pairs_mut()
        .append_pair("alt", "sse")
        .append_pair("key", cfg.api_key.trim());

    let mut body = json!({
        "contents": gemini_contents(&req.messages),
        "generationConfig": {
            "temperature": req.temperature,
            "maxOutputTokens": req.max_tokens
        }
    });
    if !req.system.trim().is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": req.system }] });
    }
    if !req.tools.is_empty() {
        body["tools"] = json!([{
            "functionDeclarations": req.tools.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
                "parameters": gemini_schema(&t.input_schema)
            })).collect::<Vec<_>>()
        }]);
    }

    let resp = http_client().post(url).json(&body).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Gemini API error ({status}): {}", truncate_for_error(&text));
    }

    let mut events = Box::pin(super::sse::sse_data(resp));
    let stream = async_stream::stream! {
        use futures_util::StreamExt;
        let mut usage = Usage::default();
        let mut call_index = 0usize;

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
                yield Err(anyhow::anyhow!("Gemini API error: {message}"));
                return;
            }
            if let Some(meta) = value.get("usageMetadata") {
                usage.input_tokens = meta["promptTokenCount"].as_u64().unwrap_or(usage.input_tokens);
                usage.output_tokens = meta["candidatesTokenCount"].as_u64().unwrap_or(usage.output_tokens);
            }
            let Some(parts) = value["candidates"][0]["content"]["parts"].as_array() else {
                continue;
            };
            for part in parts {
                if let Some(text) = part["text"].as_str() {
                    if !text.is_empty() && !part["thought"].as_bool().unwrap_or(false) {
                        yield Ok(StreamEvent::Text(text.to_string()));
                    }
                }
                if let Some(call) = part.get("functionCall") {
                    let name = call["name"].as_str().unwrap_or("tool").to_string();
                    let args = call.get("args").cloned().unwrap_or(json!({}));
                    call_index += 1;
                    yield Ok(StreamEvent::ToolCall(ToolCall {
                        id: format!("gemini_call_{call_index}"),
                        name,
                        arguments: serde_json::to_string(&args).unwrap_or_else(|_| "{}".into()),
                    }));
                }
            }
        }

        yield Ok(StreamEvent::Done(usage));
    };

    Ok(Box::pin(stream))
}

pub async fn list_models(cfg: &ProviderConfig) -> Result<Vec<String>> {
    if cfg.api_key.trim().is_empty() {
        bail!("Gemini API key is missing. Add it in BFish Settings → Providers.");
    }
    let base = cfg.base_url.trim_end_matches('/');
    let mut url = reqwest::Url::parse(&format!("{base}/models"))?;
    url.query_pairs_mut().append_pair("key", cfg.api_key.trim());
    let resp = http_client().get(url).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Gemini API error ({status}): {}", truncate_for_error(&text));
    }
    let value: Value = resp.json().await?;
    let mut models = value["models"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item["supportedGenerationMethods"]
                        .as_array()
                        .map(|methods| methods.iter().any(|m| m == "generateContent"))
                        .unwrap_or(true)
                })
                .filter_map(|item| item["name"].as_str())
                .map(|name| name.trim_start_matches("models/").to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    models.sort();
    models.dedup();
    Ok(models)
}
