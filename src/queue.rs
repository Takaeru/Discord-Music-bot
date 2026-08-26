use serde::{Deserialize, Serialize};
use serenity::all::GuildId;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::source::TrackMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LoopMode {
    #[default]
    Off,
    Track,
    Queue,
}

impl LoopMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoopMode::Off => "Off",
            LoopMode::Track => "Track (1 Song)",
            LoopMode::Queue => "Entire Queue",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            LoopMode::Off => "➡️",
            LoopMode::Track => "🔂",
            LoopMode::Queue => "🔁",
        }
    }
}

#[derive(Clone, Default)]
pub struct QueueManager {
    queues: Arc<Mutex<HashMap<GuildId, VecDeque<TrackMetadata>>>>,
    loop_modes: Arc<Mutex<HashMap<GuildId, LoopMode>>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            loop_modes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn push_track(&self, guild_id: GuildId, track: TrackMetadata) {
        let mut map = self.queues.lock().await;
        map.entry(guild_id).or_default().push_back(track);
    }

    pub async fn push_playlist(&self, guild_id: GuildId, tracks: Vec<TrackMetadata>) {
        if tracks.is_empty() {
            return;
        }
        let mut map = self.queues.lock().await;
        let queue = map.entry(guild_id).or_default();
        for track in tracks {
            queue.push_back(track);
        }
    }

    pub async fn get_current(&self, guild_id: GuildId) -> Option<TrackMetadata> {
        let map = self.queues.lock().await;
        map.get(&guild_id).and_then(|q| q.front().cloned())
    }

    pub async fn get_queue(&self, guild_id: GuildId) -> Vec<TrackMetadata> {
        let map = self.queues.lock().await;
        map.get(&guild_id)
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub async fn get_loop_mode(&self, guild_id: GuildId) -> LoopMode {
        let map = self.loop_modes.lock().await;
        map.get(&guild_id).copied().unwrap_or_default()
    }

    pub async fn set_loop_mode(&self, guild_id: GuildId, mode: LoopMode) {
        let mut map = self.loop_modes.lock().await;
        map.insert(guild_id, mode);
    }

    pub async fn advance(&self, guild_id: GuildId) -> Option<TrackMetadata> {
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            queue.pop_front();
            queue.front().cloned()
        } else {
            None
        }
    }

    pub async fn clear(&self, guild_id: GuildId) {
        let mut map = self.queues.lock().await;
        map.remove(&guild_id);
        let mut loop_map = self.loop_modes.lock().await;
        loop_map.remove(&guild_id);
    }
}
