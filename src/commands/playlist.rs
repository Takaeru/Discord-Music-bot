use serenity::all::{
    Color, CommandDataOptionValue, CommandInteraction, Context, CreateEmbed,
};
use std::sync::Arc;
use tracing::error;

use crate::commands::events::TrackEndHandler;
use crate::lang::{fmt, get_lang};
use crate::playlist::PlaylistStore;
use crate::queue::{LoopMode, QueueManager};
use crate::source::SourceManager;
use crate::utils::embed::format_duration;
use crate::utils::response::{send_followup, send_response};
use crate::utils::voice::check_voice_channel;

pub async fn handle_playlist(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
    playlist_store: &Arc<PlaylistStore>,
) {
    let user_id = command.user.id.get();

    let subcommand = command.data.options.iter().find(|opt| {
        matches!(
            opt.value,
            CommandDataOptionValue::SubCommand(_)
        )
    });

    let (sub_name, sub_options) = match subcommand {
        Some(opt) => match &opt.value {
            CommandDataOptionValue::SubCommand(options) => (opt.name.as_str(), options),
            _ => return,
        },
        None => return,
    };

    match sub_name {
        "save" => {
            let guild_id = match command.guild_id {
                Some(id) => id,
                None => {
                    let _ = send_response(ctx, command, get_lang().server_only, true).await;
                    return;
                }
            };

            let name = match sub_options.iter().find(|opt| opt.name == "name") {
                Some(opt) => match &opt.value {
                    CommandDataOptionValue::String(s) => s.trim(),
                    _ => "",
                },
                None => "",
            };

            if name.is_empty() {
                let _ = send_response(ctx, command, "⚠️ Please provide a playlist name.", true).await;
                return;
            }

            // Collect tracks: current + queue
            let mut all_tracks = Vec::new();
            if let Some(current) = queue_mgr.get_current(guild_id).await {
                all_tracks.push(current);
            }
            let upcoming = queue_mgr.get_queue(guild_id).await;
            for t in upcoming {
                // Skip if identical to current (since queue[0] is often the current track)
                if all_tracks.first().map(|c| c.url == t.url).unwrap_or(false) {
                    continue;
                }
                all_tracks.push(t);
            }

            if all_tracks.is_empty() {
                let _ = send_response(ctx, command, get_lang().playlist_empty_queue, true).await;
                return;
            }

            let count_saved = match playlist_store.save_playlist(user_id, name, all_tracks).await {
                Ok(c) => c,
                Err(e) => {
                    let err_msg = format!("❌ Failed to save playlist: {}", e);
                    let _ = send_response(ctx, command, &err_msg, true).await;
                    return;
                }
            };

            let count_str = count_saved.to_string();
            let base_msg = fmt(get_lang().playlist_saved, &[&count_str, &name]);
            let _ = send_response(ctx, command, &base_msg, false).await;
        }
        "load" => {
            let guild_id = match command.guild_id {
                Some(id) => id,
                None => {
                    let _ = send_response(ctx, command, get_lang().server_only, true).await;
                    return;
                }
            };

            let connect_to = match check_voice_channel(ctx, guild_id, command.user.id) {
                Ok(ch) => ch,
                Err(msg) => {
                    let _ = send_response(ctx, command, msg, true).await;
                    return;
                }
            };

            let name = match sub_options.iter().find(|opt| opt.name == "name") {
                Some(opt) => match &opt.value {
                    CommandDataOptionValue::String(s) => s.trim(),
                    _ => "",
                },
                None => "",
            };

            if let Err(e) = command.defer(&ctx.http).await {
                error!("Failed to defer interaction: {:?}", e);
                return;
            }

            let playlist = match playlist_store.get_playlist(user_id, name).await {
                Some(pl) => pl,
                None => {
                    let msg = fmt(get_lang().playlist_not_found, &[&name]);
                    let _ = send_followup(ctx, command, &msg).await;
                    return;
                }
            };

            if playlist.tracks.is_empty() {
                let _ = send_followup(ctx, command, "⚠️ This playlist has no tracks.").await;
                return;
            }

            let manager = songbird::get(ctx)
                .await
                .expect("Songbird Voice client placed in at initialization");

            let call_lock = match manager.join(guild_id, connect_to).await {
                Ok(lock) => lock,
                Err(e) => {
                    let msg = fmt(get_lang().failed_connect_voice, &[&e]);
                    let _ = send_followup(ctx, command, &msg).await;
                    return;
                }
            };

            let mut handler = call_lock.lock().await;
            let is_currently_playing = handler.queue().current().is_some();
            let loop_mode = queue_mgr.get_loop_mode(guild_id).await;

            let first_track = playlist.tracks[0].clone();
            let count_str = playlist.tracks.len().to_string();

            queue_mgr.push_playlist(guild_id, playlist.tracks.clone()).await;
            queue_mgr.set_text_channel(guild_id, command.channel_id).await;

            if !is_currently_playing {
                let filter = queue_mgr.get_filter(guild_id).await;
                let input = source_mgr
                    .create_input_filtered(&first_track.stream_url, None, filter.ffmpeg_filter())
                    .await;
                let track_handle = handler.enqueue_input(input).await;
                let _ = track_handle.set_volume(0.8);

                queue_mgr.set_current_track(guild_id, first_track.clone()).await;
                queue_mgr.push_history(guild_id, first_track.clone()).await;

                if loop_mode == LoopMode::Track {
                    let _ = track_handle.enable_loop();
                }

                let _ = track_handle.add_event(
                    songbird::events::Event::Track(songbird::events::TrackEvent::End),
                    TrackEndHandler {
                        guild_id,
                        queue_mgr: queue_mgr.clone(),
                        source_mgr: source_mgr.clone(),
                        call_lock: call_lock.clone(),
                        http: ctx.http.clone(),
                    },
                );
            }

            let base_msg = fmt(get_lang().playlist_loaded, &[&count_str, &name]);
            let _ = send_followup(ctx, command, &base_msg).await;
        }
        "list" => {
            let playlists = playlist_store.list_playlists(user_id).await;
            if playlists.is_empty() {
                let _ = send_response(ctx, command, get_lang().playlist_list_empty, false).await;
                return;
            }

            let mut desc = String::new();
            for (i, pl) in playlists.iter().enumerate() {
                desc.push_str(&format!(
                    "{}. 📂 **{}** — {} track(s)\n",
                    i + 1,
                    pl.name,
                    pl.tracks.len()
                ));
            }

            let embed = CreateEmbed::new()
                .title(get_lang().playlist_list_title)
                .description(desc)
                .color(Color::from_rgb(88, 101, 242))
                .footer(serenity::all::CreateEmbedFooter::new(
                    "Use /playlist load <name> to play a playlist",
                ));

            let _ = command
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new().embed(embed),
                    ),
                )
                .await;
        }
        "show" => {
            let name = match sub_options.iter().find(|opt| opt.name == "name") {
                Some(opt) => match &opt.value {
                    CommandDataOptionValue::String(s) => s.trim(),
                    _ => "",
                },
                None => "",
            };

            let playlist = match playlist_store.get_playlist(user_id, name).await {
                Some(pl) => pl,
                None => {
                    let msg = fmt(get_lang().playlist_not_found, &[&name]);
                    let _ = send_response(ctx, command, &msg, true).await;
                    return;
                }
            };

            let mut desc = String::new();
            let limit = playlist.tracks.len().min(15);
            for i in 0..limit {
                let track = &playlist.tracks[i];
                let dur = format_duration(track.duration);
                desc.push_str(&format!("{}. [{}]({}) `[{}]`\n", i + 1, track.title, track.url, dur));
            }

            if playlist.tracks.len() > 15 {
                desc.push_str(&format!("\n*...and {} more tracks*", playlist.tracks.len() - 15));
            }

            let embed = CreateEmbed::new()
                .title(fmt(get_lang().playlist_show_title, &[&playlist.name]))
                .description(desc)
                .color(Color::from_rgb(88, 101, 242))
                .footer(serenity::all::CreateEmbedFooter::new(format!(
                    "Total: {} track(s) • Use /playlist load <name> to play",
                    playlist.tracks.len()
                )));

            let _ = command
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new().embed(embed),
                    ),
                )
                .await;
        }
        "delete" => {
            let name = match sub_options.iter().find(|opt| opt.name == "name") {
                Some(opt) => match &opt.value {
                    CommandDataOptionValue::String(s) => s.trim(),
                    _ => "",
                },
                None => "",
            };

            let deleted = playlist_store.delete_playlist(user_id, name).await;
            if deleted {
                let msg = fmt(get_lang().playlist_deleted, &[&name]);
                let _ = send_response(ctx, command, &msg, false).await;
            } else {
                let msg = fmt(get_lang().playlist_not_found, &[&name]);
                let _ = send_response(ctx, command, &msg, true).await;
            }
        }
        _ => {}
    }
}
