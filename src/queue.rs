use rand::seq::SliceRandom;
use rand::thread_rng;
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
    shuffled: Arc<Mutex<HashMap<GuildId, bool>>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            loop_modes: Arc::new(Mutex::new(HashMap::new())),
            shuffled: Arc::new(Mutex::new(HashMap::new())),
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
        let is_shuffled = self.get_shuffle(guild_id).await;
        let mut map = self.queues.lock().await;
        let queue = map.entry(guild_id).or_default();
        let start_len = queue.len();

        for track in tracks {
            queue.push_back(track);
        }

        // If shuffle is active, shuffle the newly added items
        if is_shuffled && queue.len() > 1 {
            let mut rng = thread_rng();
            let slice = queue.make_contiguous();
            let shuffle_start = if start_len == 0 { 1 } else { start_len };
            if shuffle_start < slice.len() {
                slice[shuffle_start..].shuffle(&mut rng);
            }
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

    pub async fn get_shuffle(&self, guild_id: GuildId) -> bool {
        let map = self.shuffled.lock().await;
        map.get(&guild_id).copied().unwrap_or(false)
    }

    pub async fn toggle_shuffle(&self, guild_id: GuildId) -> bool {
        let mut shuf_map = self.shuffled.lock().await;
        let is_shuffled = shuf_map.get(&guild_id).copied().unwrap_or(false);
        let new_state = !is_shuffled;
        shuf_map.insert(guild_id, new_state);

        if new_state {
            let mut q_map = self.queues.lock().await;
            if let Some(queue) = q_map.get_mut(&guild_id) {
                if queue.len() > 2 {
                    let mut rng = thread_rng();
                    let slice = queue.make_contiguous();
                    slice[1..].shuffle(&mut rng);
                }
            }
        }

        new_state
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

    pub async fn cycle_queue(&self, guild_id: GuildId) -> Option<TrackMetadata> {
        let is_shuffled = self.get_shuffle(guild_id).await;
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            if let Some(front) = queue.pop_front() {
                queue.push_back(front);
                if is_shuffled && queue.len() > 2 {
                    let mut rng = thread_rng();
                    let slice = queue.make_contiguous();
                    slice[1..].shuffle(&mut rng);
                }
                queue.front().cloned()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn jump_to(&self, guild_id: GuildId, index: usize) -> Option<TrackMetadata> {
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            if index < queue.len() {
                queue.drain(0..index);
                queue.front().cloned()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn clear(&self, guild_id: GuildId) {
        let mut map = self.queues.lock().await;
        map.remove(&guild_id);
        let mut loop_map = self.loop_modes.lock().await;
        loop_map.remove(&guild_id);
        let mut shuf_map = self.shuffled.lock().await;
        shuf_map.remove(&guild_id);
    }
}
