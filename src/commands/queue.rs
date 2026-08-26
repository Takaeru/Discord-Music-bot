use serenity::all::{
    CommandInteraction, Context, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use std::sync::Arc;
use std::time::Duration;

use crate::queue::QueueManager;
use crate::utils::embed::{format_duration, source_color, source_emoji, source_icon_url};
use crate::utils::response::send_response;

pub async fn handle_queue(ctx: &Context, command: &CommandInteraction, queue_mgr: &Arc<QueueManager>) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let queue = queue_mgr.get_queue(guild_id).await;
    let loop_mode = queue_mgr.get_loop_mode(guild_id).await;

    if queue.is_empty() {
        let _ = send_response(ctx, command, "📭 The queue is currently empty.", false).await;
        return;
    }

    let first_track = &queue[0];
    let total_duration: Duration = queue.iter().filter_map(|t| t.duration).sum();

    let mut desc = String::new();
    for (i, track) in queue.iter().take(10).enumerate() {
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

    if queue.len() > 10 {
        desc.push_str(&format!("\n*...and {} more tracks in queue*", queue.len() - 10));
    }

    let mut embed = CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new(format!("Current Music Queue ({})", first_track.source))
                .icon_url(source_icon_url(&first_track.source))
                .url(&first_track.url),
        )
        .title("📋 Music Queue")
        .description(desc)
        .field("📊 Total Tracks", format!("{}", queue.len()), true)
        .field("⏱️ Total Duration", format_duration(Some(total_duration)), true)
        .field(
            "🔁 Repeat Mode",
            format!("{} {}", loop_mode.emoji(), loop_mode.as_str()),
            true,
        )
        .footer(
            CreateEmbedFooter::new(format!("Platform: {} | Loop: {}", first_track.source, loop_mode.as_str()))
                .icon_url(source_icon_url(&first_track.source)),
        )
        .color(source_color(&first_track.source));

    if let Some(thumb) = &first_track.thumbnail {
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
