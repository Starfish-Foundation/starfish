use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::write_private;
use crate::models::ChatMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

impl From<&Session> for SessionSummary {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.clone(),
            title: s.title.clone(),
            provider: s.provider.clone(),
            model: s.model.clone(),
            created_at: s.created_at,
            updated_at: s.updated_at,
            message_count: s.messages.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("invalid session id");
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    pub fn create(&self, provider: &str, model: &str) -> Result<Session> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating session directory {}", self.dir.display()))?;
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4().simple().to_string(),
            title: "New chat".to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        };
        self.save(&session)?;
        Ok(session)
    }

    pub fn list(&self) -> Vec<SessionSummary> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|e| fs::read_to_string(e.path()).ok())
            .filter_map(|raw| serde_json::from_str::<Session>(&raw).ok())
            .map(|s| SessionSummary::from(&s))
            .collect::<Vec<_>>();
        out.sort_by_key(|a| std::cmp::Reverse(a.updated_at));
        out
    }

    pub fn load(&self, id: &str) -> Result<Session> {
        let path = self.path(id)?;
        let raw = fs::read_to_string(&path).with_context(|| format!("session {id} not found"))?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path(&session.id)?;
        write_private(&path, &serde_json::to_vec_pretty(session)?)?;
        Ok(())
    }

    pub fn update<F>(&self, id: &str, f: F) -> Result<Session>
    where
        F: FnOnce(&mut Session),
    {
        let mut session = self.load(id)?;
        f(&mut session);
        session.updated_at = Utc::now();
        self.save(&session)?;
        Ok(session)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.path(id)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod store_tests;
