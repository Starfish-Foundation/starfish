use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use bfish_core::prompts;
use bfish_core::provider;
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::agent::{self, ChatBody};
use crate::state::AppState;

type ApiError = (StatusCode, Json<Value>);
type ApiResult = Result<Json<Value>, ApiError>;

fn bad(message: impl Into<String>) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": message.into() })),
    )
}

fn internal(err: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": err.to_string() })),
    )
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/status", get(status))
        .route("/api/settings", get(get_settings).put(update_settings))
        .route("/api/providers", get(providers))
        .route("/api/providers/test", post(test_provider))
        .route("/api/providers/models", get(provider_models))
        .route("/api/prompts", get(prompt_list))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/{id}",
            get(get_session).patch(patch_session).delete(delete_session),
        )
        .route("/api/chat", post(chat))
        .route("/api/chat/stop", post(stop_chat))
        .route("/api/mcp/status", get(mcp_status))
        .route("/api/mcp/install-plugin", post(install_studio_plugin))
        .layer(cors())
        .with_state(state)
}

fn cors() -> CorsLayer {
    let origins = [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:1420",
        "http://127.0.0.1:1420",
        "tauri://localhost",
        "http://tauri.localhost",
        "https://tauri.localhost",
    ]
    .into_iter()
    .filter_map(|origin| origin.parse::<HeaderValue>().ok())
    .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_private_network(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION, ACCEPT])
}

pub async fn serve(port: u16) -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let app = router(state);
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("BFish backend listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Json<Value> {
    Json(json!({
        "name": "BFish",
        "version": bfish_core::APP_VERSION,
        "description": "AI in Roblox Studio",
        "api": "/api",
        "mcp": {
            "endpoint": "/mcp",
            "port": bfish_mcp::DEFAULT_STUDIO_PORT
        }
    }))
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let status = state.engine.status().await;
    Json(json!({
        "ok": true,
        "version": bfish_core::APP_VERSION,
        "studioConnected": status.connected
    }))
}

async fn status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let bridge = state.engine.status().await;
    let error = state.mcp_error.read().await.clone();
    let settings = state.settings.read().await.clone();
    Json(json!({
        "version": bfish_core::APP_VERSION,
        "provider": settings.provider,
        "model": settings.effective_model(),
        "studio": bridge,
        "bridgeError": error
    }))
}

async fn get_settings(State(state): State<Arc<AppState>>) -> Json<Value> {
    let settings = state.settings.read().await.clone();
    Json(json!({ "settings": settings }))
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(next): Json<bfish_core::Settings>,
) -> ApiResult {
    if bfish_core::preset(&next.provider).is_none() {
        return Err(bad(format!("Unknown provider `{}`.", next.provider)));
    }
    let saved = state.save_settings(next).await.map_err(internal)?;
    Ok(Json(json!({ "ok": true, "settings": saved })))
}

async fn providers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let settings = state.settings.read().await.clone();
    Json(json!({
        "providers": bfish_core::provider_presets(),
        "saved": settings.providers
    }))
}

async fn prompt_list(State(state): State<Arc<AppState>>) -> Json<Value> {
    let settings = state.settings.read().await.clone();
    Json(json!({
        "default": prompts::default_prompt(),
        "presets": prompts::presets(),
        "current": settings.system_prompt
    }))
}

#[derive(Debug, Deserialize)]
struct ModelsQuery {
    provider: String,
}

async fn provider_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelsQuery>,
) -> ApiResult {
    let Some(preset) = bfish_core::preset(&query.provider) else {
        return Err(bad("Unknown provider."));
    };
    let settings = state.settings.read().await.clone();
    let mut cfg = settings
        .providers
        .get(&query.provider)
        .cloned()
        .unwrap_or_default();
    if cfg.base_url.trim().is_empty() {
        cfg.base_url = preset.default_base_url.to_string();
    }
    let models = provider::list_models(&query.provider, &cfg)
        .await
        .map_err(|error| bad(format!("{error:#}")))?;
    let count = models.len();
    Ok(Json(json!({
        "ok": true,
        "models": models,
        "count": count,
        "source": "provider"
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestProviderBody {
    provider: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    model: String,
}

async fn test_provider(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<TestProviderBody>,
) -> ApiResult {
    let Some(preset) = bfish_core::preset(&body.provider) else {
        return Err(bad("Unknown provider."));
    };
    let cfg = bfish_core::ProviderConfig {
        api_key: body.api_key,
        base_url: if body.base_url.trim().is_empty() {
            preset.default_base_url.to_string()
        } else {
            body.base_url
        },
        model: if body.model.trim().is_empty() {
            preset.default_model.to_string()
        } else {
            body.model
        },
    };
    let models = provider::list_models(&body.provider, &cfg)
        .await
        .map_err(|error| bad(format!("{error:#}")))?;
    let count = models.len();
    Ok(Json(json!({
        "ok": true,
        "models": models,
        "count": count,
        "source": "provider"
    })))
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "sessions": state.sessions.list() }))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CreateSession {
    provider: Option<String>,
    model: Option<String>,
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSession>,
) -> ApiResult {
    let settings = state.settings.read().await.clone();
    let provider = body
        .provider
        .filter(|p| bfish_core::preset(p).is_some())
        .unwrap_or_else(|| settings.provider.clone());
    let model = body
        .model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| settings.providers.get(&provider).map(|c| c.model.clone()))
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| settings.effective_model());

    let session = state.sessions.create(&provider, &model).map_err(internal)?;
    Ok(Json(json!({ "ok": true, "session": session })))
}

async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    let session = state
        .sessions
        .load(&id)
        .map_err(|error| bad(format!("{error:#}")))?;
    Ok(Json(json!({ "session": session })))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct PatchSession {
    title: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

async fn patch_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PatchSession>,
) -> ApiResult {
    if let Some(provider) = &body.provider {
        if bfish_core::preset(provider).is_none() {
            return Err(bad("Unknown provider."));
        }
    }
    let session = state
        .sessions
        .update(&id, |session| {
            if let Some(title) = body.title {
                let title = title.trim();
                if !title.is_empty() {
                    session.title = title.to_string();
                }
            }
            if let Some(provider) = body.provider {
                session.provider = provider;
            }
            if let Some(model) = body.model {
                if !model.trim().is_empty() {
                    session.model = model;
                }
            }
        })
        .map_err(|error| bad(format!("{error:#}")))?;
    Ok(Json(json!({ "ok": true, "session": session })))
}

async fn delete_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    state.stop_chat(&id).await;
    state.sessions.delete(&id).map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

async fn chat(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChatBody>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);
    tokio::spawn(async move {
        agent::run_chat(state, body, tx).await;
    });

    let stream = async_stream::stream! {
        while let Some(data) = rx.recv().await {
            yield Ok(Event::default().data(data));
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("bfish-keepalive"),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopChat {
    session_id: String,
}

async fn stop_chat(State(state): State<Arc<AppState>>, Json(body): Json<StopChat>) -> Json<Value> {
    let stopped = state.stop_chat(&body.session_id).await;
    Json(json!({ "ok": true, "stopped": stopped }))
}

async fn mcp_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let status = state.engine.status().await;
    let error = state.mcp_error.read().await.clone();
    let mut value = serde_json::to_value(status).unwrap_or_else(|_| json!({}));
    value["error"] = json!(error);
    value["mcpUrl"] = json!(format!("http://127.0.0.1:{}/mcp", state.engine.port()));
    Json(value)
}

async fn install_studio_plugin(State(_state): State<Arc<AppState>>) -> ApiResult {
    match tokio::task::spawn_blocking(bfish_mcp::install_plugin).await {
        Ok(Ok(path)) => Ok(Json(json!({
            "ok": true,
            "path": path.display().to_string()
        }))),
        Ok(Err(err)) => Err(bad(format!("{err:#}"))),
        Err(err) => Err(internal(format!("install task failed: {err}"))),
    }
}
