use mongodb::bson::doc;
use mongodb::options::{ClientOptions, ReplaceOptions};
use mongodb::{Client, Collection};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::source::TrackMetadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPlaylist {
    pub user_id: u64,
    pub name: String,
    pub tracks: Vec<TrackMetadata>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LocalPlaylistStorage {
    playlists: Vec<UserPlaylist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildPlaybackHistory {
    pub guild_id: u64,
    pub tracks: Vec<TrackMetadata>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LocalHistoryStorage {
    histories: Vec<GuildPlaybackHistory>,
}

#[derive(Clone)]
pub struct PlaylistStore {
    mongo_client: Option<Client>,
    database_name: String,
    local_file: PathBuf,
    local_history_file: PathBuf,
    local_lock: Arc<Mutex<()>>,
}

impl PlaylistStore {
    pub async fn init() -> Self {
        let db_name = std::env::var("MONGO_DATABASE")
            .unwrap_or_else(|_| "discord_music_bot".to_string());
        let local_file = PathBuf::from("data/playlists.json");
        let local_history_file = PathBuf::from("data/history.json");

        let mongo_uri = Self::resolve_mongo_uri();
        let mut mongo_client = None;

        if let Some(uri) = mongo_uri {
            info!("Attempting to connect to MongoDB Atlas...");
            match ClientOptions::parse(&uri).await {
                Ok(mut client_options) => {
                    // Resource optimization: cap connection pool for lightweight bot
                    client_options.max_pool_size = Some(3);
                    client_options.min_pool_size = Some(0);
                    client_options.server_selection_timeout = Some(std::time::Duration::from_secs(5));
                    client_options.connect_timeout = Some(std::time::Duration::from_secs(10));

                    match Client::with_options(client_options) {
                        Ok(client) => {
                            let ping_doc = doc! { "ping": 1 };
                            match client.database("admin").run_command(ping_doc).await {
                                Ok(_) => {
                                    info!(
                                        "Connected to MongoDB Atlas successfully! (Database: {})",
                                        db_name
                                    );
                                    mongo_client = Some(client);
                                }
                                Err(e) => {
                                    warn!(
                                        "Could not ping MongoDB Atlas: {}. Falling back to local storage ({}).",
                                        e,
                                        local_file.display()
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to build MongoDB client: {}. Falling back to local storage.", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Invalid MongoDB connection string: {}. Falling back to local storage.", e);
                }
            }
        } else {
            info!(
                "MongoDB credentials not fully configured in .env. Using local storage ({}).",
                local_file.display()
            );
        }

        Self {
            mongo_client,
            database_name: db_name,
            local_file,
            local_history_file,
            local_lock: Arc::new(Mutex::new(())),
        }
    }
    /// Returns a short human-readable status string for diagnostics.
    /// "MongoDB Atlas (db=xxx)" or "Local File Storage (data/playlists.json)"
    pub fn status(&self) -> String {
        if self.mongo_client.is_some() {
            format!("MongoDB Atlas (db={})", self.database_name)
        } else {
            format!("Local File Storage ({})", self.local_file.display())
        }
    }

 fn resolve_mongo_uri() -> Option<String> {
        if let Ok(uri) = std::env::var("MONGODB_URI") {
            let trimmed = uri.trim();
            if !trimmed.is_empty() && trimmed.starts_with("mongodb") {
                return Some(trimmed.to_string());
            }
        }

        let user = std::env::var("MONGO_USER").ok()?;
        let password = std::env::var("MONGO_PASSWORD").ok()?;
        let host = std::env::var("MONGO_HOST").ok()?;
        let app_name = std::env::var("MONGO_APP_NAME").unwrap_or_else(|_| "Cluster0".to_string());

        let user = user.trim();
        let password = password.trim();
        let host = host.trim();

        if user.is_empty() || password.is_empty() || host.is_empty() || password == "<db_password>" {
            return None;
        }

        let enc_user = utf8_percent_encode(user, NON_ALPHANUMERIC).to_string();
        let enc_pass = utf8_percent_encode(password, NON_ALPHANUMERIC).to_string();

        Some(format!(
            "mongodb+srv://{}:{}@{}/?appName={}&retryWrites=true&w=majority",
            enc_user, enc_pass, host, app_name.trim()
        ))
    }

    pub fn is_cloud(&self) -> bool {
        self.mongo_client.is_some()
    }

    fn get_collection(&self) -> Option<Collection<UserPlaylist>> {
        self.mongo_client
            .as_ref()
            .map(|client| client.database(&self.database_name).collection("playlists"))
    }

    fn get_history_collection(&self) -> Option<Collection<GuildPlaybackHistory>> {
        self.mongo_client
            .as_ref()
            .map(|client| client.database(&self.database_name).collection("playback_history"))
    }

    // --- Local storage helpers ---

    async fn read_local(&self) -> LocalPlaylistStorage {
        if !self.local_file.exists() {
            return LocalPlaylistStorage::default();
        }

        match tokio::fs::read_to_string(&self.local_file).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => LocalPlaylistStorage::default(),
        }
    }

    async fn write_local(&self, storage: &LocalPlaylistStorage) -> Result<(), String> {
        if let Some(parent) = self.local_file.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let json = serde_json::to_string_pretty(storage).map_err(|e| e.to_string())?;
        tokio::fs::write(&self.local_file, json)
            .await
            .map_err(|e| e.to_string())
    }

    async fn read_local_history(&self) -> LocalHistoryStorage {
        if !self.local_history_file.exists() {
            return LocalHistoryStorage::default();
        }

        match tokio::fs::read_to_string(&self.local_history_file).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => LocalHistoryStorage::default(),
        }
    }

    async fn write_local_history(&self, storage: &LocalHistoryStorage) -> Result<(), String> {
        if let Some(parent) = self.local_history_file.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let json = serde_json::to_string_pretty(storage).map_err(|e| e.to_string())?;
        tokio::fs::write(&self.local_history_file, json)
            .await
            .map_err(|e| e.to_string())
    }

    // --- Playlists API ---

    pub async fn save_playlist(
        &self,
        user_id: u64,
        name: &str,
        tracks: Vec<TrackMetadata>,
    ) -> Result<usize, String> {
        let count = tracks.len();
        let playlist = UserPlaylist {
            user_id,
            name: name.trim().to_string(),
            tracks,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        if let Some(col) = self.get_collection() {
            let filter = doc! {
                "user_id": user_id as i64,
                "name": doc! { "$regex": format!("^{}$", regex_escape(name.trim())), "$options": "i" }
            };
            let options = ReplaceOptions::builder().upsert(true).build();
            col.replace_one(filter, &playlist)
                .with_options(options)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(count);
        }

        // Local fallback
        let _lock = self.local_lock.lock().await;
        let mut storage = self.read_local().await;
        storage
            .playlists
            .retain(|p| !(p.user_id == user_id && p.name.eq_ignore_ascii_case(name.trim())));
        storage.playlists.push(playlist);
        self.write_local(&storage).await?;

        Ok(count)
    }

    pub async fn get_playlist(&self, user_id: u64, name: &str) -> Option<UserPlaylist> {
        if let Some(col) = self.get_collection() {
            let filter = doc! {
                "user_id": user_id as i64,
                "name": doc! { "$regex": format!("^{}$", regex_escape(name.trim())), "$options": "i" }
            };
            if let Ok(doc) = col.find_one(filter).await {
                return doc;
            }
        }

        // Local fallback
        let _lock = self.local_lock.lock().await;
        let storage = self.read_local().await;
        storage
            .playlists
            .into_iter()
            .find(|p| p.user_id == user_id && p.name.eq_ignore_ascii_case(name.trim()))
    }

    pub async fn list_playlists(&self, user_id: u64) -> Vec<UserPlaylist> {
        if let Some(col) = self.get_collection() {
            let filter = doc! { "user_id": user_id as i64 };
            if let Ok(mut cursor) = col.find(filter).await {
                let mut list = Vec::new();
                while let Ok(true) = cursor.advance().await {
                    if let Ok(pl) = cursor.deserialize_current() {
                        list.push(pl);
                    }
                }
                list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                return list;
            }
        }

        // Local fallback
        let _lock = self.local_lock.lock().await;
        let storage = self.read_local().await;
        let mut list: Vec<UserPlaylist> = storage
            .playlists
            .into_iter()
            .filter(|p| p.user_id == user_id)
            .collect();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        list
    }

    pub async fn delete_playlist(&self, user_id: u64, name: &str) -> bool {
        if let Some(col) = self.get_collection() {
            let filter = doc! {
                "user_id": user_id as i64,
                "name": doc! { "$regex": format!("^{}$", regex_escape(name.trim())), "$options": "i" }
            };
            if let Ok(res) = col.delete_one(filter).await {
                return res.deleted_count > 0;
            }
        }

        // Local fallback
        let _lock = self.local_lock.lock().await;
        let mut storage = self.read_local().await;
        let prev_len = storage.playlists.len();
        storage
            .playlists
            .retain(|p| !(p.user_id == user_id && p.name.eq_ignore_ascii_case(name.trim())));
        let deleted = storage.playlists.len() < prev_len;
        if deleted {
            let _ = self.write_local(&storage).await;
        }
        deleted
    }

    // --- Playback History API (MongoDB + Local Fallback) ---

    pub async fn load_history(&self, guild_id: u64) -> Vec<TrackMetadata> {
        if let Some(col) = self.get_history_collection() {
            let filter = doc! { "guild_id": guild_id as i64 };
            if let Ok(Some(record)) = col.find_one(filter).await {
                return record.tracks;
            }
        }

        let _lock = self.local_lock.lock().await;
        let storage = self.read_local_history().await;
        storage
            .histories
            .into_iter()
            .find(|h| h.guild_id == guild_id)
            .map(|h| h.tracks)
            .unwrap_or_default()
    }

    pub async fn save_history(&self, guild_id: u64, tracks: Vec<TrackMetadata>) -> Result<(), String> {
        let record = GuildPlaybackHistory {
            guild_id,
            tracks,
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        if let Some(col) = self.get_history_collection() {
            let filter = doc! { "guild_id": guild_id as i64 };
            let options = ReplaceOptions::builder().upsert(true).build();
            let _ = col.replace_one(filter, &record).with_options(options).await;
            return Ok(());
        }

        let _lock = self.local_lock.lock().await;
        let mut storage = self.read_local_history().await;
        storage.histories.retain(|h| h.guild_id != guild_id);
        storage.histories.push(record);
        self.write_local_history(&storage).await
    }

    pub async fn clear_history(&self, guild_id: u64) -> Result<(), String> {
        if let Some(col) = self.get_history_collection() {
            let filter = doc! { "guild_id": guild_id as i64 };
            let _ = col.delete_one(filter).await;
            return Ok(());
        }

        let _lock = self.local_lock.lock().await;
        let mut storage = self.read_local_history().await;
        storage.histories.retain(|h| h.guild_id != guild_id);
        self.write_local_history(&storage).await
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "^$\\.*+?()[]{}|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
