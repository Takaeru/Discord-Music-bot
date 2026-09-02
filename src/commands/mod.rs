pub mod control;
pub mod events;
pub mod lyrics;
pub mod play;
pub mod playlist;
pub mod queue;

use serenity::all::{
    CommandInteraction, CommandOptionType, ComponentInteraction, Context, CreateCommand,
    CreateCommandOption,
};
use std::sync::Arc;

use crate::lang::{fmt, get_lang};
use crate::playlist::PlaylistStore;
use crate::queue::QueueManager;
use crate::source::SourceManager;
use crate::utils::response::send_response;

use self::control::{
    handle_autoplay, handle_clear, handle_filter, handle_help, handle_history, handle_jump,
    handle_leave, handle_music_component, handle_pause, handle_ping, handle_recommend, handle_remove,
    handle_repeat, handle_replay, handle_resume, handle_seek, handle_shuffle, handle_skip,
    handle_stop, handle_volume,
};
use self::lyrics::handle_lyrics;
use self::play::{handle_play, handle_playnext};
use self::playlist::handle_playlist;
use self::queue::{handle_nowplaying, handle_queue, handle_queue_component};

pub fn register_commands() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("play")
            .description(get_lang().cmd_play)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "query",
                    "Song name or URL to play",
                )
                .required(true),
            ),
        CreateCommand::new("pause").description(get_lang().cmd_pause),
        CreateCommand::new("resume").description(get_lang().cmd_resume),
        CreateCommand::new("skip").description(get_lang().cmd_skip),
        CreateCommand::new("replay").description(get_lang().cmd_replay),
        CreateCommand::new("stop").description(get_lang().cmd_stop),
        CreateCommand::new("queue").description(get_lang().cmd_queue),
        CreateCommand::new("nowplaying").description(get_lang().cmd_nowplaying),
        CreateCommand::new("clear").description(get_lang().cmd_clear),
        CreateCommand::new("remove")
            .description(get_lang().cmd_remove)
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
            .description(get_lang().cmd_jump)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "position",
                    "Position number of the track to jump to",
                )
                .min_int_value(1)
                .required(true),
            ),
        CreateCommand::new("shuffle").description(get_lang().cmd_shuffle),
        CreateCommand::new("repeat")
            .description(get_lang().cmd_repeat)
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
            .description(get_lang().cmd_repeat)
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
            .description(get_lang().cmd_volume)
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
        CreateCommand::new("playnext")
            .description(get_lang().cmd_playnext)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "query",
                    "Song name or URL to play next",
                )
                .required(true),
            ),
        CreateCommand::new("seek")
            .description(get_lang().cmd_seek)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "time",
                    "Timestamp to seek to (e.g. 1:30 or 90)",
                )
                .required(true),
            ),
        CreateCommand::new("lyrics")
            .description(get_lang().cmd_lyrics)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "query",
                    "Song title to search lyrics for (defaults to currently playing)",
                )
                .required(false),
            ),
        CreateCommand::new("filter")
            .description(get_lang().cmd_filter)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "mode",
                    "Audio filter: off, bassboost, nightcore, vaporwave, 8d, or karaoke",
                )
                .add_string_choice("off (disable filter)", "off")
                .add_string_choice("bassboost (deep bass)", "bassboost")
                .add_string_choice("nightcore (high pitch & fast)", "nightcore")
                .add_string_choice("vaporwave (slowed & relaxed)", "vaporwave")
                .add_string_choice("8d (surround rotating audio)", "8d")
                .add_string_choice("karaoke (vocal reduction)", "karaoke")
                .required(true),
            ),
        CreateCommand::new("autoplay")
            .description(get_lang().cmd_autoplay)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Boolean,
                    "enable",
                    "Enable or disable autoplay (optional, toggles if omitted)",
                )
                .required(false),
            ),
        CreateCommand::new("playlist")
            .description(get_lang().cmd_playlist)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "save",
                    "Save current music queue as a personal playlist",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "name",
                        "Name of the playlist to save",
                    )
                    .required(true),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "load",
                    "Load a saved personal playlist into queue",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "name",
                        "Name of the playlist to load",
                    )
                    .required(true),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "list",
                    "List all your saved personal playlists",
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "show",
                    "Show tracks in a personal playlist",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "name",
                        "Name of the playlist to inspect",
                    )
                    .required(true),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "delete",
                    "Delete a saved personal playlist",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "name",
                        "Name of the playlist to delete",
                    )
                    .required(true),
                ),
            ),
        CreateCommand::new("history")
            .description(get_lang().cmd_history)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Boolean,
                    "clear",
                    "Clear the playback history log (optional)",
                )
                .required(false),
            ),
        CreateCommand::new("recommend")
            .description(get_lang().cmd_recommend)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "mood",
                    "Custom vibe, mood, or platform (e.g. '100 lagu yui dari spotify', 'cyberpunk', 'lo-fi')",
                )
                .required(false),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "count",
                    "Number of recommendations to generate (1 - 100, default: 5)",
                )
                .min_int_value(1)
                .max_int_value(100)
                .required(false),
            ),
        CreateCommand::new("recommendation")
            .description("Music AI Skill: Discover curated recommendations matching your mood or request")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "mood",
                    "Natural-language prompt (e.g. 'lagu Jepang buat malam hujan', 'mirip ini')",
                )
                .required(true),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "count",
                    "Number of recommendations to generate (1 - 100, default: 5)",
                )
                .min_int_value(1)
                .max_int_value(100)
                .required(false),
            ),
        CreateCommand::new("leave").description(get_lang().cmd_leave),
        CreateCommand::new("ping").description(get_lang().cmd_ping),
        CreateCommand::new("help").description(get_lang().cmd_help),
    ]
}

pub async fn handle_command(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
    playlist_store: &Arc<PlaylistStore>,
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
        "playnext" => handle_playnext(ctx, command, source_mgr, queue_mgr).await,
        "seek" => handle_seek(ctx, command, source_mgr, queue_mgr).await,
        "lyrics" => handle_lyrics(ctx, command, queue_mgr).await,
        "filter" => handle_filter(ctx, command, source_mgr, queue_mgr).await,
        "autoplay" => handle_autoplay(ctx, command, queue_mgr).await,
        "playlist" => handle_playlist(ctx, command, source_mgr, queue_mgr, playlist_store).await,
        "history" => handle_history(ctx, command, queue_mgr).await,
        "recommend" | "recommendation" => handle_recommend(ctx, command, source_mgr, queue_mgr).await,
        "leave" => handle_leave(ctx, command, queue_mgr).await,
        "ping" => handle_ping(ctx, command).await,
        "help" => handle_help(ctx, command).await,
        _ => {
            let _ = send_response(ctx, command, get_lang().unknown_command, false).await;
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
    } else if custom_id == "search_play" {
        handle_search_play(ctx, component, source_mgr, queue_mgr).await;
    } else if custom_id == "recommend_select" {
        handle_recommend_select(ctx, component, source_mgr, queue_mgr).await;
    } else if custom_id.starts_with("recommend_all:") {
        handle_recommend_all(ctx, component, source_mgr, queue_mgr).await;
    } else if custom_id.starts_with("recommend_page:") {
        handle_recommend_page(ctx, component, queue_mgr).await;
    }
}

async fn handle_search_play(
    ctx: &Context,
    component: &ComponentInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    use serenity::all::{ComponentInteractionDataKind, CreateInteractionResponseFollowup};
    use songbird::events::{Event, TrackEvent};

    use crate::commands::events::TrackEndHandler;
    use crate::queue::LoopMode;
    use crate::utils::embed::build_now_playing_embed;
    use crate::utils::voice::check_voice_channel;

    let guild_id = match component.guild_id {
        Some(id) => id,
        None => return,
    };

    // Voice channel check — must be in VC to select a track
    if let Err(msg) = check_voice_channel(ctx, guild_id, component.user.id) {
        let _ = component
            .create_response(
                &ctx.http,
                serenity::all::CreateInteractionResponse::Message(
                    serenity::all::CreateInteractionResponseMessage::new()
                        .content(msg)
                        .ephemeral(true),
                ),
            )
            .await;
        return;
    }

    let selected_index = match &component.data.kind {
        ComponentInteractionDataKind::StringSelect { values } => {
            values.first().and_then(|v| v.parse::<usize>().ok())
        }
        _ => None,
    };

    let idx = match selected_index {
        Some(i) => i,
        None => return,
    };

    let results = queue_mgr.get_search_results(component.message.id).await;
    let track = match results.get(idx) {
        Some(t) => t.clone(),
        None => {
            let _ = component
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new()
                            .content(get_lang().selection_expired)
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }
    };

    // Check if this was a /playnext search
    let is_play_next = queue_mgr.is_search_play_next(component.message.id).await;

    // Consume search results to prevent double-selection
    queue_mgr.remove_search_results(component.message.id).await;

    // Defer before expensive create_input
    let _ = component.defer(&ctx.http).await;

    let manager = songbird::get(ctx).await.unwrap();
    let call_lock = match manager.get(guild_id) {
        Some(lock) => lock,
        None => {
            let _ = component
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .content(get_lang().not_connected_vc),
                )
                .await;
            return;
        }
    };

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_currently_playing = handler.queue().current().is_some();

    let mut track = track;
    track.requester = Some(format!("<@{}>", component.user.id));
    if is_play_next {
        queue_mgr.push_next(guild_id, track.clone()).await;
    } else {
        queue_mgr.push_track(guild_id, track.clone()).await;
    }
    queue_mgr.set_text_channel(guild_id, component.channel_id).await;

    if !is_currently_playing {
        let filter = queue_mgr.get_filter(guild_id).await;
        let input = source_mgr
            .create_input_filtered(&track.stream_url, None, filter.ffmpeg_filter())
            .await;
        let track_handle = handler.enqueue_input(input).await;
        let _ = track_handle.set_volume(0.8);

        // Mark as current track and record to history because it started playing!
        queue_mgr.set_current_track(guild_id, track.clone()).await;
        queue_mgr.push_history(guild_id, track.clone()).await;

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
                http: ctx.http.clone(),
            },
        );
    }

    let queue_len = queue_mgr.get_queue(guild_id).await.len();
    let upcoming_count = queue_len.saturating_sub(1);
    let (embed, action_row) = build_now_playing_embed(&track, upcoming_count, loop_mode, false);

    if let Ok(msg) = component
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new()
                .embed(embed)
                .components(vec![action_row]),
        )
        .await
    {
        queue_mgr.set_last_message_id(guild_id, msg.id).await;
    }
}

async fn handle_recommend_select(
    ctx: &Context,
    component: &ComponentInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    use serenity::all::{ComponentInteractionDataKind, CreateInteractionResponseFollowup};
    use songbird::events::{Event, TrackEvent};
    use crate::commands::events::TrackEndHandler;
    use crate::queue::LoopMode;
    use crate::utils::voice::check_voice_channel;

    let guild_id = match component.guild_id {
        Some(id) => id,
        None => return,
    };

    let connect_to = match check_voice_channel(ctx, guild_id, component.user.id) {
        Ok(c) => c,
        Err(msg) => {
            let _ = component
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new()
                            .content(msg)
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }
    };

    let (cache_key, indices) = match &component.data.kind {
        ComponentInteractionDataKind::StringSelect { values } => {
            let mut key = String::new();
            let mut idxs = Vec::new();
            for val in values {
                let mut parts = val.splitn(2, ':');
                if let (Some(k), Some(i_str)) = (parts.next(), parts.next()) {
                    if key.is_empty() {
                        key = k.to_string();
                    }
                    if let Ok(i) = i_str.parse::<usize>() {
                        idxs.push(i);
                    }
                }
            }
            (key, idxs)
        }
        _ => return,
    };

    if indices.is_empty() {
        return;
    }

    let tracks = match queue_mgr.get_recommend_results(&cache_key).await {
        Some(t) => t,
        None => {
            let _ = component
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new()
                            .content(get_lang().selection_expired)
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }
    };

    let mut selected_tracks = Vec::new();
    let user_tag = format!("<@{}>", component.user.id);
    for idx in indices {
        if let Some(t) = tracks.get(idx) {
            let mut track = t.clone();
            track.requester = Some(user_tag.clone());
            selected_tracks.push(track);
        }
    }

    if selected_tracks.is_empty() {
        return;
    }

    let _ = component.defer(&ctx.http).await;

    let manager = songbird::get(ctx).await.unwrap();
    let call_lock = match manager.get(guild_id) {
        Some(lock) => lock,
        None => match manager.join(guild_id, connect_to).await {
            Ok(lock) => lock,
            Err(e) => {
                let err_str = format!("{:?}", e);
                let err_msg = fmt(get_lang().failed_connect_voice, &[&err_str]);
                let _ = component
                    .create_followup(
                        &ctx.http,
                        CreateInteractionResponseFollowup::new().content(err_msg),
                    )
                    .await;
                return;
            }
        },
    };

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_currently_playing = handler.queue().current().is_some();

    for track in &selected_tracks {
        queue_mgr.push_track(guild_id, track.clone()).await;
    }
    queue_mgr.set_text_channel(guild_id, component.channel_id).await;

    if !is_currently_playing {
        let first = selected_tracks[0].clone();
        let filter = queue_mgr.get_filter(guild_id).await;
        let input = source_mgr
            .create_input_filtered(&first.stream_url, None, filter.ffmpeg_filter())
            .await;
        let track_handle = handler.enqueue_input(input).await;
        let _ = track_handle.set_volume(0.8);

        queue_mgr.set_current_track(guild_id, first.clone()).await;
        queue_mgr.push_history(guild_id, first.clone()).await;

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
                http: ctx.http.clone(),
            },
        );
    }

    let confirm_msg = if selected_tracks.len() == 1 {
        fmt(get_lang().recommend_enqueued_one, &[&selected_tracks[0].title])
    } else {
        fmt(get_lang().recommend_enqueued_all, &[&selected_tracks.len().to_string()])
    };

    let _ = component
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new()
                .content(confirm_msg)
                .ephemeral(true),
        )
        .await;
}

async fn handle_recommend_all(
    ctx: &Context,
    component: &ComponentInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    use serenity::all::CreateInteractionResponseFollowup;
    use songbird::events::{Event, TrackEvent};
    use crate::commands::events::TrackEndHandler;
    use crate::queue::LoopMode;
    use crate::utils::voice::check_voice_channel;

    let guild_id = match component.guild_id {
        Some(id) => id,
        None => return,
    };

    let connect_to = match check_voice_channel(ctx, guild_id, component.user.id) {
        Ok(c) => c,
        Err(msg) => {
            let _ = component
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new()
                            .content(msg)
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }
    };

    let cache_key = component
        .data
        .custom_id
        .strip_prefix("recommend_all:")
        .unwrap_or("");

    let tracks = match queue_mgr.get_recommend_results(cache_key).await {
        Some(t) if !t.is_empty() => t,
        _ => {
            let _ = component
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new()
                            .content(get_lang().selection_expired)
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }
    };

    let _ = component.defer(&ctx.http).await;

    let manager = songbird::get(ctx).await.unwrap();
    let call_lock = match manager.get(guild_id) {
        Some(lock) => lock,
        None => match manager.join(guild_id, connect_to).await {
            Ok(lock) => lock,
            Err(e) => {
                let err_str = format!("{:?}", e);
                let err_msg = fmt(get_lang().failed_connect_voice, &[&err_str]);
                let _ = component
                    .create_followup(
                        &ctx.http,
                        CreateInteractionResponseFollowup::new().content(err_msg),
                    )
                    .await;
                return;
            }
        },
    };

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_currently_playing = handler.queue().current().is_some();

    let user_tag = format!("<@{}>", component.user.id);
    let mut stamped_tracks = Vec::new();
    for mut t in tracks {
        t.requester = Some(user_tag.clone());
        stamped_tracks.push(t);
    }

    let first_track = stamped_tracks[0].clone();
    let count_str = stamped_tracks.len().to_string();

    queue_mgr.push_playlist(guild_id, stamped_tracks).await;
    queue_mgr.set_text_channel(guild_id, component.channel_id).await;

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
            Event::Track(TrackEvent::End),
            TrackEndHandler {
                guild_id,
                queue_mgr: queue_mgr.clone(),
                source_mgr: source_mgr.clone(),
                call_lock: call_lock.clone(),
                http: ctx.http.clone(),
            },
        );
    }

    let msg = fmt(get_lang().recommend_enqueued_all, &[&count_str]);
    let _ = component
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new().content(msg),
        )
        .await;
}

async fn handle_recommend_page(
    ctx: &Context,
    component: &ComponentInteraction,
    queue_mgr: &Arc<QueueManager>,
) {
    let custom_id = component.data.custom_id.as_str();
    let parts: Vec<&str> = custom_id.split(':').collect();
    if parts.len() < 3 {
        return;
    }
    let cache_key = parts[1];
    let page = parts[2].parse::<usize>().unwrap_or(0);

    let entry = match queue_mgr.get_recommend_entry(cache_key).await {
        Some(e) => e,
        None => {
            let _ = component
                .create_response(
                    &ctx.http,
                    serenity::all::CreateInteractionResponse::Message(
                        serenity::all::CreateInteractionResponseMessage::new()
                            .content(get_lang().selection_expired)
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }
    };

    let (embed, components) = crate::commands::control::build_recommend_view(
        &entry.tracks,
        &entry.profile,
        cache_key,
        page,
    );

    let _ = component
        .create_response(
            &ctx.http,
            serenity::all::CreateInteractionResponse::UpdateMessage(
                serenity::all::CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(components),
            ),
        )
        .await;
}
