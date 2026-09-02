use serenity::all::{
    CommandDataOptionValue, CommandInteraction, Context, CreateActionRow, CreateEmbed,
    CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponseFollowup, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption,
};
use songbird::events::{Event, TrackEvent};
use std::sync::Arc;
use tracing::error;

use super::events::TrackEndHandler;
use crate::lang::{fmt, get_lang};
use crate::queue::{LoopMode, QueueManager};
use crate::source::SourceManager;
use crate::utils::embed::{build_now_playing_embed, format_duration, source_color, source_icon_url};
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
            let _ = send_response(ctx, command, get_lang().server_only, false).await;
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
                let _ = send_response(ctx, command, get_lang().invalid_query, false).await;
                return;
            }
        },
        None => {
            let _ = send_response(ctx, command, get_lang().provide_query, false).await;
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
            let msg = fmt(get_lang().failed_connect_voice, &[&e]);
            let _ = send_followup(ctx, command, &msg).await;
            return;
        }
    };

    let requester_tag = format!("<@{}>", command.user.id);

    let is_url = query.starts_with("http://") || query.starts_with("https://");
    let is_spotify_url = query.contains("open.spotify.com")
        || query.starts_with("spotify:track")
        || query.starts_with("spotify:album")
        || query.starts_with("spotify:playlist");

    // Text query → show search results dropdown for user selection
    if !is_url && !is_spotify_url {
        let (platform_target, clean_query) = SourceManager::parse_platform_intent(&query);

        let search_res = match platform_target {
            crate::source::PlatformTarget::Spotify => source_mgr.search_spotify(&clean_query, 10).await,
            crate::source::PlatformTarget::SoundCloud => source_mgr.search_soundcloud(&clean_query, 10).await,
            crate::source::PlatformTarget::YouTube | crate::source::PlatformTarget::Any => source_mgr.search(&clean_query).await,
        };

        let results = match search_res {
            Ok(tracks) => tracks,
            Err(e) => {
                let msg = fmt(get_lang().could_not_find, &[&e]);
                let _ = send_followup(ctx, command, &msg).await;
                return;
            }
        };

        let options: Vec<CreateSelectMenuOption> = results
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let duration_str = track
                    .duration
                    .map(|d| {
                        let secs = d.as_secs();
                        format!("{}:{:02}", secs / 60, secs % 60)
                    })
                    .unwrap_or_else(|| "??:??".to_string());
                let label = truncate(&format!("{}. {}", i + 1, track.title), 95);
                let artist = track.author.as_deref().unwrap_or(get_lang().unknown);
                let desc = format!("{} • {}", artist, duration_str);
                CreateSelectMenuOption::new(label, i.to_string())
                    .description(truncate(&desc, 100))
            })
            .collect();

        let select_menu = CreateSelectMenu::new(
            "search_play",
            CreateSelectMenuKind::String { options },
        )
        .placeholder(get_lang().search_placeholder);
        let action_row = CreateActionRow::SelectMenu(select_menu);

        // Build clickable links list
        let links: String = results
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let artist = track.author.as_deref().unwrap_or(get_lang().unknown);
                let duration_str = track
                    .duration
                    .map(|d| {
                        let secs = d.as_secs();
                        format!("{}:{:02}", secs / 60, secs % 60)
                    })
                    .unwrap_or_else(|| "??:??".to_string());
                let views_str = track
                    .view_count
                    .map(|v| format!(" • {}", format_views(v)))
                    .unwrap_or_default();
                format!(
                    "**{}.** [{}]({})\n{} • {}{}",
                    i + 1,
                    truncate(&track.title, 80),
                    track.url,
                    artist,
                    duration_str,
                    views_str
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let embed = CreateEmbed::new()
            .title(fmt(get_lang().search_title, &[&query]))
            .description(fmt(get_lang().search_results_desc, &[&results.len(), &links]))
            .color(0x5865F2);

        if let Ok(msg) = command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .components(vec![action_row]),
            )
            .await
        {
            queue_mgr
                .set_search_results(msg.id, guild_id, results, false)
                .await;
        }
        return;
    }

    // URL/Spotify → resolve and play directly
    let mut resolved = match source_mgr.resolve(&query).await {
        Ok(tracks) => tracks,
        Err(e) => {
            let msg = fmt(get_lang().could_not_extract, &[&e]);
            let _ = send_followup(ctx, command, &msg).await;
            return;
        }
    };

    for track in &mut resolved {
        track.requester = Some(requester_tag.clone());
    }

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_currently_playing = handler.queue().current().is_some();

    queue_mgr.set_text_channel(guild_id, command.channel_id).await;

    if resolved.len() == 1 {
        let track = resolved[0].clone();
        queue_mgr.push_track(guild_id, track.clone()).await;

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

        if let Ok(msg) = command
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
    } else {
        // Playlist handling (instant enqueue without blocking!)
        let total_tracks = resolved.len();
        let source_name = resolved[0].source.clone();
        let first_track = resolved[0].clone();

        queue_mgr.push_playlist(guild_id, resolved.clone()).await;

        if !is_currently_playing {
            let filter = queue_mgr.get_filter(guild_id).await;
            let input = source_mgr
                .create_input_filtered(&first_track.stream_url, None, filter.ffmpeg_filter())
                .await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

            // Mark as current track and record ONLY the track that actually starts playing
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

        let queue_len = queue_mgr.get_queue(guild_id).await.len();

        let mut embed = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(fmt(get_lang().playlist_enqueued, &[&source_name]))
                    .icon_url(source_icon_url(&source_name)),
            )
            .title(fmt(get_lang().playlist_added, &[&total_tracks]))
            .field(get_lang().field_first_track, format!("[**{}**]({})", first_track.title, first_track.url), false)
            .field(get_lang().field_queue_total, fmt(get_lang().tracks_count, &[&queue_len]), true)
            .field(get_lang().field_requested_by_play, &requester_tag, true)
            .footer(
                CreateEmbedFooter::new(fmt(get_lang().platform_label, &[&source_name]))
                    .icon_url(source_icon_url(&source_name)),
            )
            .color(source_color(&source_name));

        if let Some(thumb) = &first_track.thumbnail {
            embed = embed.thumbnail(thumb);
        }

        if let Ok(msg) = command
            .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().embed(embed))
            .await
        {
            queue_mgr.set_last_message_id(guild_id, msg.id).await;
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

fn format_views(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B views", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M views", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K views", n as f64 / 1_000.0)
    } else {
        format!("{} views", n)
    }
}

/// /playnext — same as /play but inserts at queue position 1 (priority)
pub async fn handle_playnext(
    ctx: &Context,
    command: &CommandInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => {
            let _ = send_response(ctx, command, get_lang().server_only, false).await;
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

    let query = match command.data.options.iter().find(|opt| opt.name == "query") {
        Some(opt) => match &opt.value {
            CommandDataOptionValue::String(s) => s.clone(),
            _ => {
                let _ = send_response(ctx, command, get_lang().invalid_query, false).await;
                return;
            }
        },
        None => {
            let _ = send_response(ctx, command, get_lang().provide_query, false).await;
            return;
        }
    };

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
            let msg = fmt(get_lang().failed_connect_voice, &[&e]);
            let _ = send_followup(ctx, command, &msg).await;
            return;
        }
    };

    let requester_tag = format!("<@{}>", command.user.id);
    let is_url = query.starts_with("http://") || query.starts_with("https://");
    let is_spotify_url = query.contains("open.spotify.com")
        || query.starts_with("spotify:track")
        || query.starts_with("spotify:album")
        || query.starts_with("spotify:playlist");

    // Text query → show search results dropdown (same as /play but flagged as play_next)
    if !is_url && !is_spotify_url {
        let (platform_target, clean_query) = SourceManager::parse_platform_intent(&query);

        let search_res = match platform_target {
            crate::source::PlatformTarget::Spotify => source_mgr.search_spotify(&clean_query, 10).await,
            crate::source::PlatformTarget::SoundCloud => source_mgr.search_soundcloud(&clean_query, 10).await,
            crate::source::PlatformTarget::YouTube | crate::source::PlatformTarget::Any => source_mgr.search(&clean_query).await,
        };

        let results = match search_res {
            Ok(tracks) => tracks,
            Err(e) => {
                let msg = fmt(get_lang().could_not_find, &[&e]);
                let _ = send_followup(ctx, command, &msg).await;
                return;
            }
        };

        let options: Vec<CreateSelectMenuOption> = results
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let duration_str = track
                    .duration
                    .map(|d| {
                        let secs = d.as_secs();
                        format!("{}:{:02}", secs / 60, secs % 60)
                    })
                    .unwrap_or_else(|| "??:??".to_string());
                let label = truncate(&format!("{}. {}", i + 1, track.title), 95);
                let artist = track.author.as_deref().unwrap_or(get_lang().unknown);
                let desc = format!("{} • {}", artist, duration_str);
                CreateSelectMenuOption::new(label, i.to_string())
                    .description(truncate(&desc, 100))
            })
            .collect();

        let select_menu = CreateSelectMenu::new(
            "search_play",
            CreateSelectMenuKind::String { options },
        )
        .placeholder(get_lang().search_placeholder);
        let action_row = CreateActionRow::SelectMenu(select_menu);

        let links: String = results
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let artist = track.author.as_deref().unwrap_or(get_lang().unknown);
                let duration_str = track
                    .duration
                    .map(|d| {
                        let secs = d.as_secs();
                        format!("{}:{:02}", secs / 60, secs % 60)
                    })
                    .unwrap_or_else(|| "??:??".to_string());
                let views_str = track
                    .view_count
                    .map(|v| format!(" • {}", format_views(v)))
                    .unwrap_or_default();
                format!(
                    "**{}.** [{}]({})\n{} • {}{}",
                    i + 1,
                    truncate(&track.title, 80),
                    track.url,
                    artist,
                    duration_str,
                    views_str
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let embed = CreateEmbed::new()
            .title(fmt(get_lang().search_title, &[&query]))
            .description(fmt(get_lang().search_results_desc, &[&results.len(), &links]))
            .color(0x5865F2);

        if let Ok(msg) = command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .components(vec![action_row]),
            )
            .await
        {
            // Flag as play_next so dropdown selection uses push_next()
            queue_mgr
                .set_search_results(msg.id, guild_id, results, true)
                .await;
        }
        return;
    }

    // URL/Spotify → resolve and play_next directly
    let mut resolved = match source_mgr.resolve(&query).await {
        Ok(tracks) => tracks,
        Err(e) => {
            let msg = fmt(get_lang().could_not_extract, &[&e]);
            let _ = send_followup(ctx, command, &msg).await;
            return;
        }
    };

    for track in &mut resolved {
        track.requester = Some(requester_tag.clone());
    }

    let mut handler = call_lock.lock().await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_currently_playing = handler.queue().current().is_some();

    queue_mgr.set_text_channel(guild_id, command.channel_id).await;

    if resolved.len() == 1 {
        let track = resolved[0].clone();
        // Use push_next instead of push_track
        queue_mgr.push_next(guild_id, track.clone()).await;

        if !is_currently_playing {
            let filter = queue_mgr.get_filter(guild_id).await;
            let input = source_mgr
                .create_input_filtered(&track.stream_url, None, filter.ffmpeg_filter())
                .await;
            let track_handle = handler.enqueue_input(input).await;
            let _ = track_handle.set_volume(0.8);

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

        if let Ok(msg) = command
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
    } else {
        // Playlist → use push_next for first track, push_playlist for rest
        let total_tracks = resolved.len();
        let source_name = resolved[0].source.clone();
        let first_track = resolved[0].clone();

        // Insert all at position 1+ preserving order
        queue_mgr.push_next_playlist(guild_id, resolved.clone()).await;

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

        let queue_len = queue_mgr.get_queue(guild_id).await.len();

        let mut embed = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(fmt(get_lang().playlist_enqueued, &[&source_name]))
                    .icon_url(source_icon_url(&source_name)),
            )
            .title(fmt(get_lang().playlist_added, &[&total_tracks]))
            .color(source_color(&source_name));

        let mut fields = vec![];
        fields.push((get_lang().field_queue_total.to_string(), fmt(get_lang().tracks_count, &[&queue_len]), true));
        fields.push((get_lang().field_first_track.to_string(), format!("[{}]({})", first_track.title, first_track.url), true));
        if let Some(dur) = first_track.duration {
            fields.push((get_lang().field_duration.to_string(), format_duration(Some(dur)), true));
        }
        embed = embed.fields(fields);

        let _ = command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new().embed(embed),
            )
            .await;
    }
}
