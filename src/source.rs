use serde::Deserialize;
use songbird::input::{Input, YoutubeDl};
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
    pub requester: Option<String>,
    pub view_count: Option<u64>,
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
    view_count: Option<u64>,
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
                .tcp_keepalive(Some(Duration::from_secs(30)))
                .pool_idle_timeout(Some(Duration::from_secs(120)))
                .pool_max_idle_per_host(20)
                .timeout(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(15))
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
            if spotify_items.is_empty() {
                return Err("Could not extract tracks from Spotify link.".to_string());
            }

            let mut resolved_tracks = Vec::new();

            // Resolve the first track immediately so it can play instantly
            let first = &spotify_items[0];
            let first_search = format!("{} - {}", first.artist, first.title);
            let first_yt = self.resolve_single_query(&first_search).await
                .ok()
                .and_then(|v| v.into_iter().next());

            let first_stream = match first_yt {
                Some(ref yt) => yt.stream_url.clone(),
                None => format!("ytsearch1:{}", first_search),
            };

            resolved_tracks.push(TrackMetadata {
                title: first.title.clone(),
                url: first.url.clone(),
                stream_url: first_stream,
                duration: first.duration.or(first_yt.as_ref().and_then(|y| y.duration)),
                thumbnail: first.thumbnail.clone().or(first_yt.as_ref().and_then(|y| y.thumbnail.clone())),
                author: Some(first.artist.clone()),
                source: "Spotify".to_string(),
                requester: None,
                view_count: None,
            });

            // For the remaining tracks in the playlist, defer audio resolution until playback
            for item in spotify_items.into_iter().skip(1) {
                let search = format!("ytsearch1:{} - {}", item.artist, item.title);
                resolved_tracks.push(TrackMetadata {
                    title: item.title,
                    url: item.url,
                    stream_url: search,
                    duration: item.duration,
                    thumbnail: item.thumbnail,
                    author: Some(item.artist),
                    source: "Spotify".to_string(),
                    requester: None,
                    view_count: None,
                });
            }

            return Ok(resolved_tracks);
        }

        self.resolve_single_query(query).await
    }

    /// Searches YouTube and returns all candidates (up to 10) for user selection.
    /// For URLs, returns a single-element vec via resolve().
    pub async fn search(&self, query: &str) -> Result<Vec<TrackMetadata>, String> {
        let is_url = query.starts_with("http://") || query.starts_with("https://");
        let is_spotify = query.contains("open.spotify.com") || query.starts_with("spotify:");

        if is_url || is_spotify {
            // URLs/Spotify → resolve directly, no search
            return self.resolve(query).await;
        }

        // Text query → search YouTube for candidates
        info!("Searching YouTube for candidates: {}", query);
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
                                // Playlist or Album - extract ALL tracks without any hardcoded limit
                                if let Some(track_list) = entity_obj.get("trackList").and_then(|t| t.as_array()) {
                                    for track in track_list.iter() {
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
        let has_playlist = is_url && query.contains("list=");

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

            if has_playlist {
                // Unlimited playlist loading
                cmd.arg("--flat-playlist");
            } else if is_url {
                cmd.arg("--no-playlist");
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
            if has_playlist || (is_url && parsed._type.as_deref() == Some("playlist")) {
                // Return all tracks in playlist/mix without capping
                for entry in entries.into_iter() {
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
            requester: None,
            view_count: entry.view_count,
        })
    }

    /// Extracts a direct progressive media stream URL (Opus in WebM or MP3) to avoid AAC ADTS errors.
    #[allow(dead_code)]
    pub async fn extract_direct_stream(&self, url: &str) -> Result<String, String> {
        let target = url.to_string();
        tokio::task::spawn_blocking(move || {
            let output = Command::new("yt-dlp")
                .args([
                    "-g",
                    "--format-sort",
                    "acodec:opus,acodec:mp3,abr:96,proto:https",
                    "-f",
                    "ba[acodec=opus][abr<=128]/ba[ext=webm][abr<=128]/ba[acodec=opus]/ba[ext=webm]/http_mp3_128/ba[ext=mp3]/ba[acodec!=aac]/ba/b",
                    "--socket-timeout",
                    "15",
                    "--retries",
                    "10",
                    "--fragment-retries",
                    "10",
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

    /// Creates a Songbird audio Input with exact 48,000 Hz Stereo Opus resampling, 96kbps fast-loading & anti-jitter pipeline.
    pub async fn create_input(&self, url: &str) -> Input {
        let stream_target = match self.extract_direct_stream(url).await {
            Ok(direct) => direct,
            Err(_) => url.to_string(),
        };

        info!("Creating fast-loading 48kHz audio pipeline for: {}", url);

        let res = tokio::task::spawn_blocking(move || {
            let mut ffmpeg = std::process::Command::new("ffmpeg");
            ffmpeg.args([
                "-reconnect",
                "1",
                "-reconnect_streamed",
                "1",
                "-reconnect_delay_max",
                "5",
                "-probesize",
                "32768",
                "-analyzeduration",
                "0",
                "-nostdin",
                "-i",
                &stream_target,
                "-vn",
                "-c:a",
                "libopus",
                "-b:a",
                "96k",
                "-ar",
                "48000",
                "-ac",
                "2",
                "-f",
                "ogg",
                "pipe:1",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

            let child = ffmpeg.spawn().ok()?;
            Some(songbird::input::ChildContainer::new(vec![child]))
        })
        .await;

        match res {
            Ok(Some(container)) => container.into(),
            _ => YoutubeDl::new(self.http_client.clone(), url.to_string()).into(),
        }
    }
}
