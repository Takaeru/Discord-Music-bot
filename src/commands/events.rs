use serenity::all::GuildId;
use serenity::async_trait;
use songbird::{
    events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent},
    Call,
};
use std::sync::Arc;

use crate::queue::{LoopMode, QueueManager};
use crate::source::{SourceManager, TrackMetadata};

pub struct TrackEndHandler {
    pub guild_id: GuildId,
    pub track: TrackMetadata,
    pub queue_mgr: Arc<QueueManager>,
    pub source_mgr: Arc<SourceManager>,
    pub call_lock: Arc<tokio::sync::Mutex<Call>>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let mode = self.queue_mgr.get_loop_mode(self.guild_id).await;
        if mode == LoopMode::Queue {
            // Re-enqueue the finished track to the back of the queue
            let mut handler = self.call_lock.lock().await;
            let input = self.source_mgr.create_input(&self.track.stream_url).await;
            let next_handle = handler.enqueue_input(input).await;
            let _ = next_handle.set_volume(0.8);

            let _ = next_handle.add_event(
                Event::Track(TrackEvent::End),
                TrackEndHandler {
                    guild_id: self.guild_id,
                    track: self.track.clone(),
                    queue_mgr: self.queue_mgr.clone(),
                    source_mgr: self.source_mgr.clone(),
                    call_lock: self.call_lock.clone(),
                },
            );

            self.queue_mgr.push_track(self.guild_id, self.track.clone()).await;
        }
        self.queue_mgr.advance(self.guild_id).await;
        None
    }
}
