use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use bfish_core::{SessionStore, Settings, SettingsStore};
use bfish_mcp::Engine;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct CancelEntry {
    pub run_id: String,
    pub token: CancellationToken,
}

pub struct AppState {
    pub settings_store: SettingsStore,
    pub settings: RwLock<Settings>,
    pub sessions: SessionStore,
    pub engine: Engine,
    pub mcp_error: RwLock<Option<String>>,
    pub cancels: Mutex<HashMap<String, CancelEntry>>,
}

impl AppState {
    pub async fn new() -> Result<Arc<Self>> {
        Self::from_store(SettingsStore::discover()).await
    }

    pub async fn from_store(settings_store: SettingsStore) -> Result<Arc<Self>> {
        let settings = settings_store.load();
        let sessions = SessionStore::new(settings_store.sessions_dir());
        let port = settings.mcp_port;
        let mut engine = Engine::new(port);

        let mcp_error = match engine.bridge().bind().await {
            Ok(listener) => {
                let bridge = engine.bridge().clone();
                let bridge_engine = engine.clone();
                tokio::spawn(async move {
                    if let Err(err) = bridge.serve_on(listener, bridge_engine).await {
                        tracing::error!("BFish bridge stopped: {err:#}");
                    }
                });
                None
            }
            Err(err) => {
                if bfish_mcp::existing_bridge(port).await {
                    tracing::warn!(
                        "Port {port} is owned by an existing BFish MCP server; using it as a remote bridge."
                    );
                    engine = Engine::remote(port);
                    None
                } else {
                    Some(format!("{err:#}"))
                }
            }
        };

        Ok(Arc::new(Self {
            settings_store,
            settings: RwLock::new(settings),
            sessions,
            engine,
            mcp_error: RwLock::new(mcp_error),
            cancels: Mutex::new(HashMap::new()),
        }))
    }

    pub async fn begin_chat(&self, session_id: &str) -> (String, CancellationToken) {
        let run_id = Uuid::new_v4().simple().to_string();
        let token = CancellationToken::new();
        let mut map = self.cancels.lock().await;
        if let Some(previous) = map.insert(
            session_id.to_string(),
            CancelEntry {
                run_id: run_id.clone(),
                token: token.clone(),
            },
        ) {
            previous.token.cancel();
        }
        (run_id, token)
    }

    pub async fn end_chat(&self, session_id: &str, run_id: &str) {
        let mut map = self.cancels.lock().await;
        let is_current = map
            .get(session_id)
            .map(|entry| entry.run_id == run_id)
            .unwrap_or(false);
        if is_current {
            map.remove(session_id);
        }
    }

    pub async fn stop_chat(&self, session_id: &str) -> bool {
        let map = self.cancels.lock().await;
        if let Some(entry) = map.get(session_id) {
            entry.token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn save_settings(&self, next: Settings) -> Result<Settings> {
        self.settings_store.save(&next)?;
        let normalized = self.settings_store.load();
        *self.settings.write().await = normalized.clone();
        Ok(normalized)
    }
}
