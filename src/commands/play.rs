use serenity::all::{
    CommandDataOptionValue, CommandInteraction, Context, CreateEmbed, CreateEmbedAuthor,
    CreateEmbedFooter, CreateInteractionResponseFollowup,
};
use songbird::events::{Event, TrackEvent};
use std::sync::Arc;
use tracing::error;

use super::events::TrackEndHandler;
use crate::queue::{LoopMode, QueueManager};
use crate::source::SourceManager;
use crate::utils::embed::{format_duration, source_color, source_icon_url};
use crate::utils::response::{send_followup, send_response};

pub async fn handle_play(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => {
            let _ = send_response(ctx, command, "❌ This command can only be used in a server.", false).await;
            return;
        }
    };

    let connect_to = match crate::utils::voice::check_voice_channel(ctx, guild_id, command.user.id) {
        Ok(channel) => channel,
        Err(msg) => {
            let _ = send_response(ctx, command, msg, true).await;
            return;
        }
    };

    // Extract query argument
    let query = match command.data.options.iter().find(|opt| opt.name == "query") {
        Some(opt) => match &opt.value {
            CommandDataOptionValue::String(s) => s.clone(),
            _ => {
                let _ = send_response(ctx, command, "❌ Invalid query parameter.", false).await;
                return;
            }
        },
        None => {
            let _ = send_response(ctx, command, "❌ Please provide a song query or URL.", false).await;
            return;
        }
    };

    // Defer response
    if let Err(e) = command.defer(&ctx.http).await {
        error!("Failed to defer interaction: {:?}", e);
        return;
    }

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialization");

    let call_lock = match manager.join(guild_id, connect_to).await {
        Ok(lock) => lock,
        Err(e) => {
            let _ = send_followup(ctx, command, &format!("❌ Failed to connect to voice channel: {:?}", e)).await;
            return;
        }
    };

    // Resolve track or playlist (fast metadata resolution)
    let resolved = match source_mgr.resolve(&query).await {
        Ok(tracks) => tracks,
        Err(e) => {
            let _ = send_followup(ctx, command, &format!("❌ Could not find or extract audio: {}", e)).await;
            return;
        }
    };

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_currently_playing = handler.queue().current().is_some();

    if resolved.len() == 1 {
        let track = resolved[0].clone();
        queue_mgr.push_track(guild_id, track.clone()).await;

        if !is_currently_playing {
            let input = source_mgr.create_input(&track.stream_url).await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            if loop_mode == LoopMode::Track {
                let _ = track_handle.enable_loop();
            }

            let _ = track_handle.add_event(
                Event::Track(TrackEvent::End),
                TrackEndHandler {
                    guild_id,
                    queue_mgr: queue_mgr.clone(),
                    source_mgr: source_mgr.clone(),
                    call_lock: call_lock.clone(),
                },
            );
        }

        let queue_len = queue_mgr.get_queue(guild_id).await.len();
        let duration_str = format_duration(track.duration);
        let author_str = track.author.as_deref().unwrap_or("Unknown Artist");

        let mut embed = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(format!("Playing from {}", track.source))
                    .icon_url(source_icon_url(&track.source))
                    .url(&track.url),
            )
            .title(&track.title)
            .url(&track.url)
            .field("👤 Artist", author_str, true)
            .field("⏱️ Duration", duration_str, true)
            .field("📌 Position", format!("#{}", queue_len), true)
            .footer(
                CreateEmbedFooter::new(format!("Platform: {}", track.source))
                    .icon_url(source_icon_url(&track.source)),
            )
            .color(source_color(&track.source));

        if let Some(thumb) = &track.thumbnail {
            embed = embed.thumbnail(thumb);
        }

        let _ = command
            .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().embed(embed))
            .await;
    } else {
        // Playlist handling (instant enqueue without blocking!)
        let total_tracks = resolved.len();
        let source_name = resolved[0].source.clone();
        let first_track = resolved[0].clone();

        queue_mgr.push_playlist(guild_id, resolved.clone()).await;

        if !is_currently_playing {
            let input = source_mgr.create_input(&first_track.stream_url).await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            if loop_mode == LoopMode::Track {
                let _ = track_handle.enable_loop();
            }

            let _ = track_handle.add_event(
                Event::Track(TrackEvent::End),
                TrackEndHandler {
                    guild_id,
                    queue_mgr: queue_mgr.clone(),
                    source_mgr: source_mgr.clone(),
                    call_lock: call_lock.clone(),
                },
            );
        }

        let queue_len = queue_mgr.get_queue(guild_id).await.len();

        let embed = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(format!("{} Playlist Enqueued", source_name))
                    .icon_url(source_icon_url(&source_name)),
            )
            .title(format!("Added {} tracks to queue", total_tracks))
            .field("📌 First Track", format!("[**{}**]({})", first_track.title, first_track.url), false)
            .field("📊 Queue Total", format!("{} tracks", queue_len), true)
            .footer(
                CreateEmbedFooter::new(format!("Platform: {}", source_name))
                    .icon_url(source_icon_url(&source_name)),
            )
            .color(source_color(&source_name));

        let _ = command
            .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().embed(embed))
            .await;
    }
}
