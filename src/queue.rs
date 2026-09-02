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

#[derive(Clone)]
pub struct RecommendEntry {
    pub tracks: Vec<TrackMetadata>,
    pub profile: crate::source::TasteProfile,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AudioFilter {
    #[default]
    Off,
    Bassboost,
    Nightcore,
    Vaporwave,
    EightD,
    Karaoke,
}

impl AudioFilter {
    pub fn ffmpeg_filter(&self) -> Option<&'static str> {
        match self {
            AudioFilter::Off => None,
            AudioFilter::Bassboost => Some("bass=g=8,dynaudnorm=f=200"),
            AudioFilter::Nightcore => Some("asetrate=48000*1.25,aresample=48000"),
            AudioFilter::Vaporwave => Some("asetrate=48000*0.8,aresample=48000"),
            AudioFilter::EightD => Some("apulsator=hz=0.125"),
            AudioFilter::Karaoke => Some("stereotools=mlev=0.03125"),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AudioFilter::Off => "Off",
            AudioFilter::Bassboost => "Bass Boost 🔊",
            AudioFilter::Nightcore => "Nightcore 🌙",
            AudioFilter::Vaporwave => "Vaporwave 🌊",
            AudioFilter::EightD => "8D Audio 🎧",
            AudioFilter::Karaoke => "Karaoke 🎤",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bassboost" | "bass" => AudioFilter::Bassboost,
            "nightcore" | "nc" => AudioFilter::Nightcore,
            "vaporwave" | "slowed" => AudioFilter::Vaporwave,
            "8d" | "eightd" => AudioFilter::EightD,
            "karaoke" => AudioFilter::Karaoke,
            _ => AudioFilter::Off,
        }
    }
}

#[derive(Clone, Default)]
pub struct QueueManager {
    queues: Arc<Mutex<HashMap<GuildId, VecDeque<TrackMetadata>>>>,
    current_track: Arc<Mutex<HashMap<GuildId, TrackMetadata>>>,
    loop_modes: Arc<Mutex<HashMap<GuildId, LoopMode>>>,
    shuffled: Arc<Mutex<HashMap<GuildId, bool>>>,
    audio_filters: Arc<Mutex<HashMap<GuildId, AudioFilter>>>,
    autoplay: Arc<Mutex<HashMap<GuildId, bool>>>,
    history: Arc<Mutex<HashMap<GuildId, VecDeque<TrackMetadata>>>>,
    text_channels: Arc<Mutex<HashMap<GuildId, ChannelId>>>,
    last_messages: Arc<Mutex<HashMap<GuildId, MessageId>>>,
    search_results: Arc<Mutex<HashMap<MessageId, SearchEntry>>>,
    recommend_results: Arc<Mutex<HashMap<String, RecommendEntry>>>,
    /// Set right before a manual stop (e.g. jump) so the old track's
    /// TrackEndHandler does NOT advance/cycle the queue again. The first
    /// End event consumes it (with a short TTL safety net).
    skip_end: Arc<Mutex<HashMap<GuildId, Instant>>>,
    playlist_store: Option<Arc<crate::playlist::PlaylistStore>>,
}

impl QueueManager {
    pub fn new(playlist_store: Option<Arc<crate::playlist::PlaylistStore>>) -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            current_track: Arc::new(Mutex::new(HashMap::new())),
            skip_end: Arc::new(Mutex::new(HashMap::new())),
            loop_modes: Arc::new(Mutex::new(HashMap::new())),
            shuffled: Arc::new(Mutex::new(HashMap::new())),
            audio_filters: Arc::new(Mutex::new(HashMap::new())),
            autoplay: Arc::new(Mutex::new(HashMap::new())),
            history: Arc::new(Mutex::new(HashMap::new())),
            text_channels: Arc::new(Mutex::new(HashMap::new())),
            last_messages: Arc::new(Mutex::new(HashMap::new())),
            search_results: Arc::new(Mutex::new(HashMap::new())),
            recommend_results: Arc::new(Mutex::new(HashMap::new())),
            playlist_store,
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

    /// Arm the skip-end latch: the next TrackEnd event for this guild is
    /// swallowed instead of advancing/cycling the queue. Call BEFORE
    /// `handler.queue().stop()` / `track.stop()` in manual transitions
    /// (jump, stop command) that already manage their own successor track.
    pub async fn set_skip_end(&self, guild_id: GuildId) {
        let mut map = self.skip_end.lock().await;
        map.insert(guild_id, Instant::now());
    }

    /// Consume the skip-end latch if armed (and fresh). Returns true when
    /// the caller (a TrackEndHandler) should do nothing.
    pub async fn take_skip_end(&self, guild_id: GuildId) -> bool {
        let mut map = self.skip_end.lock().await;
        match map.remove(&guild_id) {
            Some(armed_at) => armed_at.elapsed().as_secs() < 10,
            None => false,
        }
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
        let mode = self.get_loop_mode(guild_id).await;
        let mut map = self.queues.lock().await;
        if let Some(queue) = map.get_mut(&guild_id) {
            if index < queue.len() {
                if mode == LoopMode::Queue || mode == LoopMode::Track {
                    // Repeat modes: rotate instead of delete — jumped-over songs
                    // wrap to the back so the rotation/loop never loses tracks.
                    queue.rotate_left(index);
                } else {
                    // No loop: skip tracks (remove them from the queue)
                    queue.drain(0..index);
                }
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

    pub async fn set_recommend_results(&self, key: String, tracks: Vec<TrackMetadata>, profile: crate::source::TasteProfile) {
        let mut map = self.recommend_results.lock().await;
        map.insert(key, RecommendEntry { tracks, profile });
    }

    pub async fn get_recommend_entry(&self, key: &str) -> Option<RecommendEntry> {
        let map = self.recommend_results.lock().await;
        map.get(key).cloned()
    }

    pub async fn get_recommend_results(&self, key: &str) -> Option<Vec<TrackMetadata>> {
        let map = self.recommend_results.lock().await;
        map.get(key).map(|e| e.tracks.clone())
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
        let mut af_map = self.audio_filters.lock().await;
        af_map.remove(&guild_id);
        let mut ap_map = self.autoplay.lock().await;
        ap_map.remove(&guild_id);
        let mut hist_map = self.history.lock().await;
        hist_map.remove(&guild_id);
    }

    pub async fn get_filter(&self, guild_id: GuildId) -> AudioFilter {
        let filters = self.audio_filters.lock().await;
        filters.get(&guild_id).copied().unwrap_or_default()
    }

    pub async fn set_filter(&self, guild_id: GuildId, filter: AudioFilter) {
        let mut filters = self.audio_filters.lock().await;
        if filter == AudioFilter::Off {
            filters.remove(&guild_id);
        } else {
            filters.insert(guild_id, filter);
        }
    }

    pub async fn get_autoplay(&self, guild_id: GuildId) -> bool {
        let map = self.autoplay.lock().await;
        map.get(&guild_id).copied().unwrap_or(false)
    }

    pub async fn set_autoplay(&self, guild_id: GuildId, enabled: bool) {
        let mut map = self.autoplay.lock().await;
        map.insert(guild_id, enabled);
    }

    pub async fn toggle_autoplay(&self, guild_id: GuildId) -> bool {
        let mut map = self.autoplay.lock().await;
        let current = map.get(&guild_id).copied().unwrap_or(false);
        let new_val = !current;
        map.insert(guild_id, new_val);
        new_val
    }

    pub async fn get_history(&self, guild_id: GuildId) -> Vec<TrackMetadata> {
        let mut map = self.history.lock().await;
        if !map.contains_key(&guild_id) {
            if let Some(ref store) = self.playlist_store {
                let loaded = store.load_history(guild_id.get()).await;
                map.insert(guild_id, loaded.into());
            }
        }
        map.get(&guild_id)
            .map(|dq| dq.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Records a track into the playback history log and persists to MongoDB.
    /// If an identical track already exists (same title, URL, or YouTube video ID),
    /// it will NOT be saved ("jika lagunya sama jangan di simpan").
    /// Returns true if newly added, false if it was a duplicate and skipped.
    pub async fn push_history(&self, guild_id: GuildId, track: TrackMetadata) -> bool {
        let mut map = self.history.lock().await;

        if !map.contains_key(&guild_id) {
            if let Some(ref store) = self.playlist_store {
                let loaded = store.load_history(guild_id.get()).await;
                map.insert(guild_id, loaded.into());
            }
        }

        let dq = map.entry(guild_id).or_default();

        let track_yt_id = crate::source::SourceManager::extract_youtube_id(&track.url)
            .or_else(|| crate::source::SourceManager::extract_youtube_id(&track.stream_url));

        let already_exists = dq.iter().any(|t| {
            if t.title.eq_ignore_ascii_case(&track.title) {
                return true;
            }
            if !t.url.is_empty() && t.url == track.url {
                return true;
            }
            if let Some(ref tid) = track_yt_id {
                let other_yt_id = crate::source::SourceManager::extract_youtube_id(&t.url)
                    .or_else(|| crate::source::SourceManager::extract_youtube_id(&t.stream_url));
                if other_yt_id.as_deref() == Some(tid.as_str()) {
                    return true;
                }
            }
            false
        });

        if already_exists {
            return false;
        }

        if dq.len() >= 50 {
            dq.pop_front();
        }
        dq.push_back(track);

        // Asynchronously persist to MongoDB Atlas (or local fallback)
        if let Some(ref store) = self.playlist_store {
            let store_clone = store.clone();
            let tracks: Vec<TrackMetadata> = dq.iter().cloned().collect();
            let gid = guild_id.get();
            tokio::spawn(async move {
                let _ = store_clone.save_history(gid, tracks).await;
            });
        }

        true
    }

    pub async fn clear_history(&self, guild_id: GuildId) {
        let mut map = self.history.lock().await;
        map.insert(guild_id, VecDeque::new());

        if let Some(ref store) = self.playlist_store {
            let store_clone = store.clone();
            let gid = guild_id.get();
            tokio::spawn(async move {
                let _ = store_clone.clear_history(gid).await;
            });
        }
    }
}
