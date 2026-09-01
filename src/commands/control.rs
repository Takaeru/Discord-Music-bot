use serenity::all::{
    Color, CommandDataOptionValue, CommandInteraction, ComponentInteraction, Context, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
};
use std::sync::Arc;
use tracing::error;

use crate::lang::{fmt, get_lang};
use crate::queue::{LoopMode, QueueManager};
use crate::utils::embed::build_now_playing_embed;
use crate::utils::response::{send_followup, send_response};
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

    let _ = command.defer(&ctx.http).await;
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.pause();
            let _ = send_followup(ctx, command, get_lang().playback_paused).await;
        } else {
            let _ = send_followup(ctx, command, get_lang().nothing_playing).await;
        }
    } else {
        let _ = send_followup(ctx, command, get_lang().not_connected).await;
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

    let _ = command.defer(&ctx.http).await;
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.play();
            let _ = send_followup(ctx, command, get_lang().playback_resumed).await;
        } else {
            let _ = send_followup(ctx, command, get_lang().nothing_playing).await;
        }
    } else {
        let _ = send_followup(ctx, command, get_lang().not_connected).await;
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
            let _ = send_response(ctx, command, get_lang().skipped_current, false).await;
        } else {
            let _ = send_response(ctx, command, get_lang().nothing_to_skip, true).await;
        }
    } else {
        let _ = send_response(ctx, command, get_lang().not_connected, true).await;
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

    let _ = command.defer(&ctx.http).await;
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();
        queue_mgr.clear(guild_id).await;
        let _ = send_followup(ctx, command, get_lang().stopped_and_cleared).await;
    } else {
        let _ = send_followup(ctx, command, get_lang().not_connected).await;
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

    let msg = fmt(get_lang().repeat_mode_set, &[&mode.emoji(), &mode.as_str()]);
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
            let msg = fmt(get_lang().volume_set, &[&volume_level]);
            let _ = send_response(ctx, command, &msg, false).await;
        } else {
            let _ = send_response(ctx, command, get_lang().nothing_playing_now, true).await;
        }
    } else {
        let _ = send_response(ctx, command, get_lang().not_in_voice, true).await;
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
        // Clear queue BEFORE leaving to prevent TrackEndHandler from re-populating
        queue_mgr.clear(guild_id).await;
        if let Err(e) = manager.leave(guild_id).await {
            let msg = fmt(get_lang().failed_leave_voice, &[&e]);
            let _ = send_response(ctx, command, &msg, true).await;
        } else {
            let _ = send_response(ctx, command, get_lang().disconnected, false).await;
        }
    } else {
        let _ = send_response(ctx, command, get_lang().not_in_voice, true).await;
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
        get_lang().shuffle_enabled
    } else {
        get_lang().shuffle_disabled
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
        let msg = fmt(get_lang().cleared_n_tracks, &[&removed]);
        let _ = send_response(ctx, command, &msg, false).await;
    } else {
        let _ = send_response(ctx, command, get_lang().no_upcoming_to_clear, true).await;
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
            get_lang().position_must_be_1,
            true,
        )
        .await;
        return;
    }

    if let Some(removed_track) = queue_mgr.remove_at(guild_id, position).await {
        let msg = fmt(get_lang().removed_track, &[&position, &removed_track.title, &removed_track.url]);
        let _ = send_response(ctx, command, &msg, false).await;
    } else {
        let msg = fmt(get_lang().no_track_at_position, &[&position]);
        let _ = send_response(ctx, command, &msg, true).await;
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
        let _ = send_response(ctx, command, get_lang().track0_already_playing, true).await;
        return;
    }

    let target_track = queue_mgr.jump_to(guild_id, position).await;
    if let Some(track) = target_track {
        // Defer BEFORE expensive create_input (yt-dlp + ffmpeg can take 2-8s)
        if let Err(e) = command.defer(&ctx.http).await {
            error!("Failed to defer interaction: {:?}", e);
            return;
        }

        let manager = songbird::get(ctx).await.unwrap();
        if let Some(call_lock) = manager.get(guild_id) {
            let mut handler = call_lock.lock().await;
            handler.queue().stop();

            let input = source_mgr.create_input(&track.stream_url).await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            // Mark as current track so now-playing reports the jumped-to song
            queue_mgr.set_current_track(guild_id, track.clone()).await;

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

        let msg = fmt(get_lang().jumped_to, &[&position, &track.title, &track.url]);
        let _ = send_followup(ctx, command, &msg).await;
    } else {
        let msg = fmt(get_lang().invalid_position, &[&position]);
        let _ = send_response(ctx, command, &msg, true).await;
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
        // Defer BEFORE expensive create_input (yt-dlp + ffmpeg can take 2-8s)
        if let Err(e) = command.defer(&ctx.http).await {
            error!("Failed to defer interaction: {:?}", e);
            return;
        }

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

        let msg = fmt(get_lang().replaying, &[&current.title, &current.url]);
        let _ = send_followup(ctx, command, &msg).await;
    } else {
        let _ = send_response(ctx, command, get_lang().nothing_playing, true).await;
    }
}

pub async fn handle_ping(ctx: &Context, command: &CommandInteraction) {
    let embed = CreateEmbed::new()
        .title(get_lang().ping_title)
        .field(get_lang().ping_gateway_status, get_lang().ping_gateway_value, false)
        .field(get_lang().ping_audio_engine, get_lang().ping_audio_value, false)
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
        .title(get_lang().help_title)
        .description(get_lang().help_description)
        .field("🎵 `/play <query>`", get_lang().help_play, false)
        .field("⏭️ `/playnext <query>`", get_lang().help_playnext, false)
        .field("⏸️ `/pause` | ▶️ `/resume`", get_lang().help_pause_resume, true)
        .field("⏭️ `/skip` | 🔄 `/replay`", get_lang().help_skip_replay, true)
        .field("🔀 `/shuffle`", get_lang().help_shuffle, true)
        .field("🔁 `/repeat <mode>`", get_lang().help_repeat, true)
        .field("📋 `/queue` | 📻 `/nowplaying`", get_lang().help_queue_nowplaying, true)
        .field("🗑️ `/remove <pos>` | 🗑️ `/clear`", get_lang().help_remove_clear, true)
        .field("⏭️ `/jump <pos>`", get_lang().help_jump, true)
        .field("🔊 `/volume <0-100>`", get_lang().help_volume, true)
        .field("⏹️ `/stop` | 👋 `/leave`", get_lang().help_stop_leave, true)
        .field("🏓 `/ping`", get_lang().help_ping, true)
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
            // Defer immediately, then stop current track
            let _ = component.defer(&ctx.http).await;

            let manager = songbird::get(ctx).await.unwrap();
            if let Some(handler_lock) = manager.get(guild_id) {
                let handler = handler_lock.lock().await;
                if let Some(current) = handler.queue().current() {
                    let _ = current.disable_loop();
                    let _ = current.stop();
                }
            }

            // TrackEndHandler will advance the queue and send the now-playing message
            let _ = component
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .content(get_lang().skipped_current),
                )
                .await;
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
                            .content(get_lang().stopped_and_cleared)
                            .embeds(vec![])
                            .components(vec![]),
                    ),
                )
                .await;
        }
        _ => {}
    }
}
