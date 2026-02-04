use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::state::{Folder, SessionConfig};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Storage file not found")]
    NotFound,
}

/// Wrapper around the on-disk session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStore {
    pub sessions: Vec<SessionConfig>,
    pub folders: Vec<Folder>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            folders: Vec::new(),
        }
    }
}

/// Resolve the path where session data is persisted.
pub fn storage_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("sessions.json")
}

/// Load session data from the encrypted JSON file.
pub async fn load_sessions(app_dir: &PathBuf) -> Result<SessionStore, StorageError> {
    let path = storage_path(app_dir);
    if !path.exists() {
        tracing::info!("No session file found, returning defaults");
        return Ok(SessionStore::default());
    }
    let data = tokio::fs::read_to_string(&path).await?;
    let store: SessionStore = serde_json::from_str(&data)?;
    tracing::info!("Loaded {} sessions from disk", store.sessions.len());
    Ok(store)
}

/// Save session data to the encrypted JSON file.
pub async fn save_sessions(
    app_dir: &PathBuf,
    store: &SessionStore,
) -> Result<(), StorageError> {
    let path = storage_path(app_dir);
    let data = serde_json::to_string_pretty(store)?;
    tokio::fs::write(&path, data).await?;
    tracing::info!("Saved {} sessions to disk", store.sessions.len());
    Ok(())
}
