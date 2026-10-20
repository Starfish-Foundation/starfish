use std::sync::Arc;

use bfish_core::provider;
use bfish_core::{
    preset, ChatMessage, ChatRequest, Role, Session, SessionSummary, StreamEvent, Usage,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatBody {
    pub session_id: String,
    pub content: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

const MAX_ROUNDS: usize = 12;
const MAX_TOOL_RESULT_CHARS: usize = 80_000;

pub async fn run_chat(state: Arc<AppState>, body: ChatBody, tx: Sender<String>) {
    let session_id = body.session_id.clone();
    let (run_id, token) = state.begin_chat(&session_id).await;

    if let Err(err) = run_session(state.clone(), body, tx.clone(), token).await {
        let _ = tx
            .send(json!({ "type": "error", "message": format!("{err:#}") }).to_string())
            .await;
    }

    state.end_chat(&session_id, &run_id).await;
}

async fn run_session(
    state: Arc<AppState>,
    body: ChatBody,
    tx: Sender<String>,
    token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut session = state.sessions.load(&body.session_id)?;
    if let Some(provider) = body
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        session.provider = provider.to_string();
    }
    if let Some(model) = body
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        session.model = model.to_string();
    }

    let prompt = body.content.trim().to_string();
    if prompt.is_empty() {
        anyhow::bail!("Message is empty.");
    }

    session.messages.push(ChatMessage::user(prompt.clone()));
    if session.title == "New chat" || session.title.trim().is_empty() {
        session.title = summarize_title(&prompt);
    }
    state.sessions.save(&session)?;

    let settings = state.settings.read().await.clone();
    let Some(preset) = preset(&session.provider) else {
        anyhow::bail!(
            "Unknown provider `{}`. Choose one of the six providers in Settings.",
            session.provider
        );
    };

    let mut provider_config = settings
        .providers
        .get(&session.provider)
        .cloned()
        .unwrap_or_default();
    if provider_config.base_url.trim().is_empty() {
        provider_config.base_url = preset.default_base_url.to_string();
    }
    if session.model.trim().is_empty() {
        session.model = if provider_config.model.trim().is_empty() {
            preset.default_model.to_string()
        } else {
            provider_config.model.clone()
        };
    }
    state.sessions.save(&session)?;

    let mut system = settings.system_prompt.clone();
    system.push_str("\n\n# Live Studio context\n");
    match state.engine.bridge().connected_session().await {
        Some(studio) => {
            system.push_str(&format!(
                "Studio is connected.\nPlace: {} (PlaceId: {})\nPlugin: {}\nStudio: {}\n\
                 Inspect the place before editing and verify every mutation with a tool result.",
                studio.place_name, studio.place_id, studio.plugin_version, studio.studio_version
            ));
        }
        None => {
            system.push_str(
                "Studio is NOT connected. You cannot inspect or edit the place. Do not claim \
                 that Studio changed. Ask the user to open the BFish Connector plugin from \
                 Studio's Plugins tab, then retry.",
            );
        }
    }

    let tools = state.engine.tools();
    let mut total_usage = Usage::default();

    let _ = tx
        .send(
            json!({
                "type": "start",
                "sessionId": session.id,
                "title": session.title,
                "provider": session.provider,
                "model": session.model
            })
            .to_string(),
        )
        .await;

    for round in 0..MAX_ROUNDS {
        if tx.is_closed() {
            state.sessions.save(&session)?;
            return Ok(());
        }

        let request = ChatRequest {
            model: session.model.clone(),
            system: system.clone(),
            messages: session.messages.clone(),
            tools: tools.clone(),
            temperature: settings.temperature,
            max_tokens: settings.max_tokens,
        };

        let mut stream =
            match provider::stream_chat(&session.provider, &provider_config, request).await {
                Ok(stream) => stream,
                Err(err) => {
                    return Err(anyhow::anyhow!("{err:#}"));
                }
            };

        let mut text = String::new();
        let mut calls = Vec::new();
        let mut usage = Usage::default();

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    if !text.trim().is_empty() {
                        session.messages.push(ChatMessage::assistant(text));
                        state.sessions.save(&session)?;
                    }
                    let _ = tx.send(json!({"type":"cancelled"}).to_string()).await;
                    return Ok(());
                }
                _ = tx.closed() => {
                    state.sessions.save(&session)?;
                    return Ok(());
                }
                item = stream.next() => {
                    match item {
                        Some(Ok(StreamEvent::Text(chunk))) => {
                            if !chunk.is_empty() {
                                text.push_str(&chunk);
                                let _ = tx
                                    .send(json!({"type":"delta","text":chunk}).to_string())
                                    .await;
                            }
                        }
                        Some(Ok(StreamEvent::ToolCall(call))) => calls.push(call),
                        Some(Ok(StreamEvent::Done(report))) => usage = report,
                        Some(Err(err)) => return Err(anyhow::anyhow!("{err:#}")),
                        None => break,
                    }
                }
            }
        }

        total_usage.input_tokens += usage.input_tokens;
        total_usage.output_tokens += usage.output_tokens;

        if calls.is_empty() {
            if text.trim().is_empty() {
                anyhow::bail!(
                    "The model returned an empty response. Try again or pick another model."
                );
            }
            session.messages.push(ChatMessage::assistant(text));
            state.sessions.save(&session)?;
            send_done(&tx, &session, &total_usage).await;
            return Ok(());
        }

        let assistant_text = text.clone();
        session.messages.push(ChatMessage {
            role: Role::Assistant,
            content: assistant_text,
            tool_calls: calls.clone(),
            ..Default::default()
        });
        state.sessions.save(&session)?;

        for call in calls {
            let args = match serde_json::from_str::<Value>(&call.arguments) {
                Ok(value) if value.is_object() => value,
                _ => {
                    let message = format!(
                        "Invalid JSON arguments for `{}`: {}",
                        call.name, call.arguments
                    );
                    let _ = tx
                        .send(
                            json!({
                                "type":"tool_result",
                                "id": call.id,
                                "name": call.name,
                                "ok": false,
                                "error": message
                            })
                            .to_string(),
                        )
                        .await;
                    session.messages.push(ChatMessage::tool_result(
                        &call,
                        json!({"ok": false, "error": message}).to_string(),
                    ));
                    continue;
                }
            };

            let _ = tx
                .send(
                    json!({
                        "type": "tool_start",
                        "id": call.id,
                        "name": call.name,
                        "arguments": args
                    })
                    .to_string(),
                )
                .await;

            let outcome = state.engine.call(&call.name, &args).await;
            let (ok, content, preview) = match outcome {
                Ok(value) => {
                    let text =
                        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                    let clipped = clip(&text, MAX_TOOL_RESULT_CHARS);
                    let preview = if clipped.len() > 24_000 {
                        json!(clip(&clipped, 24_000))
                    } else {
                        serde_json::from_str(&clipped).unwrap_or_else(|_| json!(clipped))
                    };
                    (true, clipped, preview)
                }
                Err(err) => {
                    let value = json!({ "ok": false, "error": format!("{err:#}") });
                    (false, value.to_string(), value)
                }
            };

            let _ = tx
                .send(
                    json!({
                        "type": "tool_result",
                        "id": call.id,
                        "name": call.name,
                        "ok": ok,
                        "result": preview
                    })
                    .to_string(),
                )
                .await;

            session
                .messages
                .push(ChatMessage::tool_result(&call, content));
        }

        state.sessions.save(&session)?;

        if round == MAX_ROUNDS - 1 {
            anyhow::bail!(
                "Stopped after {MAX_ROUNDS} tool rounds to avoid an accidental loop. The changes made so far are saved."
            );
        }
    }

    Ok(())
}

async fn send_done(tx: &Sender<String>, session: &Session, usage: &Usage) {
    let summary = SessionSummary::from(session);
    let _ = tx
        .send(
            json!({
                "type": "done",
                "session": summary,
                "usage": {
                    "inputTokens": usage.input_tokens,
                    "outputTokens": usage.output_tokens
                }
            })
            .to_string(),
        )
        .await;
}

fn summarize_title(prompt: &str) -> String {
    let one_line = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= 52 {
        one_line
    } else {
        let short: String = one_line.chars().take(49).collect();
        format!("{short}…")
    }
}

fn clip(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut out: String = value.chars().take(max_chars).collect();
        out.push_str("\n\n[BFish truncated this tool result because it was very large.]");
        out
    }
}
