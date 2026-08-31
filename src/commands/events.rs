use serenity::all::{CreateMessage, GuildId, Http};
use serenity::async_trait;
use songbird::{
    events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent},
    Call,
};
use std::sync::Arc;

use crate::lang::get_lang;
use crate::queue::{LoopMode, QueueManager};
use crate::source::SourceManager;
use crate::utils::embed::build_now_playing_embed;

pub struct TrackEndHandler {
    pub guild_id: GuildId,
    pub queue_mgr: Arc<QueueManager>,
    pub source_mgr: Arc<SourceManager>,
    pub call_lock: Arc<tokio::sync::Mutex<Call>>,
    pub http: Arc<Http>,
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

            // Mark as current track AFTER enqueue succeeds
            self.queue_mgr.set_current_track(self.guild_id, track.clone()).await;

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
                    http: self.http.clone(),
                },
            );

            // Auto-update / send Now Playing message with control buttons
            let queue = self.queue_mgr.get_queue(self.guild_id).await;
            let upcoming = queue.len().saturating_sub(1);
            let (embed, action_row) = build_now_playing_embed(&track, upcoming, mode, false);

            if let Some(channel_id) = self.queue_mgr.get_text_channel(self.guild_id).await {
                let create_msg = CreateMessage::new()
                    .embed(embed)
                    .components(vec![action_row]);

                if let Ok(new_msg) = channel_id.send_message(&self.http, create_msg).await {
                    self.queue_mgr.set_last_message_id(self.guild_id, new_msg.id).await;
                }
            }
        } else if let Some(channel_id) = self.queue_mgr.get_text_channel(self.guild_id).await {
            let _ = channel_id
                .send_message(
                    &self.http,
                    CreateMessage::new().content(get_lang().queue_finished_playing),
                )
                .await;
        }

        None
    }
}
