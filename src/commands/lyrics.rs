use reqwest::Client;
use serde::Deserialize;
use serenity::all::{
    Color, CommandDataOptionValue, CommandInteraction, Context, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponseFollowup,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::error;

use crate::lang::{fmt, get_lang};
use crate::queue::QueueManager;
use crate::utils::embed::source_color;
use crate::utils::response::{send_followup, send_response};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LrcLibTrack {
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "albumName")]
    pub album_name: Option<String>,
    pub instrumental: Option<bool>,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
}

pub fn clean_song_title(raw: &str) -> String {
    let mut cleaned = raw.to_string();

    let junk_patterns = [
        "(official music video)",
        "[official music video]",
        "(official video)",
        "[official video]",
        "(official audio)",
        "[official audio]",
        "(official lyric video)",
        "[official lyric video]",
        "(lyric video)",
        "[lyric video]",
        "(official visualizer)",
        "[official visualizer]",
        "(visualizer)",
        "[visualizer]",
        "(lyrics)",
        "[lyrics]",
        "(audio)",
        "[audio]",
        "(mv)",
        "[mv]",
        "【official mv】",
        "【mv】",
        "official music video",
        "official video",
        "music video",
        "[4k]",
        "(4k)",
        "[hd]",
        "(hd)",
    ];

    for pat in junk_patterns {
        let lower = cleaned.to_lowercase();
        if let Some(idx) = lower.find(pat) {
            cleaned.replace_range(idx..idx + pat.len(), "");
        }
    }

    cleaned = cleaned
        .replace("()", "")
        .replace("[]", "")
        .replace("【】", "");

    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub async fn fetch_lyrics(client: &Client, query: &str) -> Option<LrcLibTrack> {
    let res = client
        .get("https://lrclib.net/api/search")
        .query(&[("q", query)])
        .header("User-Agent", "DiscordMusicBot/1.0 (https://github.com/Takaeru/Discord-Music-bot)")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    let tracks: Vec<LrcLibTrack> = res.json().await.ok()?;

    tracks
        .into_iter()
        .find(|t| t.plain_lyrics.as_ref().map(|l| !l.trim().is_empty()).unwrap_or(false))
}

pub async fn handle_lyrics(
    ctx: &Context,
    command: &CommandInteraction,
    queue_mgr: &Arc<QueueManager>,
) {
    let guild_id = match command.guild_id {
        Some(id) => id,
        None => return,
    };

    let query_opt = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "query")
        .and_then(|opt| match &opt.value {
            CommandDataOptionValue::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        });

    let (search_query, display_title, thumbnail, embed_color) = match query_opt {
        Some(q) => (q.clone(), q, None, Color::from_rgb(88, 101, 242)),
        None => {
            if let Some(current) = queue_mgr.get_current(guild_id).await {
                let clean = clean_song_title(&current.title);
                let query = if let Some(author) = &current.author {
                    format!("{} {}", author, clean)
                } else {
                    clean
                };
                let display = current.title.clone();
                let color = source_color(&current.source);
                (query, display, current.thumbnail.clone(), color)
            } else {
                let _ = send_response(ctx, command, get_lang().lyrics_no_track, true).await;
                return;
            }
        }
    };

    // Defer because external API search might take 1-3 seconds
    if let Err(e) = command.defer(&ctx.http).await {
        error!("Failed to defer lyrics interaction: {:?}", e);
        return;
    }

    let http_client = reqwest::Client::new();
    let lyrics_track = match fetch_lyrics(&http_client, &search_query).await {
        Some(t) => Some(t),
        None => {
            // Fallback: try raw display title
            fetch_lyrics(&http_client, &display_title).await
        }
    };

    match lyrics_track {
        Some(track) => {
            let mut lyrics_text = track.plain_lyrics.unwrap_or_default();

            // Discord embed description limit is 4096 characters
            if lyrics_text.len() > 4000 {
                lyrics_text.truncate(4000);
                lyrics_text.push_str("\n\n*... [Lyrics truncated due to Discord length limit]*");
            }

            let song_title = track.track_name.as_deref().unwrap_or(&display_title);
            let artist_name = track.artist_name.as_deref().unwrap_or(get_lang().unknown_artist);
            let embed_title = fmt(get_lang().lyrics_title, &[&song_title, &artist_name]);

            let requester_name = &command.user.name;
            let footer_text = fmt(get_lang().lyrics_footer, &[&requester_name]);

            let mut embed = CreateEmbed::new()
                .title(embed_title)
                .description(lyrics_text)
                .color(embed_color)
                .footer(CreateEmbedFooter::new(footer_text));

            if let Some(thumb) = thumbnail {
                embed = embed.thumbnail(thumb);
            }

            let _ = command
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new().embed(embed),
                )
                .await;
        }
        None => {
            let not_found_msg = fmt(get_lang().lyrics_not_found, &[&display_title]);
            let _ = send_followup(ctx, command, &not_found_msg).await;
        }
    }
}
