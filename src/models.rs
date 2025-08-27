// src/models.rs

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- User Models ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Guest,
    Google,
}
impl From<String> for Provider {
    fn from(s: String) -> Self {
        match s.as_str() {
            "google" => Provider::Google,
            _ => Provider::Guest,
        }
    }
}
impl From<Provider> for String {
    fn from(p: Provider) -> Self {
        match p {
            Provider::Google => "google".to_string(),
            Provider::Guest => "guest".to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct UserInfo {
    pub user_id: String,
    pub username: String,
    pub provider: Provider,
}

// --- User-related DB Functions ---

// Create a new guest user_id
pub async fn create_guest_account(
    sqlite_pool: &sqlx::SqlitePool,
) -> anyhow::Result<(String, String)> {
    let guest_id = Uuid::new_v4().to_string();
    let provider = "guest";
    let provider_id = guest_id.clone();
    let username = format!("Guest{}", &guest_id[..5]);
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M%z").to_string();

    sqlx::query!(
        r#"
        INSERT INTO users (id, provider, provider_id, username, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        guest_id,
        provider,
        provider_id,
        username,
        created_at,
    )
    .execute(sqlite_pool)
    .await?;

    tracing::info!(
        "Created guest account: {} with username {}",
        guest_id,
        username
    );

    Ok((guest_id, username))
}

// --- Request/Response DTOs ---

#[derive(Debug, Deserialize)]
pub struct PathRequest {
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub start_id: Option<u32>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub end_id: Option<u32>,
    #[serde(default)]
    pub output_as_ids: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PathNode {
    Title(String),
    Id(u32),
}

#[derive(Debug, Serialize)]
pub struct PathResponse {
    pub elapsed_s: f64,
    pub paths: Vec<Vec<PathNode>>,
    pub leaderboard_longest_rank: Option<u32>,
    pub leaderboard_most_rank: Option<u32>,
}

#[derive(Deserialize)]
pub struct ChangeUsernameRequest {
    pub username: String,
}
