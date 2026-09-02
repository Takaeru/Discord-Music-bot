use moka::future::Cache;
use serde::{Deserialize, Serialize};
use songbird::input::{Input, YoutubeDl};
use std::process::Command;
use std::time::Duration;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    query_cache: Cache<String, Vec<TrackMetadata>>,
    stream_cache: Cache<String, String>,
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .tcp_keepalive(Some(Duration::from_secs(30)))
                .pool_idle_timeout(Some(Duration::from_secs(30)))
                .pool_max_idle_per_host(3)
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            query_cache: Cache::builder()
                .max_capacity(100)
                .time_to_live(Duration::from_secs(4 * 3600))
                .build(),
            stream_cache: Cache::builder()
                .max_capacity(50)
                .time_to_live(Duration::from_secs(2 * 3600))
                .build(),
        }
    }

    /// Reads MAX_PLAYLIST_ITEMS (or MAX_PLAYLIST_TRACKS) from .env. Defaults to 50.
    /// Set to 0 for unlimited.
    pub fn get_max_playlist_limit() -> usize {
        std::env::var("MAX_PLAYLIST_ITEMS")
            .or_else(|_| std::env::var("MAX_PLAYLIST_TRACKS"))
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(50)
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
                                // Playlist or Album - extract tracks respecting MAX_PLAYLIST_ITEMS
                                if let Some(track_list) = entity_obj.get("trackList").and_then(|t| t.as_array()) {
                                    let max_items = Self::get_max_playlist_limit();
                                    let limit = if max_items > 0 { max_items } else { usize::MAX };
                                    for track in track_list.iter().take(limit) {
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

        if let Some(cached) = self.query_cache.get(&search_target).await {
            info!("Query cache HIT for: {}", search_target);
            return Ok(cached);
        }

        info!("Resolving query via yt-dlp: {}", search_target);

        let max_items = Self::get_max_playlist_limit();

        let target_for_cmd = search_target.clone();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                let mut cmd = Command::new("yt-dlp");
                cmd.args([
                    "-J",
                    "--default-search",
                    "ytsearch",
                    "--no-warnings",
                ]);

                if has_playlist {
                    cmd.arg("--flat-playlist");
                    if max_items > 0 {
                        cmd.args(["--playlist-items", &format!("1:{}", max_items)]);
                    }
                } else if is_url {
                    cmd.arg("--no-playlist");
                } else {
                    cmd.arg("--flat-playlist");
                }

                cmd.arg(&target_for_cmd);
                cmd.output()
            }),
        )
        .await
        .map_err(|_| "yt-dlp timed out after 30 seconds".to_string())?
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
            let limit = if (has_playlist || (is_url && parsed._type.as_deref() == Some("playlist"))) && max_items > 0 {
                max_items
            } else {
                usize::MAX
            };

            for entry in entries.into_iter().take(limit) {
                if let Some(track) = Self::parse_single_entry(entry, source_hint) {
                    tracks.push(track);
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

        self.query_cache.insert(search_target, tracks.clone()).await;

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
        if let Some(cached) = self.stream_cache.get(url).await {
            info!("Stream cache HIT for direct audio URL: {}", url);
            return Ok(cached);
        }

        let target = url.to_string();
        let task = tokio::task::spawn_blocking(move || {
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
                    "3",
                    "--fragment-retries",
                    "3",
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
        });

        // 20s cap — with retries 3 × socket-timeout 15s worst case would otherwise be 90s+
        let direct = match tokio::time::timeout(std::time::Duration::from_secs(20), task).await {
            Ok(join) => join.map_err(|e| format!("Task join error: {}", e))??,
            Err(_) => return Err("Stream URL resolution timed out after 20s".to_string()),
        };

        self.stream_cache.insert(url.to_string(), direct.clone()).await;
        Ok(direct)
    }

    /// Creates a Songbird audio Input with exact 48,000 Hz Stereo Opus resampling, 96kbps fast-loading & anti-jitter pipeline.
    #[allow(dead_code)]
    pub async fn create_input(&self, url: &str) -> Input {
        self.create_input_filtered(url, None, None).await
    }

    /// Creates a Songbird audio Input starting at an optional timestamp (fast keyframe seeking via FFmpeg).
    #[allow(dead_code)]
    pub async fn create_input_at(&self, url: &str, start_time: Option<Duration>) -> Input {
        self.create_input_filtered(url, start_time, None).await
    }

    /// Creates a Songbird audio Input with optional timestamp seeking and audio filter (FFmpeg -af).
    pub async fn create_input_filtered(
        &self,
        url: &str,
        start_time: Option<Duration>,
        filter: Option<&str>,
    ) -> Input {
        let stream_target = match self.extract_direct_stream(url).await {
            Ok(direct) => direct,
            Err(_) => url.to_string(),
        };

        info!(
            "Creating fast-loading 48kHz audio pipeline for: {} (seek: {:?}, filter: {:?})",
            url, start_time, filter
        );

        let filter_owned = filter.map(|s| s.to_string());

        let res = tokio::task::spawn_blocking(move || {
            let mut ffmpeg = std::process::Command::new("ffmpeg");

            if let Some(dur) = start_time {
                let secs = dur.as_secs_f64();
                ffmpeg.args(["-ss", &secs.to_string()]);
            }

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
            ]);

            if let Some(ref f) = filter_owned {
                ffmpeg.args(["-af", f]);
            }

            ffmpeg.args([
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

    pub fn extract_youtube_id(url: &str) -> Option<String> {
        if let Some(pos) = url.find("watch?v=") {
            let id_part = &url[pos + 8..];
            let id = id_part.split('&').next()?.split('?').next()?;
            if id.len() >= 11 {
                return Some(id[..11].to_string());
            }
        } else if let Some(pos) = url.find("youtu.be/") {
            let id_part = &url[pos + 9..];
            let id = id_part.split('&').next()?.split('?').next()?;
            if id.len() >= 11 {
                return Some(id[..11].to_string());
            }
        }
        None
    }

    pub async fn get_recommendation(
        &self,
        seed: &TrackMetadata,
        history: &[TrackMetadata],
    ) -> Option<TrackMetadata> {
        let video_id = Self::extract_youtube_id(&seed.url)
            .or_else(|| Self::extract_youtube_id(&seed.stream_url));

        let clean_title = seed
            .title
            .replace("(Official Video)", "")
            .replace("[Official Video]", "")
            .replace("(Official Music Video)", "")
            .replace("[Official Music Video]", "")
            .replace("(Lyric Video)", "")
            .replace("[Lyric Video]", "")
            .replace("(MV)", "")
            .replace("[MV]", "")
            .replace("【MV】", "");

        let is_duplicate = |cand: &TrackMetadata| -> bool {
            if cand.title.eq_ignore_ascii_case(&seed.title) || cand.url == seed.url {
                return true;
            }
            let cand_yt_id = Self::extract_youtube_id(&cand.url)
                .or_else(|| Self::extract_youtube_id(&cand.stream_url));

            history.iter().any(|h| {
                if h.title.eq_ignore_ascii_case(&cand.title) {
                    return true;
                }
                if !h.url.is_empty() && h.url == cand.url {
                    return true;
                }
                if let Some(ref cid) = cand_yt_id {
                    let h_yt_id = Self::extract_youtube_id(&h.url)
                        .or_else(|| Self::extract_youtube_id(&h.stream_url));
                    if h_yt_id.as_deref() == Some(cid.as_str()) {
                        return true;
                    }
                }
                false
            })
        };

        // 1. Primary: YouTube Mix Playlist
        if let Some(ref id) = video_id {
            let mix_url = format!("https://www.youtube.com/watch?v={}&list=RD{}", id, id);
            info!("Attempting autoplay via YouTube Mix: {}", mix_url);
            if let Ok(resolved) = self.resolve_single_query(&mix_url).await {
                for track in resolved {
                    if !is_duplicate(&track) {
                        return Some(track);
                    }
                }
            }
        }

        // 2. Secondary Fallback: YouTube Search
        let search_query = match &seed.author {
            Some(author) if !author.is_empty() && author != "YouTube" => {
                format!("ytsearch15:{} songs", author.trim())
            }
            _ => {
                format!("ytsearch15:{} songs", clean_title.trim())
            }
        };

        info!("Attempting autoplay via YouTube Search fallback: {}", search_query);
        if let Ok(resolved) = self.resolve_single_query(&search_query).await {
            for track in resolved {
                if !is_duplicate(&track) {
                    return Some(track);
                }
            }
        }

        None
    }
}
