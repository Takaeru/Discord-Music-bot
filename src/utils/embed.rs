use serenity::all::{ButtonStyle, Color, CreateActionRow, CreateButton, CreateEmbed};
use std::env;
use std::time::Duration;

use crate::queue::LoopMode;
use crate::source::TrackMetadata;

pub fn source_emoji(source: &str) -> Option<String> {
    match source {
        "Spotify" => env::var("EMOJI_SPOTIFY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        "YouTube" => env::var("EMOJI_YOUTUBE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        "SoundCloud" => env::var("EMOJI_SOUNDCLOUD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

pub fn source_icon_url(source: &str) -> &'static str {
    match source {
        "Spotify" => "https://raw.githubusercontent.com/walkxcode/dashboard-icons/main/png/spotify.png",
        "YouTube" => "https://raw.githubusercontent.com/walkxcode/dashboard-icons/main/png/youtube.png",
        "SoundCloud" => "https://raw.githubusercontent.com/walkxcode/dashboard-icons/main/png/soundcloud.png",
        _ => "https://raw.githubusercontent.com/walkxcode/dashboard-icons/main/png/audiobookshelf.png",
    }
}

pub fn source_color(source: &str) -> Color {
    match source {
        "Spotify" => Color::from_rgb(30, 215, 96),   // Spotify Vibrant Green
        "YouTube" => Color::from_rgb(255, 0, 0),     // YouTube Vibrant Red
        "SoundCloud" => Color::from_rgb(255, 85, 0), // SoundCloud Orange
        _ => Color::from_rgb(88, 101, 242),          // Discord Blurple
    }
}

pub fn format_duration(dur: Option<Duration>) -> String {
    match dur {
        Some(d) => {
            let secs = d.as_secs();
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let rem_secs = secs % 60;
            if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, mins, rem_secs)
            } else {
                format!("{}:{:02}", mins, rem_secs)
            }
        }
        None => "--:--".to_string(),
    }
}

pub fn build_now_playing_embed(
    track: &TrackMetadata,
    upcoming_count: usize,
    loop_mode: LoopMode,
    is_paused: bool,
) -> (CreateEmbed, CreateActionRow) {
    let loop_str = match loop_mode {
        LoopMode::Off => "❌ Nonaktif",
        LoopMode::Track => "🔂 1 Lagu",
        LoopMode::Queue => "🔁 Semua Antrean",
    };

    let dur_str = format_duration(track.duration);
    let artist_str = track.author.as_deref().unwrap_or("Unknown Artist");
    let requester_str = track.requester.as_deref().unwrap_or("-");

    let mut embed = CreateEmbed::new()
        .title("🎵 Sekarang Diputar")
        .description(format!("[{}]({})", track.title, track.url))
        .field("⏱️ Durasi", dur_str, true)
        .field("🎶 Artis", artist_str, true)
        .field("👤 Diminta oleh", requester_str, true)
        .field("📌 Antrian", format!("{} lagu berikutnya", upcoming_count), true)
        .field("🔁 Loop", loop_str, true)
        .field("\u{200b}", "🎶 Gunakan tombol di bawah untuk kontrol musik", false)
        .color(Color::from_rgb(88, 101, 242));

    if let Some(thumb) = &track.thumbnail {
        embed = embed.thumbnail(thumb);
    }

    let pause_btn = if is_paused {
        CreateButton::new("music_resume")
            .label("Resume")
            .emoji('▶')
            .style(ButtonStyle::Primary)
    } else {
        CreateButton::new("music_pause")
            .label("Pause")
            .emoji('⏸')
            .style(ButtonStyle::Primary)
    };

    let skip_btn = CreateButton::new("music_skip")
        .label("Skip")
        .emoji('⏭')
        .style(ButtonStyle::Primary);

    let loop_btn = CreateButton::new("music_loop")
        .label("Loop")
        .emoji('🔁')
        .style(ButtonStyle::Secondary);

    let stop_btn = CreateButton::new("music_stop")
        .label("Stop")
        .emoji('⏹')
        .style(ButtonStyle::Danger);

    let action_row = CreateActionRow::Buttons(vec![pause_btn, skip_btn, loop_btn, stop_btn]);

    (embed, action_row)
}
