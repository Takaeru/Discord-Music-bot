pub mod control;
pub mod events;
pub mod play;
pub mod queue;

use serenity::all::{
    CommandInteraction, CommandOptionType, ComponentInteraction, Context, CreateCommand,
    CreateCommandOption,
};
use std::sync::Arc;

use crate::queue::QueueManager;
use crate::source::SourceManager;
use crate::utils::response::send_response;

use self::control::{
    handle_clear, handle_help, handle_jump, handle_leave, handle_music_component, handle_pause,
    handle_ping, handle_remove, handle_repeat, handle_replay, handle_resume, handle_shuffle,
    handle_skip, handle_stop, handle_volume,
};
use self::play::handle_play;
use self::queue::{handle_nowplaying, handle_queue, handle_queue_component};

pub fn register_commands() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("play")
            .description("Play audio from YouTube, Spotify, SoundCloud, or search query")
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
        CreateCommand::new("replay").description("Replay the current track from the beginning"),
        CreateCommand::new("stop").description("Stop playback and clear the queue"),
        CreateCommand::new("queue").description("View the current music queue"),
        CreateCommand::new("nowplaying").description("Show details of the currently playing track"),
        CreateCommand::new("clear").description("Clear all upcoming tracks from the queue"),
        CreateCommand::new("remove")
            .description("Remove a specific track from queue by position")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "position",
                    "Position number of the track to remove (e.g. 1, 2, 3)",
                )
                .min_int_value(1)
                .required(true),
            ),
        CreateCommand::new("jump")
            .description("Jump directly to a song in the queue")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "position",
                    "Position number of the track to jump to",
                )
                .min_int_value(1)
                .required(true),
            ),
        CreateCommand::new("shuffle").description("Toggle random / shuffle mode for the queue"),
        CreateCommand::new("repeat")
            .description("Set repeat / loop mode (off, track, queue)")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "mode",
                    "Loop mode: off, track (1 song), or queue (entire list)",
                )
                .add_string_choice("off", "off")
                .add_string_choice("track (1 song)", "track")
                .add_string_choice("queue (all songs)", "queue")
                .required(true),
            ),
        CreateCommand::new("loop")
            .description("Set repeat / loop mode (off, track, queue)")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "mode",
                    "Loop mode: off, track (1 song), or queue (entire list)",
                )
                .add_string_choice("off", "off")
                .add_string_choice("track (1 song)", "track")
                .add_string_choice("queue (all songs)", "queue")
                .required(true),
            ),
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
        CreateCommand::new("ping").description("Check bot latency and audio pipeline status"),
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
        "replay" => handle_replay(ctx, command, source_mgr, queue_mgr).await,
        "stop" => handle_stop(ctx, command, queue_mgr).await,
        "queue" => handle_queue(ctx, command, queue_mgr).await,
        "nowplaying" => handle_nowplaying(ctx, command, queue_mgr).await,
        "clear" => handle_clear(ctx, command, queue_mgr).await,
        "remove" => handle_remove(ctx, command, queue_mgr).await,
        "jump" => handle_jump(ctx, command, source_mgr, queue_mgr).await,
        "shuffle" => handle_shuffle(ctx, command, queue_mgr).await,
        "repeat" | "loop" => handle_repeat(ctx, command, queue_mgr).await,
        "volume" => handle_volume(ctx, command).await,
        "leave" => handle_leave(ctx, command, queue_mgr).await,
        "ping" => handle_ping(ctx, command).await,
        "help" => handle_help(ctx, command).await,
        _ => {
            let _ = send_response(ctx, command, "⚠️ Unknown command.", false).await;
        }
    }
}

pub async fn handle_component(
    ctx: &Context,
    component: &ComponentInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let custom_id = component.data.custom_id.as_str();

    if custom_id.starts_with("queue_") {
        handle_queue_component(ctx, component, source_mgr, queue_mgr).await;
    } else if custom_id.starts_with("music_") {
        handle_music_component(ctx, component, source_mgr, queue_mgr).await;
    }
}
