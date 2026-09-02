use serenity::all::{
    ButtonStyle, Color, CommandDataOptionValue, CommandInteraction, ComponentInteraction, Context,
    CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption,
};
use std::sync::Arc;
use tracing::error;

use crate::lang::{fmt, get_lang};
use crate::queue::{LoopMode, QueueManager};
use crate::utils::embed::{build_now_playing_embed, format_duration, truncate};
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
            // Arm the latch BEFORE stopping: the old track's End handler would
            // otherwise re-advance the (already rotated) queue and enqueue a
            // wrong successor. We manage the successor ourselves below.
            queue_mgr.set_skip_end(guild_id).await;
            handler.queue().stop();

            let filter = queue_mgr.get_filter(guild_id).await;
            let input = source_mgr
                .create_input_filtered(&track.stream_url, None, filter.ffmpeg_filter())
                .await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            // Mark as current track so now-playing reports the jumped-to song
            queue_mgr.set_current_track(guild_id, track.clone()).await;
            queue_mgr.push_history(guild_id, track.clone()).await;

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

            let filter = queue_mgr.get_filter(guild_id).await;
            let input = source_mgr
                .create_input_filtered(&current.stream_url, None, filter.ffmpeg_filter())
                .await;
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

fn parse_time_str(input: &str) -> Option<std::time::Duration> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }

    if let Ok(secs) = s.parse::<u64>() {
        return Some(std::time::Duration::from_secs(secs));
    }

    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => {
                let m = parts[0].trim().parse::<u64>().ok()?;
                let sec = parts[1].trim().parse::<u64>().ok()?;
                if sec >= 60 {
                    return None;
                }
                return Some(std::time::Duration::from_secs(m * 60 + sec));
            }
            3 => {
                let h = parts[0].trim().parse::<u64>().ok()?;
                let m = parts[1].trim().parse::<u64>().ok()?;
                let sec = parts[2].trim().parse::<u64>().ok()?;
                if m >= 60 || sec >= 60 {
                    return None;
                }
                return Some(std::time::Duration::from_secs(h * 3600 + m * 60 + sec));
            }
            _ => return None,
        }
    }

    let lower = s.to_lowercase();
    if lower.ends_with('s') && !lower.contains('m') && !lower.contains('h') {
        if let Ok(secs) = lower.trim_end_matches('s').trim().parse::<u64>() {
            return Some(std::time::Duration::from_secs(secs));
        }
    }

    None
}

pub async fn handle_seek(
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

    let time_str = match command.data.options.iter().find(|opt| opt.name == "time") {
        Some(opt) => match &opt.value {
            CommandDataOptionValue::String(s) => s.trim(),
            _ => "",
        },
        None => "",
    };

    let target_dur = match parse_time_str(time_str) {
        Some(d) => d,
        None => {
            let _ = send_response(ctx, command, get_lang().invalid_time_format, true).await;
            return;
        }
    };

    if let Some(current) = queue_mgr.get_current(guild_id).await {
        if let Some(max_dur) = current.duration {
            if target_dur > max_dur {
                let dur_formatted = crate::utils::embed::format_duration(Some(max_dur));
                let msg = fmt(get_lang().seek_exceeds_duration, &[&dur_formatted]);
                let _ = send_response(ctx, command, &msg, true).await;
                return;
            }
        }

        if let Err(e) = command.defer(&ctx.http).await {
            error!("Failed to defer interaction: {:?}", e);
            return;
        }

        let manager = songbird::get(ctx).await.unwrap();
        if let Some(call_lock) = manager.get(guild_id) {
            let mut handler = call_lock.lock().await;

            // Arm skip_end latch so the old track's End event does not advance the queue
            queue_mgr.set_skip_end(guild_id).await;
            handler.queue().stop();

            let filter = queue_mgr.get_filter(guild_id).await;
            let input = source_mgr
                .create_input_filtered(
                    &current.stream_url,
                    Some(target_dur),
                    filter.ffmpeg_filter(),
                )
                .await;
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

        let target_formatted = crate::utils::embed::format_duration(Some(target_dur));
        let msg = fmt(get_lang().seek_success, &[&target_formatted]);
        let _ = send_followup(ctx, command, &msg).await;
    } else {
        let _ = send_response(ctx, command, get_lang().nothing_playing, true).await;
    }
}

pub async fn handle_filter(
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

    let mode_str = match command.data.options.iter().find(|opt| opt.name == "mode") {
        Some(opt) => match &opt.value {
            CommandDataOptionValue::String(s) => s.as_str(),
            _ => "off",
        },
        None => "off",
    };

    let filter = crate::queue::AudioFilter::from_str(mode_str);
    queue_mgr.set_filter(guild_id, filter).await;

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(call_lock) = manager.get(guild_id) {
        let mut handler = call_lock.lock().await;

        if let Some(current_handle) = handler.queue().current() {
            let current_pos = match current_handle.get_info().await {
                Ok(info) => info.position,
                Err(_) => std::time::Duration::from_secs(0),
            };

            if let Some(current_track) = queue_mgr.get_current(guild_id).await {
                if let Err(e) = command.defer(&ctx.http).await {
                    error!("Failed to defer interaction: {:?}", e);
                    return;
                }

                queue_mgr.set_skip_end(guild_id).await;
                handler.queue().stop();

                let input = source_mgr
                    .create_input_filtered(
                        &current_track.stream_url,
                        Some(current_pos),
                        filter.ffmpeg_filter(),
                    )
                    .await;
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

                let response_msg = if filter == crate::queue::AudioFilter::Off {
                    get_lang().filter_disabled.to_string()
                } else {
                    let fname = filter.name();
                    fmt(get_lang().filter_set, &[&fname])
                };
                let _ = send_followup(ctx, command, &response_msg).await;
                return;
            }
        }
    }

    let response_msg = if filter == crate::queue::AudioFilter::Off {
        get_lang().filter_disabled.to_string()
    } else {
        let fname = filter.name();
        fmt(get_lang().filter_set, &[&fname])
    };
    let _ = send_response(ctx, command, &response_msg, false).await;
}

pub async fn handle_autoplay(
    ctx: &Context,
    command: &CommandInteraction,
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

    let explicit_enable = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "enable")
        .and_then(|opt| match opt.value {
            CommandDataOptionValue::Boolean(b) => Some(b),
            _ => None,
        });

    let new_state = match explicit_enable {
        Some(enabled) => {
            queue_mgr.set_autoplay(guild_id, enabled).await;
            enabled
        }
        None => queue_mgr.toggle_autoplay(guild_id).await,
    };

    let msg = if new_state {
        get_lang().autoplay_enabled
    } else {
        get_lang().autoplay_disabled
    };

    let _ = send_response(ctx, command, msg, false).await;
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
        .field("⏩ `/seek <time>`", get_lang().help_seek, true)
        .field("🔀 `/shuffle`", get_lang().help_shuffle, true)
        .field("🔁 `/repeat <mode>`", get_lang().help_repeat, true)
        .field("🎛️ `/filter <mode>`", get_lang().help_filter, true)
        .field("📻 `/autoplay [enable]`", get_lang().help_autoplay, true)
        .field("📋 `/queue` | 📻 `/nowplaying`", get_lang().help_queue_nowplaying, true)
        .field("🗑️ `/remove <pos>` | 🗑️ `/clear`", get_lang().help_remove_clear, true)
        .field("⏭️ `/jump <pos>`", get_lang().help_jump, true)
        .field("🔊 `/volume <0-100>`", get_lang().help_volume, true)
        .field("⏹️ `/stop` | 👋 `/leave`", get_lang().help_stop_leave, true)
        .field("📝 `/lyrics [query]`", get_lang().help_lyrics, true)
        .field("📑 `/playlist <cmd>`", get_lang().help_playlist, true)
        .field("📜 `/history [clear]`", get_lang().help_history, true)
        .field("✨ `/recommend`", get_lang().cmd_recommend, true)
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

pub async fn handle_history(
    ctx: &Context,
    command: &CommandInteraction,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => {
            let _ = send_response(ctx, command, get_lang().server_only, true).await;
            return;
        }
    };

    let should_clear = command
        .data
        .options
        .iter()
        .any(|opt| opt.name == "clear" && opt.value.as_bool().unwrap_or(false));

    if should_clear {
        queue_mgr.clear_history(guild_id).await;
        let _ = send_response(ctx, command, get_lang().history_cleared, false).await;
        return;
    }

    let history = queue_mgr.get_history(guild_id).await;
    if history.is_empty() {
        let _ = send_response(ctx, command, get_lang().history_empty, false).await;
        return;
    }

    let mut desc = String::new();
    let total = history.len();
    let display_limit = total.min(15);

    for (i, track) in history.iter().rev().take(display_limit).enumerate() {
        let dur = format_duration(track.duration);
        let author = track.author.as_deref().unwrap_or("Unknown");
        desc.push_str(&format!(
            "{}. [{}]({}) — `{}` `[{}]`\n",
            i + 1,
            truncate(&track.title, 55),
            track.url,
            author,
            dur
        ));
    }

    if total > display_limit {
        desc.push_str(&format!("\n*...and {} more unique songs in log*", total - display_limit));
    }

    let footer_str = fmt(get_lang().history_footer, &[&total.to_string()]);

    let embed = CreateEmbed::new()
        .title(get_lang().history_title)
        .description(desc)
        .color(Color::from_rgb(88, 101, 242))
        .footer(serenity::all::CreateEmbedFooter::new(footer_str));

    let _ = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed),
            ),
        )
        .await;
}

pub fn build_recommend_view(
    tracks: &[crate::source::TrackMetadata],
    profile: &crate::source::TasteProfile,
    cache_key: &str,
    page: usize,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    const PAGE_SIZE: usize = 10;
    let total_tracks = tracks.len();
    let total_pages = ((total_tracks as f64) / (PAGE_SIZE as f64)).ceil() as usize;
    let total_pages = total_pages.max(1);
    let current_page = page.min(total_pages.saturating_sub(1));

    let start_idx = current_page * PAGE_SIZE;
    let end_idx = (start_idx + PAGE_SIZE).min(total_tracks);
    let page_tracks = &tracks[start_idx..end_idx];

    let mut desc = String::new();

    if current_page == 0 {
        desc.push_str(&format!(
            "**{}**\n> {}\n\n**{}**\n",
            get_lang().recommend_taste_header,
            profile.summary,
            get_lang().recommend_songs_header,
        ));
    } else {
        desc.push_str(&format!(
            "**{} (Page {}/{})**\n\n",
            get_lang().recommend_songs_header,
            current_page + 1,
            total_pages
        ));
    }

    let mut select_options = Vec::new();

    for (i, track) in page_tracks.iter().enumerate() {
        let global_idx = start_idx + i;
        let source_badge = match track.source.as_str() {
            "Spotify" => "🟢 `Spotify`",
            "SoundCloud" => "🟠 `SoundCloud`",
            _ => "🔴 `YouTube`",
        };

        let official_badge = if track.is_official { " ⭐ Official" } else { " 🎧 Community" };
        let author = track.author.as_deref().unwrap_or("Unknown Artist");
        let dur = format_duration(track.duration);

        desc.push_str(&format!(
            "{}. [{}]({}) — **{}** `[{}]` ↳ {}{}\n",
            global_idx + 1,
            truncate(&track.title, 45),
            track.url,
            truncate(author, 25),
            dur,
            source_badge,
            official_badge,
        ));

        let opt_label = format!("{}. {}", global_idx + 1, truncate(&track.title, 80));
        let opt_desc = format!("{} • {}{} • {}", truncate(author, 30), dur, official_badge, track.source);
        let opt_val = format!("{}:{}", cache_key, global_idx);

        select_options.push(
            CreateSelectMenuOption::new(opt_label, opt_val).description(opt_desc),
        );
    }

    let (base_footer, embed_color) = match profile.requested_platform {
        crate::source::PlatformTarget::Spotify => (
            "🟢 Platform: Spotify (Direct Match 100%)",
            Color::from_rgb(30, 215, 96),
        ),
        crate::source::PlatformTarget::SoundCloud => (
            "🟠 Platform: SoundCloud (Direct Match 100%)",
            Color::from_rgb(255, 85, 0),
        ),
        crate::source::PlatformTarget::YouTube => (
            "🔴 Platform: YouTube (Direct Match 100%)",
            Color::from_rgb(255, 0, 0),
        ),
        crate::source::PlatformTarget::Any => (
            "✨ Official First Cascade: 🔴 YouTube Official ➔ 🟢 Spotify Official ➔ 🟠 SoundCloud Official ➔ 🎧 Fallback",
            Color::from_rgb(88, 101, 242),
        ),
    };

    let footer_text = if total_pages > 1 {
        format!("Page {}/{} • Total {} tracks • {}", current_page + 1, total_pages, total_tracks, base_footer)
    } else {
        base_footer.to_string()
    };

    let embed = CreateEmbed::new()
        .title(get_lang().recommend_title)
        .description(desc)
        .color(embed_color)
        .footer(serenity::all::CreateEmbedFooter::new(footer_text));

    let max_selectable = (select_options.len() as u8).max(1);
    let select_menu = CreateSelectMenu::new(
        "recommend_select",
        CreateSelectMenuKind::String {
            options: select_options,
        },
    )
    .min_values(1)
    .max_values(max_selectable)
    .placeholder(get_lang().recommend_select_placeholder);

    let row1 = CreateActionRow::SelectMenu(select_menu);

    let mut buttons = Vec::new();
    if total_pages > 1 {
        buttons.push(
            CreateButton::new(format!("recommend_page:{}:{}", cache_key, current_page.saturating_sub(1)))
                .label("◀️ Prev")
                .style(ButtonStyle::Primary)
                .disabled(current_page == 0),
        );
        buttons.push(
            CreateButton::new("recommend_page_info")
                .label(format!("{}/{}", current_page + 1, total_pages))
                .style(ButtonStyle::Secondary)
                .disabled(true),
        );
        buttons.push(
            CreateButton::new(format!("recommend_page:{}:{}", cache_key, current_page + 1))
                .label("Next ▶️")
                .style(ButtonStyle::Primary)
                .disabled(current_page + 1 >= total_pages),
        );
    }

    let play_all_label = if total_tracks > 10 {
        format!("{} ({})", get_lang().recommend_play_all, total_tracks)
    } else {
        get_lang().recommend_play_all.to_string()
    };

    buttons.push(
        CreateButton::new(format!("recommend_all:{}", cache_key))
            .label(play_all_label)
            .style(ButtonStyle::Success),
    );

    let row2 = CreateActionRow::Buttons(buttons);

    (embed, vec![row1, row2])
}

pub async fn handle_recommend(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<crate::source::SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => {
            let _ = send_response(ctx, command, get_lang().server_only, true).await;
            return;
        }
    };

    let _ = command.defer(&ctx.http).await;

    let explicit_count = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "count")
        .and_then(|opt| match &opt.value {
            CommandDataOptionValue::Integer(i) => Some(*i as usize),
            _ => None,
        });

    let mood_raw = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "mood")
        .and_then(|opt| match &opt.value {
            CommandDataOptionValue::String(s) => Some(s.as_str().trim()),
            _ => None,
        })
        .filter(|s| !s.is_empty());

    let (_, natural_count, _) = mood_raw
        .map(crate::source::SourceManager::parse_platform_intent_and_count)
        .unwrap_or((crate::source::PlatformTarget::Any, None, String::new()));

    let target_count = explicit_count
        .or(natural_count)
        .unwrap_or(5)
        .clamp(1, 100);

    let history = queue_mgr.get_history(guild_id).await;
    let (mut profile, tracks) = source_mgr.get_recommendations(&history, target_count, mood_raw).await;

    if tracks.is_empty() {
        let _ = command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new().content(get_lang().recommend_empty),
            )
            .await;
        return;
    }

    if source_mgr.ai().is_enabled() {
        if let Some(first) = tracks.first() {
            let author = first.author.as_deref().unwrap_or("");
            if let Ok(t) = source_mgr.ai().get_trivia(&first.title, author).await {
                profile.summary.push_str(&format!("\n\n> {}", t));
            }
        }
    }

    let cache_key = format!("rec_{}_{}", guild_id.get(), command.id.get());
    queue_mgr.set_recommend_results(cache_key.clone(), tracks.clone(), profile.clone()).await;

    let (embed, components) = build_recommend_view(&tracks, &profile, &cache_key, 0);

    let _ = command
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new()
                .embed(embed)
                .components(components),
        )
        .await;
}
