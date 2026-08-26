use serenity::all::GuildId;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::source::TrackMetadata;

#[derive(Clone, Default)]
pub struct QueueManager {
    queues: Arc<Mutex<HashMap<GuildId, VecDeque<TrackMetadata>>>>,
    current: Arc<Mutex<HashMap<GuildId, TrackMetadata>>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            current: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn push_track(&self, guild_id: GuildId, track: TrackMetadata) {
        let mut curr_map = self.current.lock().await;
        if !curr_map.contains_key(&guild_id) {
            curr_map.insert(guild_id, track.clone());
        }

        let mut map = self.queues.lock().await;
        map.entry(guild_id).or_default().push_back(track);
    }

    pub async fn push_playlist(&self, guild_id: GuildId, tracks: Vec<TrackMetadata>) {
        if tracks.is_empty() {
            return;
        }

        let mut curr_map = self.current.lock().await;
        if !curr_map.contains_key(&guild_id) {
            curr_map.insert(guild_id, tracks[0].clone());
        }

        let mut map = self.queues.lock().await;
        let queue = map.entry(guild_id).or_default();
        for track in tracks {
            queue.push_back(track);
        }
    }

    pub async fn get_current(&self, guild_id: GuildId) -> Option<TrackMetadata> {
        let map = self.current.lock().await;
        map.get(&guild_id).cloned()
    }

    pub async fn get_queue(&self, guild_id: GuildId) -> Vec<TrackMetadata> {
        let map = self.queues.lock().await;
        map.get(&guild_id).map(|q| q.iter().cloned().collect()).unwrap_or_default()
    }

    pub async fn advance(&self, guild_id: GuildId) -> Option<TrackMetadata> {
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            queue.pop_front();
            let next = queue.front().cloned();
            let mut curr_map = self.current.lock().await;
            if let Some(track) = &next {
                curr_map.insert(guild_id, track.clone());
            } else {
                curr_map.remove(&guild_id);
            }
            next
        } else {
            None
        }
    }

    pub async fn clear(&self, guild_id: GuildId) {
        let mut map = self.queues.lock().await;
        map.remove(&guild_id);
        let mut curr_map = self.current.lock().await;
        curr_map.remove(&guild_id);
    }
}
