use serenity::all::{
    Color, CommandDataOptionValue, CommandInteraction, Context, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use std::sync::Arc;

use crate::queue::{LoopMode, QueueManager};
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

pub async fn handle_help(ctx: &Context, command: &CommandInteraction) {
    let embed = CreateEmbed::new()
        .title("📖 Discord Music Bot - Help Guide")
        .description("Lightweight high-performance music bot powered by Rust & Songbird.")
        .field("🎵 `/play <query>`", "Play YouTube / Spotify / SoundCloud / search keywords", false)
        .field("⏸️ `/pause`", "Pause currently playing song", true)
        .field("▶️ `/resume`", "Resume paused playback", true)
        .field("⏭️ `/skip`", "Skip to the next song", true)
        .field("🔀 `/shuffle`", "Toggle random / shuffle mode on or off", true)
        .field("🔁 `/repeat <mode>`", "Repeat mode: `off`, `track` (1 song), or `queue`", true)
        .field("⏹️ `/stop`", "Stop music and clear queue", true)
        .field("📋 `/queue`", "View interactive song list & controls (private)", true)
        .field("📻 `/nowplaying`", "Show currently playing track info", true)
        .field("🔊 `/volume <0-100>`", "Set volume level", true)
        .field("👋 `/leave`", "Disconnect bot from voice", true)
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
