use serenity::all::{
    Color, CommandDataOptionValue, CommandInteraction, Context, CreateCommand,
    CreateCommandOption, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateInteractionResponseFollowup,
    CommandOptionType,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::error;

use crate::queue::QueueManager;
use crate::source::SourceManager;

pub fn register_commands() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("play")
            .description("Play audio from YouTube, SoundCloud, Spotify, or search query")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "query",
                    "Song name or URL to play",
                )
                .required(true),
            ),
        CreateCommand::new("pause").description("Pause the currently playing track"),
        CreateCommand::new("resume").description("Resume playback of the paused track"),
        CreateCommand::new("skip").description("Skip the current track and play the next in queue"),
        CreateCommand::new("stop").description("Stop playback and clear the queue"),
        CreateCommand::new("queue").description("View the current music queue"),
        CreateCommand::new("nowplaying").description("Show details of the currently playing track"),
        CreateCommand::new("volume")
            .description("Adjust playback volume (0 - 100)")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "level",
                    "Volume level between 0 and 100",
                )
                .min_int_value(0)
                .max_int_value(100)
                .required(true),
            ),
        CreateCommand::new("leave").description("Disconnect the bot from the voice channel"),
        CreateCommand::new("help").description("Show available music commands"),
    ]
}

pub async fn handle_command(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let cmd_name = command.data.name.as_str();

    match cmd_name {
        "play" => handle_play(ctx, command, source_mgr, queue_mgr).await,
        "pause" => handle_pause(ctx, command).await,
        "resume" => handle_resume(ctx, command).await,
        "skip" => handle_skip(ctx, command, queue_mgr).await,
        "stop" => handle_stop(ctx, command, queue_mgr).await,
        "queue" => handle_queue(ctx, command, queue_mgr).await,
        "nowplaying" => handle_nowplaying(ctx, command, queue_mgr).await,
        "volume" => handle_volume(ctx, command).await,
        "leave" => handle_leave(ctx, command, queue_mgr).await,
        "help" => handle_help(ctx, command).await,
        _ => {
            let _ = send_response(ctx, command, "⚠️ Unknown command.", false).await;
        }
    }
}

async fn handle_play(
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

    if resolved.len() == 1 {
        let track = resolved[0].clone();
        queue_mgr.push_track(guild_id, track.clone()).await;

        let input = source_mgr.create_input(&track.url);
        let track_handle = handler.enqueue_input(input).await;
        let _ = track_handle.set_volume(0.8);

        let duration_str = format_duration(track.duration);
        let author_str = track.author.as_deref().unwrap_or("Unknown Artist");

        let mut embed = CreateEmbed::new()
            .title("🎶 Added to Queue")
            .description(format!("[**{}**]({})", track.title, track.url))
            .field("👤 Artist", author_str, true)
            .field("⏱️ Duration", duration_str, true)
            .field("📌 Position", format!("#{}", handler.queue().len()), true)
            .color(Color::from_rgb(88, 101, 242));

        if let Some(thumb) = &track.thumbnail {
            embed = embed.thumbnail(thumb);
        }

        let _ = command
            .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().embed(embed))
            .await;
    } else {
        // Playlist handling
        let total_tracks = resolved.len();
        queue_mgr.push_playlist(guild_id, resolved.clone()).await;

        for track in &resolved {
            let input = source_mgr.create_input(&track.url);
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);
        }

        let embed = CreateEmbed::new()
            .title("📑 Playlist Enqueued")
            .description(format!("Successfully enqueued **{}** tracks from playlist.", total_tracks))
            .field("📌 First Track", format!("[**{}**]({})", resolved[0].title, resolved[0].url), false)
            .field("📊 Queue Total", format!("{} tracks", handler.queue().len()), true)
            .color(Color::from_rgb(59, 165, 93));

        let _ = command
            .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().embed(embed))
            .await;
    }
}

async fn handle_pause(ctx: &Context, command: &CommandInteraction) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.pause();
            let _ = send_response(ctx, command, "⏸️ Playback paused.", false).await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is currently playing.", false).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", false).await;
    }
}

async fn handle_resume(ctx: &Context, command: &CommandInteraction) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.play();
            let _ = send_response(ctx, command, "▶️ Playback resumed.", false).await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is currently playing.", false).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", false).await;
    }
}

async fn handle_skip(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.stop();
            queue_mgr.advance(guild_id).await;
            let _ = send_response(ctx, command, "⏭️ Skipped current track.", false).await;
        } else {
            let _ = send_response(ctx, command, "⚠️ Nothing is playing to skip.", false).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", false).await;
    }
}

async fn handle_stop(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();
        queue_mgr.clear(guild_id).await;
        let _ = send_response(ctx, command, "⏹️ Playback stopped and queue cleared.", false).await;
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not connected to a voice channel.", false).await;
    }
}

async fn handle_queue(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let queue = queue_mgr.get_queue(guild_id).await;

    if queue.is_empty() {
        let _ = send_response(ctx, command, "📭 The queue is currently empty.", false).await;
        return;
    }

    let mut desc = String::new();
    for (i, track) in queue.iter().take(10).enumerate() {
        let pos = if i == 0 { "▶️ [Now Playing]".to_string() } else { format!("{}.", i) };
        let dur = format_duration(track.duration);
        desc.push_str(&format!("`{}` **{}** (`{}`)\n", pos, track.title, dur));
    }

    if queue.len() > 10 {
        desc.push_str(&format!("\n*...and {} more tracks*", queue.len() - 10));
    }

    let embed = CreateEmbed::new()
        .title("📋 Current Queue")
        .description(desc)
        .field("📊 Total Tracks", format!("{}", queue.len()), true)
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

async fn handle_nowplaying(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    if let Some(current) = queue_mgr.get_current(guild_id).await {
        let author = current.author.as_deref().unwrap_or("Unknown Artist");
        let dur = format_duration(current.duration);

        let mut embed = CreateEmbed::new()
            .title("📻 Now Playing")
            .description(format!("[**{}**]({})", current.title, current.url))
            .field("👤 Artist", author, true)
            .field("⏱️ Duration", dur, true)
            .color(Color::from_rgb(235, 69, 158));

        if let Some(thumb) = &current.thumbnail {
            embed = embed.thumbnail(thumb);
        }

        let _ = command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed),
                ),
            )
            .await;
    } else {
        let _ = send_response(ctx, command, "⚠️ Nothing is currently playing.", false).await;
    }
}

async fn handle_volume(ctx: &Context, command: &CommandInteraction) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

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
            let _ = send_response(ctx, command, "⚠️ Nothing is playing right now.", false).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not in a voice channel.", false).await;
    }
}

async fn handle_leave(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let manager = songbird::get(ctx).await.unwrap();
    if manager.get(guild_id).is_some() {
        if let Err(e) = manager.leave(guild_id).await {
            let _ = send_response(ctx, command, &format!("❌ Failed to leave voice: {:?}", e), false).await;
        } else {
            queue_mgr.clear(guild_id).await;
            let _ = send_response(ctx, command, "👋 Disconnected from voice channel.", false).await;
        }
    } else {
        let _ = send_response(ctx, command, "⚠️ Bot is not in a voice channel.", false).await;
    }
}

async fn handle_help(ctx: &Context, command: &CommandInteraction) {
    let embed = CreateEmbed::new()
        .title("📖 Discord Music Bot - Help Guide")
        .description("Lightweight high-performance music bot powered by Rust & Songbird.")
        .field("🎵 `/play <query>`", "Play YouTube / Spotify / SoundCloud / search keywords", false)
        .field("⏸️ `/pause`", "Pause currently playing song", true)
        .field("▶️ `/resume`", "Resume paused playback", true)
        .field("⏭️ `/skip`", "Skip to the next song", true)
        .field("⏹️ `/stop`", "Stop music and clear queue", true)
        .field("📋 `/queue`", "View current song list", true)
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

fn format_duration(dur: Option<Duration>) -> String {
    match dur {
        Some(d) => {
            let secs = d.as_secs();
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let rem_secs = secs % 60;
            if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, mins, rem_secs)
            } else {
                format!("{:02}:{:02}", mins, rem_secs)
            }
        }
        None => "--:--".to_string(),
    }
}

async fn send_response(
    ctx: &Context,
    command: &CommandInteraction,
    content: &str,
    ephemeral: bool,
) -> Result<(), serenity::Error> {
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(ephemeral),
            ),
        )
        .await
}

async fn send_followup(
    ctx: &Context,
    command: &CommandInteraction,
    content: &str,
) -> Result<(), serenity::Error> {
    command
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new().content(content),
        )
        .await
        .map(|_| ())
}
