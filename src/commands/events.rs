use serenity::all::GuildId;
use serenity::async_trait;
use songbird::{
    events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent},
    Call,
};
use std::sync::Arc;

use crate::queue::{LoopMode, QueueManager};
use crate::source::SourceManager;

pub struct TrackEndHandler {
    pub guild_id: GuildId,
    pub queue_mgr: Arc<QueueManager>,
    pub source_mgr: Arc<SourceManager>,
    pub call_lock: Arc<tokio::sync::Mutex<Call>>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let mode = self.queue_mgr.get_loop_mode(self.guild_id).await;

        let next_track = if mode == LoopMode::Queue {
            self.queue_mgr.cycle_queue(self.guild_id).await
        } else {
            self.queue_mgr.advance(self.guild_id).await
        };

        if let Some(track) = next_track {
            let mut handler = self.call_lock.lock().await;
            let input = self.source_mgr.create_input(&track.stream_url).await;
            let next_handle = handler.enqueue_input(input).await;
            let _ = next_handle.set_volume(0.8);

            if mode == LoopMode::Track {
                let _ = next_handle.enable_loop();
            }

            let _ = next_handle.add_event(
                Event::Track(TrackEvent::End),
                TrackEndHandler {
                    guild_id: self.guild_id,
                    queue_mgr: self.queue_mgr.clone(),
                    source_mgr: self.source_mgr.clone(),
                    call_lock: self.call_lock.clone(),
                },
            );
        }

        None
    }
}
