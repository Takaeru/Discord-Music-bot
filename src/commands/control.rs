use serenity::all::{
    Color, CommandDataOptionValue, CommandInteraction, ComponentInteraction, Context, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use std::sync::Arc;

use crate::queue::{LoopMode, QueueManager};
use crate::utils::embed::build_now_playing_embed;
use crate::utils::response::send_response;
use crate::utils::voice::check_voice_channel;

pub async fn handle_pause(ctx: &Context, command: &CommandInteraction) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.pause();
            let _ = send_response(ctx, command, "⏸️ Playback paused.", false).await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is currently playing.", true).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", true).await;
    }
}

pub async fn handle_resume(ctx: &Context, command: &CommandInteraction) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.play();
            let _ = send_response(ctx, command, "▶️ Playback resumed.", false).await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is currently playing.", true).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", true).await;
    }
}

pub async fn handle_skip(ctx: &Context, command: &CommandInteraction, _queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.disable_loop();
            let _ = current.stop();
            let _ = send_response(ctx, command, "⏭️ Skipped current track.", false).await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is playing to skip.", true).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", true).await;
    }
}

pub async fn handle_stop(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();
        queue_mgr.clear(guild_id).await;
        let _ = send_response(ctx, command, "⏹️ Playback stopped and queue cleared.", false).await;
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", true).await;
    }
}

pub async fn handle_repeat(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let mode_str = match command.data.options.iter().find(|opt| opt.name == "mode") {
        Some(opt) => match &opt.value {
            CommandDataOptionValue::String(s) => s.as_str(),
            _ => "off",
        },
        None => "off",
    };

    let mode = match mode_str {
        "track" | "song" => LoopMode::Track,
        "queue" => LoopMode::Queue,
        _ => LoopMode::Off,
    };

    queue_mgr.set_loop_mode(guild_id, mode).await;

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            match mode {
                LoopMode::Track => {
                    let _ = current.enable_loop();
                }
                LoopMode::Queue | LoopMode::Off => {
                    let _ = current.disable_loop();
                }
            }
        }
    }

    let msg = format!("{} Repeat mode set to **{}**", mode.emoji(), mode.as_str());
    let _ = send_response(ctx, command, &msg, false).await;
}

pub async fn handle_volume(ctx: &Context, command: &CommandInteraction) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let volume_level = match command.data.options.iter().find(|opt| opt.name == "level") {
        Some(opt) => match opt.value {
            CommandDataOptionValue::Integer(v) => v as f32,
            _ => 100.0,
        },
        None => 100.0,
    };

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let factor = volume_level / 100.0;
            let _ = current.set_volume(factor);
            let _ = send_response(
                ctx,
                command,
                &format!("🔊 Volume set to **{}%**", volume_level),
                false,
            )
            .await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is playing right now.", true).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not in a voice channel.", true).await;
    }
}

pub async fn handle_leave(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let manager = songbird::get(ctx).await.unwrap();
    if manager.get(guild_id).is_some() {
        if let Err(e) = manager.leave(guild_id).await {
            let _ = send_response(ctx, command, &format!("❌ Failed to leave voice: {:?}", e), true).await;
        } else {
            queue_mgr.clear(guild_id).await;
            let _ = send_response(ctx, command, "👋 Disconnected from voice channel.", false).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not in a voice channel.", true).await;
    }
}

pub async fn handle_shuffle(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let is_shuffled = queue_mgr.toggle_shuffle(guild_id).await;
    let msg = if is_shuffled {
        "🔀 Random / Shuffle mode **enabled**! Upcoming tracks have been randomized."
    } else {
        "➡️ Random / Shuffle mode **disabled**."
    };

    let _ = send_response(ctx, command, msg, false).await;
}

pub async fn handle_clear(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let removed = queue_mgr.clear_upcoming(guild_id).await;
    if removed > 0 {
        let _ = send_response(
            ctx,
            command,
            &format!("🗑️ Cleared **{}** upcoming track(s) from the queue.", removed),
            false,
        )
        .await;
    } else {
        let _ = send_response(ctx, command, "⚠️ The queue has no upcoming tracks to clear.", true).await;
    }
}

pub async fn handle_remove(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let position = match command.data.options.iter().find(|opt| opt.name == "position") {
        Some(opt) => match opt.value {
            CommandDataOptionValue::Integer(v) => v as usize,
            _ => 0,
        },
        None => 0,
    };

    if position == 0 {
        let _ = send_response(
            ctx,
            command,
            "⚠️ Position must be 1 or greater. Use `/skip` to skip the currently playing song (#0).",
            true,
        )
        .await;
        return;
    }

    if let Some(removed_track) = queue_mgr.remove_at(guild_id, position).await {
        let _ = send_response(
            ctx,
            command,
            &format!("🗑️ Removed **#{}** [**{}**]({}) from the queue.", position, removed_track.title, removed_track.url),
            false,
        )
        .await;
    } else {
        let _ = send_response(
            ctx,
            command,
            &format!("⚠️ No track found at position **#{}**.", position),
            true,
        )
        .await;
    }
}

pub async fn handle_jump(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<crate::source::SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    let position = match command.data.options.iter().find(|opt| opt.name == "position") {
        Some(opt) => match opt.value {
            CommandDataOptionValue::Integer(v) => v as usize,
            _ => 0,
        },
        None => 0,
    };

    if position == 0 {
        let _ = send_response(ctx, command, "⚠️ Track #0 is already playing. Use `/replay` to restart it.", true).await;
        return;
    }

    let target_track = queue_mgr.jump_to(guild_id, position).await;
    if let Some(track) = target_track {
        let manager = songbird::get(ctx).await.unwrap();
        if let Some(call_lock) = manager.get(guild_id) {
            let mut handler = call_lock.lock().await;
            handler.queue().stop();

            let input = source_mgr.create_input(&track.stream_url).await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
            if loop_mode == LoopMode::Track {
                let _ = track_handle.enable_loop();
            }

            let _ = track_handle.add_event(
                songbird::events::Event::Track(songbird::events::TrackEvent::End),
                crate::commands::events::TrackEndHandler {
                    guild_id,
                    queue_mgr: queue_mgr.clone(),
                    source_mgr: source_mgr.clone(),
                    call_lock: call_lock.clone(),
                    http: ctx.http.clone(),
                },
            );
        }

        let _ = send_response(
            ctx,
            command,
            &format!("⏭️ Jumped to **#{}**: [**{}**]({})", position, track.title, track.url),
            false,
        )
        .await;
    } else {
        let _ = send_response(
            ctx,
            command,
            &format!("⚠️ Invalid track position **#{}**.", position),
            true,
        )
        .await;
    }
}

pub async fn handle_replay(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<crate::source::SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, command.user.id) {
        let _ = send_response(ctx, command, msg, true).await;
        return;
    }

    if let Some(current) = queue_mgr.get_current(guild_id).await {
        let manager = songbird::get(ctx).await.unwrap();
        if let Some(call_lock) = manager.get(guild_id) {
            let mut handler = call_lock.lock().await;
            handler.queue().stop();

            let input = source_mgr.create_input(&current.stream_url).await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
            if loop_mode == LoopMode::Track {
                let _ = track_handle.enable_loop();
            }

            let _ = track_handle.add_event(
                songbird::events::Event::Track(songbird::events::TrackEvent::End),
                crate::commands::events::TrackEndHandler {
                    guild_id,
                    queue_mgr: queue_mgr.clone(),
                    source_mgr: source_mgr.clone(),
                    call_lock: call_lock.clone(),
                    http: ctx.http.clone(),
                },
            );
        }

        let _ = send_response(
            ctx,
            command,
            &format!("🔄 Replaying: [**{}**]({})", current.title, current.url),
            false,
        )
        .await;
    } else {
        let _ = send_response(ctx, command, "⚠️ Nothing is currently playing.", true).await;
    }
}

pub async fn handle_ping(ctx: &Context, command: &CommandInteraction) {
    let embed = CreateEmbed::new()
        .title("🏓 Pong!")
        .field("⚡ Bot Gateway Status", "🟢 Connected & Operational", false)
        .field("📻 Audio Engine", "Songbird 48kHz Stereo Opus (96kbps)", false)
        .color(Color::from_rgb(88, 101, 242));

    let _ = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed),
            ),
        )
        .await;
}

pub async fn handle_help(ctx: &Context, command: &CommandInteraction) {
    let embed = CreateEmbed::new()
        .title("📖 Discord Music Bot - Help Guide")
        .description("Lightweight high-performance music bot powered by Rust & Songbird.")
        .field("🎵 `/play <query>`", "Play YouTube / Spotify / SoundCloud / keywords", false)
        .field("⏸️ `/pause` | ▶️ `/resume`", "Pause or resume current playback", true)
        .field("⏭️ `/skip` | 🔄 `/replay`", "Skip song or replay from beginning", true)
        .field("🔀 `/shuffle`", "Toggle random / shuffle mode on or off", true)
        .field("🔁 `/repeat <mode>`", "Repeat mode: `off`, `track`, or `queue`", true)
        .field("📋 `/queue` | 📻 `/nowplaying`", "View interactive queue or current song info", true)
        .field("🗑️ `/remove <pos>` | 🗑️ `/clear`", "Remove specific song or clear upcoming queue", true)
        .field("⏭️ `/jump <pos>`", "Jump directly to a song in the queue", true)
        .field("🔊 `/volume <0-100>`", "Set volume level", true)
        .field("⏹️ `/stop` | 👋 `/leave`", "Stop music or disconnect bot from voice", true)
        .field("🏓 `/ping`", "Check bot latency and audio engine status", true)
        .color(Color::from_rgb(88, 101, 242));

    let _ = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed),
            ),
        )
        .await;
}

pub async fn handle_music_component(
    ctx: &Context,
    component: &ComponentInteraction,
    _source_mgr: &Arc<crate::source::SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match component.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Err(msg) = check_voice_channel(ctx, guild_id, component.user.id) {
        let _ = component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(msg)
                        .ephemeral(true),
                ),
            )
            .await;
        return;
    }

    let custom_id = component.data.custom_id.as_str();

    match custom_id {
        "music_pause" => {
            let manager = songbird::get(ctx).await.unwrap();
            if let Some(handler_lock) = manager.get(guild_id) {
                let handler = handler_lock.lock().await;
                if let Some(current) = handler.queue().current() {
                    let _ = current.pause();
                }
            }

            if let Some(current_track) = queue_mgr.get_current(guild_id).await {
                let queue = queue_mgr.get_queue(guild_id).await;
                let upcoming = queue.len().saturating_sub(1);
                let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
                let (embed, row) = build_now_playing_embed(&current_track, upcoming, loop_mode, true);
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .embed(embed)
                                .components(vec![row]),
                        ),
                    )
                    .await;
            }
        }
        "music_resume" => {
            let manager = songbird::get(ctx).await.unwrap();
            if let Some(handler_lock) = manager.get(guild_id) {
                let handler = handler_lock.lock().await;
                if let Some(current) = handler.queue().current() {
                    let _ = current.play();
                }
            }

            if let Some(current_track) = queue_mgr.get_current(guild_id).await {
                let queue = queue_mgr.get_queue(guild_id).await;
                let upcoming = queue.len().saturating_sub(1);
                let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
                let (embed, row) = build_now_playing_embed(&current_track, upcoming, loop_mode, false);
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .embed(embed)
                                .components(vec![row]),
                        ),
                    )
                    .await;
            }
        }
        "music_skip" => {
            let manager = songbird::get(ctx).await.unwrap();
            if let Some(handler_lock) = manager.get(guild_id) {
                let handler = handler_lock.lock().await;
                if let Some(current) = handler.queue().current() {
                    let _ = current.disable_loop();
                    let _ = current.stop();
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

            if let Some(current_track) = queue_mgr.get_current(guild_id).await {
                let queue = queue_mgr.get_queue(guild_id).await;
                let upcoming = queue.len().saturating_sub(1);
                let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
                let (embed, row) = build_now_playing_embed(&current_track, upcoming, loop_mode, false);
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .embed(embed)
                                .components(vec![row]),
                        ),
                    )
                    .await;
            } else {
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .content("📭 Antrean telah selesai.")
                                .embeds(vec![])
                                .components(vec![]),
                        ),
                    )
                    .await;
            }
        }
        "music_loop" => {
            let current_mode = queue_mgr.get_loop_mode(guild_id).await;
            let next_mode = match current_mode {
                LoopMode::Off => LoopMode::Track,
                LoopMode::Track => LoopMode::Queue,
                LoopMode::Queue => LoopMode::Off,
            };
            queue_mgr.set_loop_mode(guild_id, next_mode).await;

            let manager = songbird::get(ctx).await.unwrap();
            let is_paused = if let Some(handler_lock) = manager.get(guild_id) {
                let handler = handler_lock.lock().await;
                if let Some(current) = handler.queue().current() {
                    match next_mode {
                        LoopMode::Track => {
                            let _ = current.enable_loop();
                        }
                        LoopMode::Queue | LoopMode::Off => {
                            let _ = current.disable_loop();
                        }
                    }
                    matches!(current.get_info().await, Ok(info) if info.playing == songbird::tracks::PlayMode::Pause)
                } else {
                    false
                }
            } else {
                false
            };

            if let Some(current_track) = queue_mgr.get_current(guild_id).await {
                let queue = queue_mgr.get_queue(guild_id).await;
                let upcoming = queue.len().saturating_sub(1);
                let (embed, row) = build_now_playing_embed(&current_track, upcoming, next_mode, is_paused);
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .embed(embed)
                                .components(vec![row]),
                        ),
                    )
                    .await;
            }
        }
        "music_stop" => {
            let manager = songbird::get(ctx).await.unwrap();
            if let Some(handler_lock) = manager.get(guild_id) {
                let handler = handler_lock.lock().await;
                handler.queue().stop();
            }
            queue_mgr.clear(guild_id).await;

            let _ = component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("⏹️ Pemutaran dihentikan dan antrean dibersihkan.")
                            .embeds(vec![])
                            .components(vec![]),
                    ),
                )
                .await;
        }
        _ => {}
    }
}
