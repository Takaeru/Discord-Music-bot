use serenity::all::Color;
use std::env;
use std::time::Duration;

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
                format!("{:02}:{:02}", mins, rem_secs)
            }
        }
        None => "--:--".to_string(),
    }
}
