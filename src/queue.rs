use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use serenity::all::{ChannelId, GuildId, MessageId};
use std::collections::{HashMap, VecDeque};
use crate::lang::get_lang;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::source::TrackMetadata;

const SEARCH_TTL_SECS: u64 = 300; // 5 minutes

struct SearchEntry {
    guild_id: GuildId,
    results: Vec<TrackMetadata>,
    created_at: Instant,
    play_next: bool,
}

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
            LoopMode::Off => get_lang().loop_off,
            LoopMode::Track => get_lang().loop_track,
            LoopMode::Queue => get_lang().loop_queue,
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
    current_track: Arc<Mutex<HashMap<GuildId, TrackMetadata>>>,
    loop_modes: Arc<Mutex<HashMap<GuildId, LoopMode>>>,
    shuffled: Arc<Mutex<HashMap<GuildId, bool>>>,
    text_channels: Arc<Mutex<HashMap<GuildId, ChannelId>>>,
    last_messages: Arc<Mutex<HashMap<GuildId, MessageId>>>,
    search_results: Arc<Mutex<HashMap<MessageId, SearchEntry>>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            current_track: Arc::new(Mutex::new(HashMap::new())),
            loop_modes: Arc::new(Mutex::new(HashMap::new())),
            shuffled: Arc::new(Mutex::new(HashMap::new())),
            text_channels: Arc::new(Mutex::new(HashMap::new())),
            last_messages: Arc::new(Mutex::new(HashMap::new())),
            search_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn push_track(&self, guild_id: GuildId, track: TrackMetadata) {
        let mut map = self.queues.lock().await;
        map.entry(guild_id).or_default().push_back(track);
    }

    /// Insert track at position 1 (right after currently playing) for /playnext
    pub async fn push_next(&self, guild_id: GuildId, track: TrackMetadata) {
        let mut map = self.queues.lock().await;
        let queue = map.entry(guild_id).or_default();
        if queue.is_empty() {
            queue.push_back(track);
        } else {
            queue.insert(1, track);
        }
    }

    /// Insert multiple tracks at position 1 preserving order for /playnext playlist
    pub async fn push_next_playlist(&self, guild_id: GuildId, tracks: Vec<TrackMetadata>) {
        if tracks.is_empty() {
            return;
        }
        let mut map = self.queues.lock().await;
        let queue = map.entry(guild_id).or_default();
        for (i, track) in tracks.into_iter().enumerate() {
            if queue.is_empty() {
                queue.push_back(track);
            } else {
                queue.insert(1 + i, track);
            }
        }
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
        // Check the authoritative current_track first (set when track actually starts playing)
        let ct_map = self.current_track.lock().await;
        if let Some(track) = ct_map.get(&guild_id) {
            return Some(track.clone());
        }
        // Fallback to queue front
        let map = self.queues.lock().await;
        map.get(&guild_id).and_then(|q| q.front().cloned())
    }

    /// Set the currently playing track (called from TrackEndHandler after enqueue)
    pub async fn set_current_track(&self, guild_id: GuildId, track: TrackMetadata) {
        let mut map = self.current_track.lock().await;
        map.insert(guild_id, track);
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

    pub async fn remove_at(&self, guild_id: GuildId, index: usize) -> Option<TrackMetadata> {
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            if index < queue.len() {
                queue.remove(index)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn clear_upcoming(&self, guild_id: GuildId) -> usize {
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            if queue.len() > 1 {
                let removed_count = queue.len() - 1;
                queue.truncate(1);
                removed_count
            } else {
                0
            }
        } else {
            0
        }
    }

    pub async fn set_text_channel(&self, guild_id: GuildId, channel_id: ChannelId) {
        let mut map = self.text_channels.lock().await;
        map.insert(guild_id, channel_id);
    }

    pub async fn get_text_channel(&self, guild_id: GuildId) -> Option<ChannelId> {
        let map = self.text_channels.lock().await;
        map.get(&guild_id).copied()
    }

    pub async fn set_last_message_id(&self, guild_id: GuildId, msg_id: MessageId) {
        let mut map = self.last_messages.lock().await;
        map.insert(guild_id, msg_id);
    }

    pub async fn get_last_message_id(&self, guild_id: GuildId) -> Option<MessageId> {
        let map = self.last_messages.lock().await;
        map.get(&guild_id).copied()
    }

    pub async fn set_search_results(
        &self,
        msg_id: MessageId,
        guild_id: GuildId,
        results: Vec<TrackMetadata>,
        play_next: bool,
    ) {
        let mut map = self.search_results.lock().await;
        // Cleanup expired entries while we hold the lock
        map.retain(|_, entry| entry.created_at.elapsed().as_secs() < SEARCH_TTL_SECS);
        map.insert(
            msg_id,
            SearchEntry {
                guild_id,
                results,
                created_at: Instant::now(),
                play_next,
            },
        );
    }

    pub async fn get_search_results(&self, msg_id: MessageId) -> Vec<TrackMetadata> {
        let map = self.search_results.lock().await;
        map.get(&msg_id)
            .filter(|e| e.created_at.elapsed().as_secs() < SEARCH_TTL_SECS)
            .map(|e| e.results.clone())
            .unwrap_or_default()
    }

    pub async fn remove_search_results(&self, msg_id: MessageId) {
        let mut map = self.search_results.lock().await;
        map.remove(&msg_id);
    }

    /// Check if search results were created via /playnext
    pub async fn is_search_play_next(&self, msg_id: MessageId) -> bool {
        let map = self.search_results.lock().await;
        map.get(&msg_id)
            .filter(|e| e.created_at.elapsed().as_secs() < SEARCH_TTL_SECS)
            .map(|e| e.play_next)
            .unwrap_or(false)
    }

    pub async fn clear(&self, guild_id: GuildId) {
        let mut map = self.queues.lock().await;
        map.remove(&guild_id);
        let mut ct_map = self.current_track.lock().await;
        ct_map.remove(&guild_id);
        let mut loop_map = self.loop_modes.lock().await;
        loop_map.remove(&guild_id);
        let mut shuf_map = self.shuffled.lock().await;
        shuf_map.remove(&guild_id);
        let mut tc_map = self.text_channels.lock().await;
        tc_map.remove(&guild_id);
        let mut msg_map = self.last_messages.lock().await;
        msg_map.remove(&guild_id);
        let mut sr_map = self.search_results.lock().await;
        sr_map.retain(|_, entry| entry.guild_id != guild_id);
    }
}
