use serenity::all::{
    ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind, Context,
    CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption,
};
use songbird::events::{Event, TrackEvent};
use std::sync::Arc;
use std::time::Duration;

use super::events::TrackEndHandler;
use crate::queue::{LoopMode, QueueManager};
use crate::source::{SourceManager, TrackMetadata};
use crate::utils::embed::{format_duration, source_color, source_emoji, source_icon_url};
use crate::utils::response::send_response;

const PAGE_SIZE: usize = 10;

pub fn build_queue_view(
    queue: &[TrackMetadata],
    loop_mode: LoopMode,
    is_shuffled: bool,
    page: usize,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let total_tracks = queue.len();
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

            if i == 0 {
                desc.push_str(&format!(
                    "**▶️ Now Playing:**\n{}[**{}**]({}) • `{}` (`{}`)\n\n**Up Next:**\n",
                    prefix, track.title, track.url, track.source, dur
                ));
            } else {
                desc.push_str(&format!(
                    "`{:02}.` {}[**{}**]({}) • `{}` (`{}`)\n",
                    i, prefix, track.title, track.url, track.source, dur
                ));
            }
        }
    } else {
        // Page 2+: Show tracks in current page range
        desc.push_str(&format!("**📋 Queue (Page {}/{}):**\n", current_page + 1, total_pages));
        for (i, track) in page_tracks.iter().enumerate() {
            let actual_idx = start_idx + i;
            let dur = format_duration(track.duration);
            let prefix = match source_emoji(&track.source) {
                Some(e) => format!("{} ", e),
                None => String::new(),
            };
            desc.push_str(&format!(
                "`{:02}.` {}[**{}**]({}) • `{}` (`{}`)\n",
                actual_idx, prefix, track.title, track.url, track.source, dur
            ));
        }
    }

    if current_page == 0 && total_tracks > PAGE_SIZE {
        desc.push_str(&format!("\n*...and {} more tracks in queue*", total_tracks - PAGE_SIZE));
    }

    let shuffle_status_str = if is_shuffled { "🔀 Shuffled (On)" } else { "➡️ Sequential (Off)" };

    let mut embed = CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new(format!("Current Music Queue ({})", first_track.source))
                .icon_url(source_icon_url(&first_track.source))
                .url(&first_track.url),
        )
        .title("📋 Music Queue")
        .description(desc)
        .field("📊 Total Tracks", format!("{}", total_tracks), true)
        .field("⏱️ Total Duration", format_duration(Some(total_duration)), true)
        .field(
            "🔁 Repeat Mode",
            format!("{} {}", loop_mode.emoji(), loop_mode.as_str()),
            true,
        )
        .field("🔀 Random Mode", shuffle_status_str, true)
        .footer(
            CreateEmbedFooter::new(format!(
                "Page {}/{} | Platform: {} | Loop: {} | Random: {}",
                current_page + 1,
                total_pages,
                first_track.source,
                loop_mode.as_str(),
                if is_shuffled { "On" } else { "Off" }
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
            format!("▶️ Currently Playing ({})", track.source)
        } else {
            format!("Jump to track #{} ({})", actual_idx, track.source)
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
        .placeholder("🎵 Choose a song from the list to jump & play directly...");

        action_rows.push(CreateActionRow::SelectMenu(select_menu));
    }

    // Direct Play Buttons for songs on the current page (Row 1: tracks 0..5, Row 2: tracks 5..10)
    let mut play_buttons_row1 = Vec::new();
    let mut play_buttons_row2 = Vec::new();

    for (i, _) in page_tracks.iter().enumerate() {
        let actual_idx = start_idx + i;
        let is_playing = actual_idx == 0;

        let label = if is_playing {
            "▶️ #0 (Playing)".to_string()
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
        .label("◀️ Prev")
        .style(ButtonStyle::Primary)
        .disabled(current_page == 0);

    let indicator_button = CreateButton::new("queue_indicator")
        .label(format!("{}/{}", current_page + 1, total_pages))
        .style(ButtonStyle::Secondary)
        .disabled(true);

    let next_button = CreateButton::new(format!("queue_page:{}", current_page + 1))
        .label("Next ▶️")
        .style(ButtonStyle::Primary)
        .disabled(current_page + 1 >= total_pages);

    let shuffle_button = CreateButton::new("queue_shuffle")
        .label(if is_shuffled { "🔀 Random: ON" } else { "🔀 Random: OFF" })
        .style(if is_shuffled { ButtonStyle::Success } else { ButtonStyle::Secondary });

    let skip_button = CreateButton::new("queue_skip")
        .label("⏭️ Skip")
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
        let _ = send_response(ctx, command, "📭 The queue is currently empty.", true).await;
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
                            .content("📭 The queue is currently empty.")
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
                            .content("📭 The queue is currently empty.")
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
                if let Some(target_track) = queue_mgr.jump_to(guild_id, idx).await {
                    let manager = songbird::get(ctx).await.unwrap();
                    if let Some(handler_lock) = manager.get(guild_id) {
                        let mut handler = handler_lock.lock().await;
                        handler.queue().stop();

                        let input = source_mgr.create_input(&target_track.stream_url).await;
                        let track_handle = handler.enqueue_input(input).await;
                        let _ = track_handle.set_volume(0.8);

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
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("📭 The queue is currently empty.")
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
    } else if custom_id == "queue_skip" {
        let manager = songbird::get(ctx).await.unwrap();
        if let Some(handler_lock) = manager.get(guild_id) {
            let handler = handler_lock.lock().await;
            if let Some(current) = handler.queue().current() {
                let _ = current.disable_loop();
                let _ = current.stop();
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await;

        let queue = queue_mgr.get_queue(guild_id).await;
        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
        let is_shuffled = queue_mgr.get_shuffle(guild_id).await;

        if queue.is_empty() {
            let _ = component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("📭 The queue is now empty.")
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
                        .content("⏹️ Playback stopped and queue cleared.")
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

    if let Some(current) = queue_mgr.get_current(guild_id).await {
        let loop_mode = queue_mgr.get_loop_mode(guild_id).await;
        let author = current.author.as_deref().unwrap_or("Unknown Artist");
        let dur = format_duration(current.duration);

        let mut embed = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(format!("Now Playing ({})", current.source))
                    .icon_url(source_icon_url(&current.source))
                    .url(&current.url),
            )
            .title(&current.title)
            .url(&current.url)
            .field("👤 Artist", author, true)
            .field("⏱️ Duration", dur, true)
            .field("🔁 Loop Mode", format!("{} {}", loop_mode.emoji(), loop_mode.as_str()), true)
            .footer(
                CreateEmbedFooter::new(format!("Platform: {}", current.source))
                    .icon_url(source_icon_url(&current.source)),
            )
            .color(source_color(&current.source));

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
