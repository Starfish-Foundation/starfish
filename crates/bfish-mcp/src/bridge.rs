use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

pub const DEFAULT_STUDIO_PORT: u16 = 34871;
const SESSION_STALE_SECONDS: i64 = 30;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub plugin_version: String,
    pub place_name: String,
    pub place_id: u64,
    pub studio_version: String,
    pub role: String,
    pub connected_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioLog {
    pub at: DateTime<Utc>,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub connected: bool,
    pub port: u16,
    pub sessions: Vec<SessionInfo>,
    pub calls: u64,
    pub recent_logs: Vec<StudioLog>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub endpoint: String,
    pub data: Value,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HelloPayload {
    pub plugin_version: String,
    pub place_name: String,
    pub place_id: u64,
    pub studio_version: String,
    pub role: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultPayload {
    pub session: Option<String>,
    pub id: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPayload {
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct PollQuery {
    pub session: String,
    pub wait: Option<u64>,
}

#[derive(Debug)]
pub struct CommandOutcome {
    pub ok: bool,
    pub result: Value,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum PollOutcome {
    Command(Command),
    Timeout,
    UnknownSession,
}

struct QueueEntry {
    session_id: String,
    command: Command,
}

struct Inner {
    port: u16,
    state: Mutex<BridgeState>,
    calls: AtomicU64,
}

#[derive(Default)]
struct BridgeState {
    sessions: HashMap<String, SessionInfo>,
    queue: VecDeque<QueueEntry>,
    pending: HashMap<String, tokio::sync::oneshot::Sender<CommandOutcome>>,
    logs: VecDeque<StudioLog>,
}

#[derive(Clone)]
pub struct Bridge {
    inner: Arc<Inner>,
}

impl Bridge {
    pub fn new(port: u16) -> Self {
        Self {
            inner: Arc::new(Inner {
                port,
                state: Mutex::new(BridgeState::default()),
                calls: AtomicU64::new(0),
            }),
        }
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    /// Bind the local Studio bridge and MCP HTTP endpoint.
    pub async fn serve(&self, engine: crate::engine::Engine) -> Result<()> {
        let listener = self.bind().await?;
        self.serve_on(listener, engine).await
    }

    pub async fn bind(&self) -> Result<tokio::net::TcpListener> {
        let addr = format!("127.0.0.1:{}", self.inner.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .with_context(|| {
                format!(
                    "could not bind BFish bridge to {addr}. Another BFish instance may be running."
                )
            })?;
        tracing::info!(
            "BFish bridge listening on http://{}",
            listener.local_addr()?
        );
        Ok(listener)
    }

    pub async fn serve_on(
        &self,
        listener: tokio::net::TcpListener,
        engine: crate::engine::Engine,
    ) -> Result<()> {
        let router = Router::new()
            .route("/health", get(health))
            .route("/studio/hello", post(hello))
            .route("/studio/poll", get(poll))
            .route("/studio/result", post(studio_result))
            .route("/studio/log", post(studio_log))
            .route("/studio/status", get(studio_status))
            .route(
                "/mcp",
                post(crate::mcp::http_handler).get(crate::mcp::http_info),
            )
            .with_state(engine);

        axum::serve(listener, router).await?;
        Ok(())
    }

    pub async fn hello(&self, payload: HelloPayload) -> String {
        let id = Uuid::new_v4().simple().to_string();
        let now = Utc::now();
        let session = SessionInfo {
            id: id.clone(),
            plugin_version: if payload.plugin_version.is_empty() {
                "unknown".to_string()
            } else {
                payload.plugin_version
            },
            place_name: if payload.place_name.is_empty() {
                "Untitled place".to_string()
            } else {
                payload.place_name
            },
            place_id: payload.place_id,
            studio_version: payload.studio_version,
            role: if payload.role.is_empty() {
                "edit".to_string()
            } else {
                payload.role
            },
            connected_at: now,
            last_seen: now,
        };
        let mut state = self.inner.state.lock().await;
        // Keep at most four recent Studio windows; drop stale ones and their
        // queued work so a restarted Studio cannot receive old commands.
        state.sessions.insert(id.clone(), session);
        if state.sessions.len() > 4 {
            let mut entries = state
                .sessions
                .iter()
                .map(|(key, s)| (key.clone(), s.last_seen))
                .collect::<Vec<_>>();
            entries.sort_by_key(|(_, seen)| *seen);
            for (key, _) in entries.into_iter().take(state.sessions.len() - 4) {
                state.sessions.remove(&key);
                state.queue.retain(|entry| entry.session_id != key);
            }
        }
        let live = state
            .sessions
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        state.queue.retain(|entry| live.contains(&entry.session_id));
        id
    }

    pub async fn poll(&self, session_id: &str, wait_secs: u64) -> PollOutcome {
        let wait = wait_secs.clamp(1, 25);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(wait);

        loop {
            {
                let mut state = self.inner.state.lock().await;
                let Some(session) = state.sessions.get_mut(session_id) else {
                    return PollOutcome::UnknownSession;
                };
                session.last_seen = Utc::now();

                if let Some(index) = state.queue.iter().position(|e| e.session_id == session_id) {
                    if let Some(entry) = state.queue.remove(index) {
                        return PollOutcome::Command(entry.command);
                    }
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return PollOutcome::Timeout;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Queue a command for the most recently active Studio session and wait for
    /// the plugin to report back.
    pub async fn call(&self, endpoint: &str, data: Value, timeout: Duration) -> Result<Value> {
        let (session_id, place) = {
            let state = self.inner.state.lock().await;
            let Some(session) = state.sessions.values().max_by_key(|s| s.last_seen) else {
                bail!(
                    "Roblox Studio is not connected. Open Studio, then open the BFish Connector plugin from the Plugins tab."
                );
            };
            let age = Utc::now()
                .signed_duration_since(session.last_seen)
                .num_seconds();
            if age > SESSION_STALE_SECONDS {
                bail!(
                    "Roblox Studio stopped responding {}s ago. Click Reconnect in the BFish Connector plugin.",
                    age
                );
            }
            (session.id.clone(), session.place_name.clone())
        };

        let command = Command {
            id: Uuid::new_v4().simple().to_string(),
            endpoint: endpoint.to_string(),
            data,
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut state = self.inner.state.lock().await;
            state.pending.insert(command.id.clone(), tx);
            state.queue.push_back(QueueEntry {
                session_id: session_id.clone(),
                command: command.clone(),
            });
        }
        self.inner.calls.fetch_add(1, Ordering::Relaxed);

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(outcome)) => {
                if outcome.ok {
                    Ok(outcome.result)
                } else {
                    bail!(
                        "{}",
                        outcome
                            .error
                            .unwrap_or_else(|| "Studio reported an unknown error".to_string())
                    )
                }
            }
            Ok(Err(_)) => bail!("Studio connection closed while running `{endpoint}` in {place}."),
            Err(_) => {
                let mut state = self.inner.state.lock().await;
                state.pending.remove(&command.id);
                state.queue.retain(|entry| entry.command.id != command.id);
                bail!(
                    "Timed out after {}s waiting for Studio (`{endpoint}`). The operation may still be running; check the place before retrying.",
                    timeout.as_secs()
                )
            }
        }
    }

    async fn complete(&self, payload: ResultPayload) {
        let mut state = self.inner.state.lock().await;
        if let Some(sender) = state.pending.remove(&payload.id) {
            let _ = sender.send(CommandOutcome {
                ok: payload.ok,
                result: payload.result,
                error: payload.error,
            });
        }
    }

    pub async fn log(&self, payload: LogPayload) {
        if payload.message.trim().is_empty() {
            return;
        }
        let mut state = self.inner.state.lock().await;
        state.logs.push_back(StudioLog {
            at: Utc::now(),
            level: if payload.level.is_empty() {
                "info".to_string()
            } else {
                payload.level
            },
            message: payload.message,
        });
        while state.logs.len() > 200 {
            state.logs.pop_front();
        }
    }

    pub async fn status(&self) -> BridgeStatus {
        let state = self.inner.state.lock().await;
        let now = Utc::now();
        let connected = state
            .sessions
            .values()
            .any(|s| now.signed_duration_since(s.last_seen).num_seconds() <= 12);
        let mut sessions = state.sessions.values().cloned().collect::<Vec<_>>();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_seen));
        let recent_logs = state.logs.iter().rev().take(20).cloned().collect();
        BridgeStatus {
            connected,
            port: self.inner.port,
            sessions,
            calls: self.inner.calls.load(Ordering::Relaxed),
            recent_logs,
        }
    }

    pub async fn connected_session(&self) -> Option<SessionInfo> {
        let state = self.inner.state.lock().await;
        let now = Utc::now();
        state
            .sessions
            .values()
            .filter(|s| {
                now.signed_duration_since(s.last_seen).num_seconds() <= SESSION_STALE_SECONDS
            })
            .max_by_key(|s| s.last_seen)
            .cloned()
    }
}

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "bfish-mcp",
        "version": bfish_core::APP_VERSION
    }))
}

async fn hello(
    State(engine): State<crate::engine::Engine>,
    Json(payload): Json<HelloPayload>,
) -> Json<Value> {
    let session_id = engine.bridge().hello(payload).await;
    Json(json!({
        "ok": true,
        "sessionId": session_id,
        "serverVersion": bfish_core::APP_VERSION
    }))
}

async fn poll(
    State(engine): State<crate::engine::Engine>,
    Query(query): Query<PollQuery>,
) -> Response {
    match engine
        .bridge()
        .poll(&query.session, query.wait.unwrap_or(25))
        .await
    {
        PollOutcome::Command(command) => Json(command).into_response(),
        PollOutcome::Timeout => StatusCode::NO_CONTENT.into_response(),
        PollOutcome::UnknownSession => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "unknown_session",
                "message": "The BFish server restarted. Reconnect the plugin."
            })),
        )
            .into_response(),
    }
}

async fn studio_result(
    State(engine): State<crate::engine::Engine>,
    Json(payload): Json<ResultPayload>,
) -> Json<Value> {
    engine.bridge().complete(payload).await;
    Json(json!({ "ok": true }))
}

async fn studio_log(
    State(engine): State<crate::engine::Engine>,
    Json(payload): Json<LogPayload>,
) -> Json<Value> {
    engine.bridge().log(payload).await;
    Json(json!({ "ok": true }))
}

async fn studio_status(State(engine): State<crate::engine::Engine>) -> Json<BridgeStatus> {
    Json(engine.bridge().status().await)
}
