use serde::Deserialize;
use songbird::input::{HttpRequest, Input, YoutubeDl};
use std::process::Command;
use std::time::Duration;
use tracing::{error, info};

#[derive(Debug, Clone, Deserialize)]
pub struct TrackMetadata {
    pub title: String,
    pub url: String,
    pub stream_url: String,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub author: Option<String>,
    pub source: String,
}

#[derive(Deserialize)]
struct YtDlpOutput {
    title: Option<String>,
    webpage_url: Option<String>,
    url: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    uploader: Option<String>,
    extractor_key: Option<String>,
    entries: Option<Vec<YtDlpOutput>>,
    _type: Option<String>,
}

#[derive(Debug, Clone)]
struct SpotifyTrackInfo {
    title: String,
    artist: String,
    url: String,
    thumbnail: Option<String>,
    duration: Option<Duration>,
}

pub struct SourceManager {
    http_client: reqwest::Client,
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Resolves a user query (YouTube, Spotify, SoundCloud, or keyword) into a list of TrackMetadata.
    pub async fn resolve(&self, query: &str) -> Result<Vec<TrackMetadata>, String> {
        let is_spotify = query.contains("open.spotify.com") || query.starts_with("spotify:");

        if is_spotify {
            info!("Resolving Spotify URL: {}", query);
            let spotify_items = self.resolve_spotify_items(query).await?;
            let mut resolved_tracks = Vec::new();

            for item in spotify_items {
                let search = format!("{} - {}", item.artist, item.title);
                info!("Matching audio for Spotify track: {}", search);

                if let Ok(yt_tracks) = self.resolve_single_query(&search).await {
                    if let Some(yt) = yt_tracks.into_iter().next() {
                        resolved_tracks.push(TrackMetadata {
                            title: item.title,
                            url: item.url,
                            stream_url: yt.stream_url,
                            duration: item.duration.or(yt.duration),
                            thumbnail: item.thumbnail.or(yt.thumbnail),
                            author: Some(item.artist),
                            source: "Spotify".to_string(),
                        });
                    }
                }
            }

            if resolved_tracks.is_empty() {
                return Err("Could not find playable audio for the Spotify link.".to_string());
            }

            return Ok(resolved_tracks);
        }

        self.resolve_single_query(query).await
    }

    /// Resolves Spotify tracks, albums, or playlists into rich metadata using Spotify Embed API.
    async fn resolve_spotify_items(&self, url: &str) -> Result<Vec<SpotifyTrackInfo>, String> {
        let embed_url = if url.contains("/track/") {
            let id = url
                .split("/track/")
                .nth(1)
                .and_then(|s| s.split('?').next())
                .unwrap_or("")
                .trim_matches('/');
            format!("https://open.spotify.com/embed/track/{}", id)
        } else if url.contains("/playlist/") {
            let id = url
                .split("/playlist/")
                .nth(1)
                .and_then(|s| s.split('?').next())
                .unwrap_or("")
                .trim_matches('/');
            format!("https://open.spotify.com/embed/playlist/{}", id)
        } else if url.contains("/album/") {
            let id = url
                .split("/album/")
                .nth(1)
                .and_then(|s| s.split('?').next())
                .unwrap_or("")
                .trim_matches('/');
            format!("https://open.spotify.com/embed/album/{}", id)
        } else {
            return Err("Unsupported Spotify URL format.".to_string());
        };

        info!("Fetching Spotify metadata from: {}", embed_url);

        let resp = self
            .http_client
            .get(&embed_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Spotify embed page: {}", e))?;

        let html = resp.text().await.unwrap_or_default();
        let mut items = Vec::new();

        // Extract __NEXT_DATA__ JSON payload from Spotify embed page
        if let Some(start) = html.find("id=\"__NEXT_DATA__\"") {
            if let Some(json_start) = html[start..].find('>') {
                let rest = &html[start + json_start + 1..];
                if let Some(json_end) = rest.find("</script>") {
                    let json_str = &rest[..json_end];
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let entity = v.pointer("/props/pageProps/state/data/entity");

                        if let Some(entity_obj) = entity {
                            let entity_type = entity_obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

                            if entity_type == "track" {
                                let title = entity_obj
                                    .get("name")
                                    .or_else(|| entity_obj.get("title"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let mut artists = Vec::new();
                                if let Some(artist_arr) = entity_obj.get("artists").and_then(|a| a.as_array()) {
                                    for a in artist_arr {
                                        if let Some(name) = a.get("name").and_then(|n| n.as_str()) {
                                            artists.push(name);
                                        }
                                    }
                                }

                                let artist = artists.join(", ");
                                let duration = entity_obj
                                    .get("duration")
                                    .and_then(|d| d.as_u64())
                                    .map(Duration::from_millis);

                                let thumbnail = entity_obj
                                    .pointer("/visualIdentity/image/0/url")
                                    .and_then(|u| u.as_str())
                                    .map(|s| s.to_string());

                                let track_id = entity_obj.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                let track_url = if !track_id.is_empty() {
                                    format!("https://open.spotify.com/track/{}", track_id)
                                } else {
                                    url.to_string()
                                };

                                if !title.is_empty() {
                                    items.push(SpotifyTrackInfo {
                                        title,
                                        artist: if artist.is_empty() { "Spotify Artist".to_string() } else { artist },
                                        url: track_url,
                                        thumbnail,
                                        duration,
                                    });
                                }
                            } else {
                                // Playlist or Album
                                if let Some(track_list) = entity_obj.get("trackList").and_then(|t| t.as_array()) {
                                    for track in track_list.iter().take(50) {
                                        let title = track.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                        let subtitle = track.get("subtitle").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                        let duration = track.get("duration").and_then(|d| d.as_u64()).map(Duration::from_millis);
                                        let track_id = track.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                        let track_url = if !track_id.is_empty() {
                                            format!("https://open.spotify.com/track/{}", track_id)
                                        } else {
                                            url.to_string()
                                        };

                                        if !title.is_empty() {
                                            items.push(SpotifyTrackInfo {
                                                title,
                                                artist: if subtitle.is_empty() { "Spotify Artist".to_string() } else { subtitle },
                                                url: track_url,
                                                thumbnail: None,
                                                duration,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if items.is_empty() {
            return Err("Could not extract tracks from Spotify link.".to_string());
        }

        Ok(items)
    }

    /// Resolves YouTube, SoundCloud, or direct URLs via yt-dlp.
    async fn resolve_single_query(&self, query: &str) -> Result<Vec<TrackMetadata>, String> {
        let is_url = query.starts_with("http://") || query.starts_with("https://");
        let is_pure_playlist = is_url && (query.contains("/playlist?list=") || query.contains("&list=PL") || query.contains("?list=PL"));
        let is_watch_url = is_url && (query.contains("watch?v=") || query.contains("youtu.be/"));

        let source_hint = if query.contains("soundcloud.com") {
            "SoundCloud"
        } else if query.contains("youtube.com") || query.contains("youtu.be") || !is_url {
            "YouTube"
        } else {
            "Direct Stream"
        };

        let search_target = if is_url || query.starts_with("ytsearch") {
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
                    if let Some(track) = Self::parse_single_entry(entry, source_hint) {
                        tracks.push(track);
                    }
                }
            } else {
                // For search queries or single video results, pick the first result
                if let Some(first) = entries.into_iter().next() {
                    if let Some(track) = Self::parse_single_entry(first, source_hint) {
                        tracks.push(track);
                    }
                }
            }
        } else {
            if let Some(track) = Self::parse_single_entry(parsed, source_hint) {
                tracks.push(track);
            }
        }

        if tracks.is_empty() {
            return Err("No tracks found for the requested query.".to_string());
        }

        Ok(tracks)
    }

    fn parse_single_entry(entry: YtDlpOutput, source_hint: &str) -> Option<TrackMetadata> {
        let title = entry.title?;
        let webpage = entry.webpage_url.unwrap_or_else(|| entry.url.clone().unwrap_or_default());
        let url = if !webpage.is_empty() {
            webpage
        } else {
            entry.url.clone().unwrap_or_default()
        };
        if url.is_empty() {
            return None;
        }

        let stream_url = url.clone();
        let duration = entry.duration.map(|d| Duration::from_secs_f64(d));
        let thumbnail = entry.thumbnail;
        let author = entry.uploader;

        let source = if let Some(extractor) = entry.extractor_key {
            if extractor.to_lowercase().contains("soundcloud") {
                "SoundCloud".to_string()
            } else if extractor.to_lowercase().contains("youtube") {
                "YouTube".to_string()
            } else {
                source_hint.to_string()
            }
        } else {
            source_hint.to_string()
        };

        Some(TrackMetadata {
            title,
            url,
            stream_url,
            duration,
            thumbnail,
            author,
            source,
        })
    }

    /// Extracts a direct progressive media stream URL (Opus in WebM or MP3) to avoid AAC ADTS errors.
    pub async fn extract_direct_stream(&self, url: &str) -> Result<String, String> {
        let target = url.to_string();
        tokio::task::spawn_blocking(move || {
            let output = Command::new("yt-dlp")
                .args([
                    "-g",
                    "--format-sort",
                    "acodec:opus,acodec:mp3,proto:https",
                    "-f",
                    "bestaudio[acodec=opus]/bestaudio[ext=webm]/http_mp3_128/bestaudio[ext=mp3]/bestaudio[acodec!=aac]/bestaudio/best",
                    "--no-warnings",
                    &target,
                ])
                .output()
                .map_err(|e| format!("Failed to run yt-dlp -g: {}", e))?;

            if output.status.success() {
                let direct = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !direct.is_empty() && direct.starts_with("http") {
                    return Ok(direct);
                }
            }
            Err("Failed to resolve direct audio URL".to_string())
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// Creates a Songbird audio Input from a track URL using direct progressive HttpRequest or YoutubeDl fallback.
    pub async fn create_input(&self, url: &str) -> Input {
        if let Ok(direct_url) = self.extract_direct_stream(url).await {
            info!("Playing via direct progressive audio stream (Opus/MP3): {}", direct_url.split('?').next().unwrap_or(&direct_url));
            HttpRequest::new(self.http_client.clone(), direct_url).into()
        } else {
            info!("Playing via YoutubeDl fallback for: {}", url);
            YoutubeDl::new(self.http_client.clone(), url.to_string()).into()
        }
    }
}
