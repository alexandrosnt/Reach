//! Turso Platform API client for creating databases and tokens.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::vault::error::VaultError;
use crate::vault::types::TursoDbInfo;

const TURSO_API_BASE: &str = "https://api.turso.tech/v1";

/// Create a new database in Turso.
pub async fn create_database(
    org: &str,
    api_token: &str,
    db_name: &str,
    group: &str,
) -> Result<TursoDbInfo, VaultError> {
    let client = Client::new();
    let url = format!("{}/organizations/{}/databases", TURSO_API_BASE, org);

    #[derive(Serialize)]
    struct CreateDbRequest<'a> {
        name: &'a str,
        group: &'a str,
    }

    #[derive(Deserialize)]
    struct CreateDbResponse {
        database: DatabaseInfo,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct DatabaseInfo {
        db_id: String,
        hostname: String,
        name: String,
    }

    let response = client
        .post(&url)
        .bearer_auth(api_token)
        .json(&CreateDbRequest { name: db_name, group })
        .send()
        .await
        .map_err(|e| VaultError::SyncError(format!("Failed to create Turso database: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(VaultError::SyncError(format!(
            "Turso API error ({}): {}",
            status, body
        )));
    }

    let result: CreateDbResponse = response
        .json()
        .await
        .map_err(|e| VaultError::SyncError(format!("Failed to parse Turso response: {}", e)))?;

    Ok(TursoDbInfo {
        db_id: result.database.db_id,
        hostname: result.database.hostname,
        name: result.database.name,
    })
}

/// Create an auth token for a Turso database.
pub async fn create_database_token(
    org: &str,
    api_token: &str,
    db_name: &str,
) -> Result<String, VaultError> {
    let client = Client::new();
    let url = format!(
        "{}/organizations/{}/databases/{}/auth/tokens",
        TURSO_API_BASE, org, db_name
    );

    #[derive(Deserialize)]
    struct TokenResponse {
        jwt: String,
    }

    let response = client
        .post(&url)
        .bearer_auth(api_token)
        .send()
        .await
        .map_err(|e| VaultError::SyncError(format!("Failed to create Turso token: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(VaultError::SyncError(format!(
            "Turso API error ({}): {}",
            status, body
        )));
    }

    let result: TokenResponse = response
        .json()
        .await
        .map_err(|e| VaultError::SyncError(format!("Failed to parse Turso token response: {}", e)))?;

    Ok(result.jwt)
}
