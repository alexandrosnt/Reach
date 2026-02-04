use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::state::SavedPlaybook;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaybookStore {
    pub playbooks: Vec<SavedPlaybook>,
}

pub fn storage_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("playbooks.json")
}

pub async fn load_playbooks(app_dir: &PathBuf) -> Result<PlaybookStore, StorageError> {
    let path = storage_path(app_dir);
    if !path.exists() {
        return Ok(PlaybookStore::default());
    }
    let data = tokio::fs::read_to_string(&path).await?;
    let store: PlaybookStore = serde_json::from_str(&data)?;
    tracing::info!("Loaded {} playbooks from disk", store.playbooks.len());
    Ok(store)
}

pub async fn save_playbooks(
    app_dir: &PathBuf,
    store: &PlaybookStore,
) -> Result<(), StorageError> {
    let path = storage_path(app_dir);
    let data = serde_json::to_string_pretty(store)?;
    tokio::fs::write(&path, data).await?;
    tracing::info!("Saved {} playbooks to disk", store.playbooks.len());
    Ok(())
}
