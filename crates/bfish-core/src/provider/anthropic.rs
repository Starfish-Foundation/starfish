use std::collections::BTreeMap;

use anyhow::{bail, Result};
use futures_util::stream::BoxStream;
use serde_json::{json, Value};

use super::{http_client, require_key, truncate_for_error};
use crate::config::ProviderConfig;
use crate::models::{
    parse_arguments, ChatMessage, ChatRequest, Role, StreamEvent, ToolCall, Usage,
};

fn anthropic_messages(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();

    for message in messages {
        match message.role {
            Role::User => out.push(json!({
                "role": "user",
                "content": [{ "type": "text", "text": message.content }]
            })),
            Role::Assistant => {
                let mut blocks = Vec::new();
                if !message.content.is_empty() {
                    blocks.push(json!({ "type": "text", "text": message.content }));
                }
                for call in &message.tool_calls {
                    let input = parse_arguments(&call.arguments);
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": if input.is_object() { input } else { json!({}) }
                    }));
                }
                if blocks.is_empty() {
                    blocks.push(json!({ "type": "text", "text": "(no content)" }));
                }
                out.push(json!({ "role": "assistant", "content": blocks }));
            }
            Role::Tool => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id.clone().unwrap_or_default(),
                    "content": message.content
                });
                let append = out
                    .last()
                    .map(|last| {
                        last["role"] == "user"
                            && last["content"]
                                .as_array()
                                .map(|blocks| blocks.iter().any(|b| b["type"] == "tool_result"))
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if append {
                    if let Some(blocks) = out.last_mut().and_then(|m| m["content"].as_array_mut()) {
                        blocks.push(block);
                    }
                } else {
                    out.push(json!({ "role": "user", "content": [block] }));
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
    require_key("anthropic", cfg)?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/messages");

    let mut body = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "temperature": req.temperature,
        "stream": true,
        "messages": anthropic_messages(&req.messages),
    });
    if !req.system.trim().is_empty() {
        body["system"] = json!(req.system);
    }
    if !req.tools.is_empty() {
        body["tools"] = json!(req
            .tools
            .iter()
            .map(|t| json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema
            }))
            .collect::<Vec<_>>());
    }

    let resp = http_client()
        .post(&url)
        .header("x-api-key", cfg.api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!(
            "Anthropic API error ({status}): {}",
            truncate_for_error(&text)
        );
    }

    let mut events = Box::pin(super::sse::sse_data(resp));
    let stream = async_stream::stream! {
        use futures_util::StreamExt;
        let mut tools: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
        let mut usage = Usage::default();

        while let Some(item) = events.next().await {
            let data = match item {
                Ok(data) => data,
                Err(err) => {
                    yield Err(err);
                    return;
                }
            };
            let value: Value = match serde_json::from_str(&data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let event_type = value["type"].as_str().unwrap_or_default();
            match event_type {
                "error" => {
                    let message = value["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown Anthropic streaming error");
                    yield Err(anyhow::anyhow!("Anthropic API error: {message}"));
                    return;
                }
                "message_start" => {
                    usage.input_tokens = value["message"]["usage"]["input_tokens"]
                        .as_u64()
                        .unwrap_or(0);
                }
                "content_block_start" => {
                    let index = value["index"].as_u64().unwrap_or(0) as usize;
                    let block = &value["content_block"];
                    if block["type"] == "tool_use" {
                        tools.insert(
                            index,
                            (
                                block["id"].as_str().unwrap_or_default().to_string(),
                                block["name"].as_str().unwrap_or_default().to_string(),
                                String::new(),
                            ),
                        );
                    }
                }
                "content_block_delta" => {
                    let index = value["index"].as_u64().unwrap_or(0) as usize;
                    let delta = &value["delta"];
                    match delta["type"].as_str().unwrap_or_default() {
                        "text_delta" => {
                            if let Some(text) = delta["text"].as_str() {
                                if !text.is_empty() {
                                    yield Ok(StreamEvent::Text(text.to_string()));
                                }
                            }
                        }
                        "input_json_delta" => {
                            if let Some(partial) = delta["partial_json"].as_str() {
                                if let Some((_, _, args)) = tools.get_mut(&index) {
                                    args.push_str(partial);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                "content_block_stop" => {
                    let index = value["index"].as_u64().unwrap_or(0) as usize;
                    if let Some((id, name, args)) = tools.remove(&index) {
                        yield Ok(StreamEvent::ToolCall(ToolCall {
                            id,
                            name,
                            arguments: if args.trim().is_empty() { "{}".into() } else { args },
                        }));
                    }
                }
                "message_delta" => {
                    usage.output_tokens = value["usage"]["output_tokens"]
                        .as_u64()
                        .unwrap_or(usage.output_tokens);
                }
                "message_stop" => break,
                _ => {}
            }
        }

        yield Ok(StreamEvent::Done(usage));
    };

    Ok(Box::pin(stream))
}

pub async fn list_models(cfg: &ProviderConfig) -> Result<Vec<String>> {
    require_key("anthropic", cfg)?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/models?limit=100");
    let resp = http_client()
        .get(&url)
        .header("x-api-key", cfg.api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!(
            "Anthropic API error ({status}): {}",
            truncate_for_error(&text)
        );
    }
    let value: Value = resp.json().await?;
    let mut models = value["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    models.sort();
    Ok(models)
}
