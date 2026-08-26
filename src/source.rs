use serde::Deserialize;
use songbird::input::{Input, YoutubeDl};
use std::process::Command;
use std::time::Duration;
use tracing::{error, info};

#[derive(Debug, Clone, Deserialize)]
pub struct TrackMetadata {
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub author: Option<String>,
}

#[derive(Deserialize)]
struct YtDlpOutput {
    title: Option<String>,
    webpage_url: Option<String>,
    url: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    uploader: Option<String>,
    entries: Option<Vec<YtDlpOutput>>,
    _type: Option<String>,
}

pub struct SourceManager {
    http_client: reqwest::Client,
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    /// Resolves a user query into a list of TrackMetadata using yt-dlp.
    /// Handles single URLs, search queries, and playlists.
    pub async fn resolve(&self, query: &str) -> Result<Vec<TrackMetadata>, String> {
        let is_url = query.starts_with("http://") || query.starts_with("https://");
        let is_pure_playlist = is_url && (query.contains("/playlist?list=") || query.contains("&list=PL") || query.contains("?list=PL"));
        let is_watch_url = is_url && (query.contains("watch?v=") || query.contains("youtu.be/"));

        let search_target = if is_url {
            query.to_string()
        } else {
            format!("ytsearch10:{}", query)
        };

        info!("Resolving query via yt-dlp: {}", search_target);

        let output = tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new("yt-dlp");
            cmd.args([
                "-J",
                "--default-search",
                "ytsearch",
                "--no-warnings",
            ]);

            // If it's a single video link with a mix/radio attached (&list=RD...), don't extract the whole 900+ radio mix
            if is_watch_url && !is_pure_playlist {
                cmd.arg("--no-playlist");
            } else if is_pure_playlist {
                // Limit playlists to maximum 50 tracks to prevent queue overload
                cmd.args(["--flat-playlist", "--playlist-end", "50"]);
            } else {
                cmd.arg("--flat-playlist");
            }

            cmd.arg(&search_target);
            cmd.output()
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| format!("Failed to execute yt-dlp: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp extraction failed: {}", stderr);
            return Err(format!("Could not extract audio metadata: {}", stderr.trim()));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: YtDlpOutput = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse metadata JSON: {}", e))?;

        let mut tracks = Vec::new();

        if let Some(entries) = parsed.entries {
            if is_pure_playlist || (is_url && parsed._type.as_deref() == Some("playlist") && !is_watch_url) {
                // Return all tracks in playlist (capped at 50)
                for entry in entries.into_iter().take(50) {
                    if let Some(track) = Self::parse_single_entry(entry) {
                        tracks.push(track);
                    }
                }
            } else {
                // For search queries or single video results, pick the first result
                if let Some(first) = entries.into_iter().next() {
                    if let Some(track) = Self::parse_single_entry(first) {
                        tracks.push(track);
                    }
                }
            }
        } else {
            if let Some(track) = Self::parse_single_entry(parsed) {
                tracks.push(track);
            }
        }

        if tracks.is_empty() {
            return Err("No tracks found for the requested query.".to_string());
        }

        Ok(tracks)
    }

    fn parse_single_entry(entry: YtDlpOutput) -> Option<TrackMetadata> {
        let title = entry.title?;
        let url = entry.webpage_url.or(entry.url)?;
        let duration = entry.duration.map(|d| Duration::from_secs_f64(d));
        let thumbnail = entry.thumbnail;
        let author = entry.uploader;

        Some(TrackMetadata {
            title,
            url,
            duration,
            thumbnail,
            author,
        })
    }

    /// Creates a Songbird audio Input from a track URL using YoutubeDl.
    pub fn create_input(&self, url: &str) -> Input {
        YoutubeDl::new(self.http_client.clone(), url.to_string()).into()
    }
}
