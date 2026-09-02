use serenity::all::{
    ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind, Context,
    CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
    CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption,
};
use songbird::events::{Event, TrackEvent};
use std::sync::Arc;
use std::time::Duration;

use super::events::TrackEndHandler;
use crate::lang::{fmt, get_lang};
use crate::queue::{LoopMode, QueueManager};
use crate::source::{SourceManager, TrackMetadata};
use crate::utils::embed::{build_now_playing_embed, format_duration, source_color, source_emoji, source_icon_url};
use crate::utils::response::{send_followup, send_response};

const PAGE_SIZE: usize = 10;

pub fn build_queue_view(
    queue: &[TrackMetadata],
    loop_mode: LoopMode,
    is_shuffled: bool,
    page: usize,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let total_tracks = queue.len();
    if total_tracks == 0 {
        return (CreateEmbed::new().title(get_lang().queue_empty_title).description(get_lang().queue_empty_desc), vec![]);
    }
    let total_pages = ((total_tracks as f64) / (PAGE_SIZE as f64)).ceil() as usize;
    let total_pages = total_pages.max(1);
    let current_page = page.min(total_pages - 1);

    let start_idx = current_page * PAGE_SIZE;
    let end_idx = (start_idx + PAGE_SIZE).min(total_tracks);
    let page_tracks = &queue[start_idx..end_idx];

    let first_track = &queue[0];
    let total_duration: Duration = queue.iter().filter_map(|t| t.duration).sum();

    let mut desc = String::new();

    if current_page == 0 {
        // Page 1: Highlight Now Playing and show Up Next
        for (i, track) in page_tracks.iter().enumerate() {
            let dur = format_duration(track.duration);
            let prefix = match source_emoji(&track.source) {
                Some(e) => format!("{} ", e),
                None => String::new(),
            };
            let req_str = track.requester.as_deref().map(|r| format!(" • {}", r)).unwrap_or_default();

            if i == 0 {
                desc.push_str(&format!(
                    "{}{}[**{}**]({}) • `{}` (`{}`){}\n\n{}",
                    get_lang().now_playing_label, prefix, track.title, track.url, track.source, dur, req_str,
                    get_lang().up_next_label
                ));
            } else {
                desc.push_str(&format!(
                    "`{:02}.` {}[**{}**]({}) • `{}` (`{}`){}\n",
                    i, prefix, track.title, track.url, track.source, dur, req_str
                ));
            }
        }
    } else {
        // Page 2+: Show tracks in current page range
        desc.push_str(&fmt(get_lang().queue_page_header, &[&(current_page + 1), &total_pages]));
        for (i, track) in page_tracks.iter().enumerate() {
            let actual_idx = start_idx + i;
            let dur = format_duration(track.duration);
            let prefix = match source_emoji(&track.source) {
                Some(e) => format!("{} ", e),
                None => String::new(),
            };
            let req_str = track.requester.as_deref().map(|r| format!(" • {}", r)).unwrap_or_default();
            desc.push_str(&format!(
                "`{:02}.` {}[**{}**]({}) • `{}` (`{}`){}\n",
                actual_idx, prefix, track.title, track.url, track.source, dur, req_str
            ));
        }
    }

    if current_page == 0 && total_tracks > PAGE_SIZE {
        desc.push_str(&fmt(get_lang().more_tracks, &[&(total_tracks - PAGE_SIZE)]));
    }

    let shuffle_status_str = if is_shuffled { get_lang().shuffle_on } else { get_lang().shuffle_off };

    let mut embed = CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new(fmt(get_lang().queue_author_label, &[&first_track.source]))
                .icon_url(source_icon_url(&first_track.source))
                .url(&first_track.url),
        )
        .title(get_lang().queue_title)
        .description(desc)
        .field(get_lang().field_total_tracks, format!("{}", total_tracks), true)
        .field(get_lang().field_total_duration, format_duration(Some(total_duration)), true)
        .field(
            get_lang().field_repeat_mode,
            format!("{} {}", loop_mode.emoji(), loop_mode.as_str()),
            true,
        )
        .field(get_lang().field_random_mode, shuffle_status_str, true)
        .footer(
            CreateEmbedFooter::new(fmt(
                get_lang().queue_footer,
                &[&(current_page + 1), &total_pages, &first_track.source, &loop_mode.as_str(),
                  &if is_shuffled { get_lang().queue_footer_shuffle_active } else { get_lang().queue_footer_shuffle_inactive }]
            ))
            .icon_url(source_icon_url(&first_track.source)),
        )
        .color(source_color(&first_track.source));

    if let Some(thumb) = &first_track.thumbnail {
        embed = embed.thumbnail(thumb);
    }

    // Build Interactive Select Menu Options (Up to 25 tracks)
    let mut select_options = Vec::new();
    for (i, track) in page_tracks.iter().enumerate() {
        let actual_idx = start_idx + i;
        let dur = format_duration(track.duration);
        let mut title_label = format!("{}. {} ({})", actual_idx, track.title, dur);
        if title_label.chars().count() > 95 {
            title_label = title_label.chars().take(92).collect::<String>() + "...";
        }

        let desc_label = if actual_idx == 0 {
            fmt(get_lang().currently_playing, &[&track.source])
        } else {
            fmt(get_lang().queue_track_jump_desc, &[&actual_idx, &track.source])
        };

        select_options.push(
            CreateSelectMenuOption::new(title_label, format!("{}", actual_idx))
                .description(desc_label),
        );
    }

    let mut action_rows = Vec::new();

    if !select_options.is_empty() {
        let select_menu = CreateSelectMenu::new(
            "queue_jump",
            CreateSelectMenuKind::String {
                options: select_options,
            },
        )
        .placeholder(get_lang().queue_select_placeholder);

        action_rows.push(CreateActionRow::SelectMenu(select_menu));
    }

    // Direct Play Buttons for songs on the current page (Row 1: tracks 0..5, Row 2: tracks 5..10)
    let mut play_buttons_row1 = Vec::new();
    let mut play_buttons_row2 = Vec::new();

    for (i, _) in page_tracks.iter().enumerate() {
        let actual_idx = start_idx + i;
        let is_playing = actual_idx == 0;

        let label = if is_playing {
            get_lang().playing_indicator.to_string()
        } else {
            format!("▶️ #{:02}", actual_idx)
        };

        let btn = CreateButton::new(format!("queue_play:{}", actual_idx))
            .label(label)
            .style(if is_playing {
                ButtonStyle::Success
            } else {
                ButtonStyle::Primary
            })
            .disabled(is_playing);

        if i < 5 {
            play_buttons_row1.push(btn);
        } else if i < 10 {
            play_buttons_row2.push(btn);
        }
    }

    if !play_buttons_row1.is_empty() {
        action_rows.push(CreateActionRow::Buttons(play_buttons_row1));
    }
    if !play_buttons_row2.is_empty() {
        action_rows.push(CreateActionRow::Buttons(play_buttons_row2));
    }

    // Navigation and Control Buttons
    let prev_button = CreateButton::new(format!("queue_page:{}", current_page.saturating_sub(1)))
        .label(get_lang().btn_prev)
        .style(ButtonStyle::Primary)
        .disabled(current_page == 0);

    let indicator_button = CreateButton::new("queue_indicator")
        .label(format!("{}/{}", current_page + 1, total_pages))
        .style(ButtonStyle::Secondary)
        .disabled(true);

    let next_button = CreateButton::new(format!("queue_page:{}", current_page + 1))
        .label(get_lang().btn_next)
        .style(ButtonStyle::Primary)
        .disabled(current_page + 1 >= total_pages);

    let shuffle_button = CreateButton::new("queue_shuffle")
        .label(if is_shuffled { get_lang().btn_random_on } else { get_lang().btn_random_off })
        .style(if is_shuffled { ButtonStyle::Success } else { ButtonStyle::Secondary });

    let skip_button = CreateButton::new("queue_skip")
        .label(get_lang().btn_skip_queue)
        .style(ButtonStyle::Secondary);

    action_rows.push(CreateActionRow::Buttons(vec![
        prev_button,
        indicator_button,
        next_button,
        shuffle_button,
        skip_button,
    ]));

    (embed, action_rows)
}

pub async fn handle_queue(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let queue = queue_mgr.get_queue(guild_id).await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
    let is_shuffled = queue_mgr.get_shuffle(guild_id).await;

    if queue.is_empty() {
        let _ = send_response(ctx, command, get_lang().queue_empty_msg, true).await;
        return;
    }

    let (embed, components) = build_queue_view(&queue, loop_mode, is_shuffled, 0);

    let _ = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(components)
                    .ephemeral(true),
            ),
        )
        .await;
}

pub async fn handle_queue_component(
    ctx: &Context,
    component: &ComponentInteraction,
    source_mgr: &Arc<SourceManager>,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match component.guild_id {
        Some(id) => id,
        None => return,
    };

    let custom_id = component.data.custom_id.as_str();

    // If attempting to modify playback (play, jump, shuffle, skip, stop), require active voice channel
    if !custom_id.starts_with("queue_page:") && custom_id != "queue_indicator" {
        if let Err(msg) = crate::utils::voice::check_voice_channel(ctx, guild_id, component.user.id) {
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
    }

    if custom_id.starts_with("queue_page:") {
        let page: usize = custom_id
            .split(':')
            .nth(1)
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);

        let queue = queue_mgr.get_queue(guild_id).await;
        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
        let is_shuffled = queue_mgr.get_shuffle(guild_id).await;

        if queue.is_empty() {
            let _ = component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(get_lang().queue_empty_msg)
                            .embeds(vec![])
                            .components(vec![]),
                    ),
                )
                .await;
            return;
        }

        let (embed, components) = build_queue_view(&queue, loop_mode, is_shuffled, page);

        let _ = component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components),
                ),
            )
            .await;
    } else if custom_id == "queue_shuffle" {
        let is_shuffled = queue_mgr.toggle_shuffle(guild_id).await;
        let queue = queue_mgr.get_queue(guild_id).await;
        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;

        if queue.is_empty() {
            let _ = component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(get_lang().queue_empty_msg)
                            .embeds(vec![])
                            .components(vec![]),
                    ),
                )
                .await;
            return;
        }

        let (embed, components) = build_queue_view(&queue, loop_mode, is_shuffled, 0);

        let _ = component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components),
                ),
            )
            .await;
    } else if custom_id.starts_with("queue_play:") || custom_id == "queue_jump" {
        let target_idx = if custom_id.starts_with("queue_play:") {
            custom_id
                .split(':')
                .nth(1)
                .and_then(|p| p.parse::<usize>().ok())
        } else if let ComponentInteractionDataKind::StringSelect { values } = &component.data.kind {
            values.first().and_then(|val| val.parse::<usize>().ok())
        } else {
            None
        };

        if let Some(idx) = target_idx {
            if idx > 0 {
                // Defer BEFORE expensive create_input (yt-dlp + ffmpeg can take 2-8s)
                let _ = component.defer(&ctx.http).await;

                if let Some(target_track) = queue_mgr.jump_to(guild_id, idx).await {
                    let manager = songbird::get(ctx).await.unwrap();
                    if let Some(handler_lock) = manager.get(guild_id) {
                        let mut handler = handler_lock.lock().await;
                        // Arm latch BEFORE stop so the old track's End handler
                        // doesn't re-advance the rotated queue.
                        queue_mgr.set_skip_end(guild_id).await;
                        handler.queue().stop();
                        let filter = queue_mgr.get_filter(guild_id).await;
                        let input = source_mgr
                            .create_input_filtered(
                                &target_track.stream_url,
                                None,
                                filter.ffmpeg_filter(),
                            )
                            .await;
                        let track_handle = handler.enqueue_input(input).await;
                        let _ = track_handle.set_volume(0.8);

                        // Mark as current track so now-playing reports the jumped-to song
                        queue_mgr.set_current_track(guild_id, target_track.clone()).await;

                        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
                        if loop_mode == LoopMode::Track {
                            let _ = track_handle.enable_loop();
                        }

                        let _ = track_handle.add_event(
                            Event::Track(TrackEvent::End),
                            TrackEndHandler {
                                guild_id,
                                queue_mgr: queue_mgr.clone(),
                                source_mgr: source_mgr.clone(),
                                call_lock: handler_lock.clone(),
                                http: ctx.http.clone(),
                            },
                        );
                    }
                }
            }
        }

        let queue = queue_mgr.get_queue(guild_id).await;
        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
        let is_shuffled = queue_mgr.get_shuffle(guild_id).await;

        if queue.is_empty() {
            let _ = component
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .content(get_lang().queue_empty_msg)
                        .embeds(Vec::<CreateEmbed>::new())
                        .components(Vec::<CreateActionRow>::new()),
                )
                .await;
            return;
        }

        let (embed, components) = build_queue_view(&queue, loop_mode, is_shuffled, 0);

        let _ = component
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .components(components),
            )
            .await;
    } else if custom_id == "queue_skip" {
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

        // TrackEndHandler will advance the queue and send now-playing message
        let _ = component
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .content(get_lang().skipped_current),
            )
            .await;
    } else if custom_id == "queue_stop" {
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
}

pub async fn handle_nowplaying(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let _ = command.defer(&ctx.http).await;

    if let Some(current) = queue_mgr.get_current(guild_id).await {
        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
        let queue_len = queue_mgr.get_queue(guild_id).await.len();
        let upcoming_count = queue_len.saturating_sub(1);

        let manager = songbird::get(ctx).await.unwrap();
        let is_paused = if let Some(handler_lock) = manager.get(guild_id) {
            let handler = handler_lock.lock().await;
            if let Some(track_handle) = handler.queue().current() {
                matches!(track_handle.get_info().await, Ok(info) if info.playing == songbird::tracks::PlayMode::Pause)
            } else {
                false
            }
        } else {
            false
        };

        let (embed, action_row) = build_now_playing_embed(&current, upcoming_count, loop_mode, is_paused);

        let _ = command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .components(vec![action_row]),
            )
            .await;
    } else {
        let _ = send_followup(ctx, command, get_lang().nothing_playing).await;
    }
}
