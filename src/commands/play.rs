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

    // Find user's voice channel without holding CacheRef across await
    let user_voice_channel_id = ctx
        .cache
        .guild(guild_id)
        .and_then(|g| g.voice_states.get(&command.user.id).and_then(|vs| vs.channel_id));

    let connect_to = match user_voice_channel_id {
        Some(channel) => channel,
        None => {
            let _ = send_response(ctx, command, "⚠️ You must be in a voice channel to play music.", false).await;
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

    // Resolve track or playlist
    let resolved = match source_mgr.resolve(&query).await {
        Ok(tracks) => tracks,
        Err(e) => {
            let _ = send_followup(ctx, command, &format!("❌ Could not find or extract audio: {}", e)).await;
            return;
        }
    };

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;

    if resolved.len() == 1 {
        let track = resolved[0].clone();
        queue_mgr.push_track(guild_id, track.clone()).await;

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
                track: track.clone(),
                queue_mgr: queue_mgr.clone(),
                source_mgr: source_mgr.clone(),
                call_lock: call_lock.clone(),
            },
        );

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
            .field("📌 Position", format!("#{}", handler.queue().len()), true)
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
        // Playlist handling
        let total_tracks = resolved.len();
        let source_name = resolved[0].source.clone();
        queue_mgr.push_playlist(guild_id, resolved.clone()).await;

        for track in &resolved {
            let input = source_mgr.create_input(&track.stream_url).await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            let _ = track_handle.add_event(
                Event::Track(TrackEvent::End),
                TrackEndHandler {
                    guild_id,
                    track: track.clone(),
                    queue_mgr: queue_mgr.clone(),
                    source_mgr: source_mgr.clone(),
                    call_lock: call_lock.clone(),
                },
            );
        }

        let embed = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(format!("{} Playlist Enqueued", source_name))
                    .icon_url(source_icon_url(&source_name)),
            )
            .title(format!("Added {} tracks to queue", total_tracks))
            .field("📌 First Track", format!("[**{}**]({})", resolved[0].title, resolved[0].url), false)
            .field("📊 Queue Total", format!("{} tracks", handler.queue().len()), true)
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
