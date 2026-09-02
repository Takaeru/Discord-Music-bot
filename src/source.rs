use moka::future::Cache;
use serde::{Deserialize, Serialize};
use songbird::input::{Input, YoutubeDl};
use std::process::Command;
use std::sync::Arc;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasteProfile {
    pub top_artists: Vec<String>,
    pub dominant_region: String,
    pub top_keywords: Vec<String>,
    pub summary: String,
}

pub struct SourceManager {
    http_client: reqwest::Client,
    query_cache: Cache<String, Vec<TrackMetadata>>,
    stream_cache: Cache<String, String>,
    spotify_token_cache: Cache<String, String>,
    ai_client: Arc<crate::ai::AiClient>,
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
            spotify_token_cache: Cache::builder()
                .max_capacity(2)
                .time_to_live(Duration::from_secs(50 * 60))
                .build(),
            ai_client: Arc::new(crate::ai::AiClient::init()),
        }
    }

    pub fn ai(&self) -> &Arc<crate::ai::AiClient> {
        &self.ai_client
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
        let ytdlp_bin = std::env::var("YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".to_string());

        let target_for_cmd = search_target.clone();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            tokio::task::spawn_blocking(move || {
                let mut cmd = Command::new(&ytdlp_bin);
                cmd.args([
                    "-J",
                    "--default-search",
                    "ytsearch",
                    "--no-warnings",
                    "--socket-timeout",
                    "10",
                    "--retries",
                    "2",
                    "--extractor-args",
                    "youtube:player_client=android,web",
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
        .map_err(|_| "yt-dlp timed out after 45 seconds".to_string())?
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
        let ytdlp_bin = std::env::var("YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".to_string());
        let task = tokio::task::spawn_blocking(move || {
            let output = Command::new(&ytdlp_bin)
                .args([
                    "-g",
                    "--format-sort",
                    "acodec:opus,acodec:mp3,abr:96,proto:https",
                    "-f",
                    "ba[acodec=opus][abr<=128]/ba[ext=webm][abr<=128]/ba[acodec=opus]/ba[ext=webm]/http_mp3_128/ba[ext=mp3]/ba[acodec!=aac]/ba/b",
                    "--socket-timeout",
                    "10",
                    "--retries",
                    "2",
                    "--fragment-retries",
                    "2",
                    "--no-warnings",
                    "--extractor-args",
                    "youtube:player_client=android,web",
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

    /// Fetches an access token for Spotify: checks env credentials first, then falls back to Spotify Web anonymous token.
    pub async fn get_spotify_token(&self) -> Result<String, String> {
        if let Some(token) = self.spotify_token_cache.get("token").await {
            return Ok(token);
        }

        // 1. Check if official client credentials exist in .env
        if let (Ok(client_id), Ok(client_secret)) = (
            std::env::var("SPOTIFY_CLIENT_ID"),
            std::env::var("SPOTIFY_CLIENT_SECRET"),
        ) {
            if !client_id.trim().is_empty() && !client_secret.trim().is_empty() {
                let params = [("grant_type", "client_credentials")];
                if let Ok(res) = self
                    .http_client
                    .post("https://accounts.spotify.com/api/token")
                    .basic_auth(client_id.trim(), Some(client_secret.trim()))
                    .form(&params)
                    .send()
                    .await
                {
                    if res.status().is_success() {
                        if let Ok(json) = res.json::<serde_json::Value>().await {
                            if let Some(token) = json.get("access_token").and_then(|v| v.as_str()) {
                                self.spotify_token_cache
                                    .insert("token".to_string(), token.to_string())
                                    .await;
                                return Ok(token.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 2. Default: Anonymous guest token from Spotify Web Player
        let res = self
            .http_client
            .get("https://open.spotify.com/get_access_token")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send()
            .await
            .map_err(|e| format!("Spotify token HTTP error: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("Spotify token HTTP status: {}", res.status()));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Spotify token JSON: {}", e))?;

        let token = json
            .get("accessToken")
            .or_else(|| json.get("access_token"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Could not extract accessToken from Spotify web".to_string())?
            .to_string();

        self.spotify_token_cache
            .insert("token".to_string(), token.clone())
            .await;
        Ok(token)
    }

    /// Searches Spotify tracks via Web API using cached anonymous token.
    pub async fn search_spotify(&self, query: &str, limit: usize) -> Result<Vec<TrackMetadata>, String> {
        let token = self.get_spotify_token().await?;
        let encoded_query = percent_encoding::utf8_percent_encode(
            query,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        let url = format!(
            "https://api.spotify.com/v1/search?q={}&type=track&limit={}",
            encoded_query,
            limit.min(10)
        );

        let res = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send()
            .await
            .map_err(|e| format!("Spotify search HTTP error: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("Spotify search failed: HTTP {}", res.status()));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Spotify search JSON: {}", e))?;

        let mut tracks = Vec::new();
        if let Some(items) = json
            .get("tracks")
            .and_then(|t| t.get("items"))
            .and_then(|i| i.as_array())
        {
            for item in items {
                let title = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if title.is_empty() {
                    continue;
                }

                let artist = item
                    .get("artists")
                    .and_then(|a| a.as_array())
                    .and_then(|a| a.first())
                    .and_then(|ar| ar.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Spotify Artist")
                    .to_string();

                let url = item
                    .get("external_urls")
                    .and_then(|u| u.get("spotify"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();

                let thumbnail = item
                    .get("album")
                    .and_then(|al| al.get("images"))
                    .and_then(|imgs| imgs.as_array())
                    .and_then(|imgs| imgs.first())
                    .and_then(|img| img.get("url"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());

                let duration = item
                    .get("duration_ms")
                    .and_then(|d| d.as_u64())
                    .map(Duration::from_millis);

                let stream_url = format!("ytsearch1:{} - {}", artist, title);

                tracks.push(TrackMetadata {
                    title,
                    url,
                    stream_url,
                    duration,
                    thumbnail,
                    author: Some(artist),
                    source: "Spotify".to_string(),
                    requester: None,
                    view_count: None,
                });
            }
        }

        Ok(tracks)
    }

    /// Searches SoundCloud using yt-dlp scsearch.
    pub async fn search_soundcloud(&self, query: &str, limit: usize) -> Result<Vec<TrackMetadata>, String> {
        let sc_query = format!("scsearch{}:{}", limit.min(10), query);
        self.resolve_single_query(&sc_query).await
    }

    /// Analyzes playback history to construct a profile of the server's music taste.
    pub fn analyze_taste(history: &[TrackMetadata]) -> TasteProfile {
        if history.is_empty() {
            return TasteProfile {
                top_artists: vec![],
                dominant_region: "Global".to_string(),
                top_keywords: vec![],
                summary: "Belum ada riwayat lagu. Merekomendasikan lagu viral global terpopuler!".to_string(),
            };
        }

        use std::collections::HashMap;
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        let mut jp_score = 0;
        let mut kr_score = 0;
        let mut id_score = 0;
        let mut en_score = 0;

        let id_keywords = [
            "lagu", "dangdut", "koplo", "galau", "indonesia", "sheila", "tulus", "hindia",
            "fiersa", "denny", "caknan", "mahalini", "tiara", "lyodra", "judika", "noah",
            "dewa", "rossa", "armada", "payung teduh", "nadin", "feast", "fourtwnty", "pamungkas",
        ];

        let genre_keywords = [
            "lofi", "remix", "cover", "acoustic", "rock", "pop", "r&b", "hip hop", "rap",
            "jazz", "city pop", "vocaloid", "ost", "anime", "slowed", "reverb", "phonk",
            "edm", "chill", "metal", "synthwave", "ballad", "piano",
        ];

        let mut keyword_counts: HashMap<String, usize> = HashMap::new();

        for track in history {
            if let Some(ref author) = track.author {
                let a = author.trim();
                if !a.is_empty()
                    && a != "YouTube"
                    && a != "SoundCloud"
                    && a != "Spotify Artist"
                    && a != "Various Artists"
                {
                    *artist_counts.entry(a.to_string()).or_insert(0) += 1;
                }
            }

            let text = format!("{} {}", track.title, track.author.as_deref().unwrap_or(""));
            let text_lower = text.to_lowercase();

            // Detect Japanese (Hiragana, Katakana, CJK Ideographs)
            let has_jp = text.chars().any(|c| {
                (c >= '\u{3040}' && c <= '\u{30ff}') || (c >= '\u{4e00}' && c <= '\u{9faf}')
            });
            if has_jp {
                jp_score += 3;
            }

            // Detect Korean Hangul
            let has_kr = text.chars().any(|c| c >= '\u{ac00}' && c <= '\u{d7af}');
            if has_kr {
                kr_score += 3;
            }

            // Detect Indonesian
            if id_keywords.iter().any(|&k| text_lower.contains(k)) {
                id_score += 2;
            } else {
                en_score += 1;
            }

            for &k in &genre_keywords {
                if text_lower.contains(k) {
                    *keyword_counts.entry(k.to_string()).or_insert(0) += 1;
                }
            }
        }

        // Top artists sorted
        let mut artists_vec: Vec<(String, usize)> = artist_counts.into_iter().collect();
        artists_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top_artists: Vec<String> = artists_vec.into_iter().take(3).map(|(a, _)| a).collect();

        // Top keywords sorted
        let mut kw_vec: Vec<(String, usize)> = keyword_counts.into_iter().collect();
        kw_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top_keywords: Vec<String> = kw_vec.into_iter().take(3).map(|(k, _)| k).collect();

        // Dominant region
        let dominant_region = if jp_score >= kr_score && jp_score >= id_score && jp_score > en_score / 3 {
            "🇯🇵 J-Pop / Anime".to_string()
        } else if kr_score >= jp_score && kr_score >= id_score && kr_score > en_score / 3 {
            "🇰🇷 K-Pop / K-Indie".to_string()
        } else if id_score >= jp_score && id_score >= kr_score && id_score > 0 {
            "🇮🇩 Indonesian Pop / Indie".to_string()
        } else {
            "🌐 International / Pop".to_string()
        };

        let mut parts = vec![dominant_region.clone()];
        if !top_artists.is_empty() {
            parts.push(format!("Top Artist: {}", top_artists.join(", ")));
        }
        if !top_keywords.is_empty() {
            parts.push(format!("Genre: {}", top_keywords.join(", ")));
        }
        let summary = parts.join(" • ");

        TasteProfile {
            top_artists,
            dominant_region,
            top_keywords,
            summary,
        }
    }

    /// Generates music recommendations based on server playback history or custom mood
    /// using weighted probability rarity:
    /// - 40% YouTube
    /// - 30% Spotify
    /// - 30% SoundCloud
    pub async fn get_recommendations(
        &self,
        history: &[TrackMetadata],
        target_count: usize,
        mood: Option<&str>,
    ) -> (TasteProfile, Vec<TrackMetadata>) {
        let mut profile = Self::analyze_taste(history);
        let mut seeds = Vec::new();

        // 1. If custom mood is provided, try AI mood curation first
        if let Some(m) = mood.filter(|s| !s.trim().is_empty()) {
            if self.ai_client.is_enabled() {
                if let Ok(curation) = self.ai_client.curate_mood(m, history).await {
                    profile.summary = format!("🎭 **Mood: \"{}\"**\n{}", m, curation.commentary);
                    seeds = curation.query_seeds;
                }
            }
            if seeds.is_empty() {
                seeds.push(format!("{} songs", m.trim()));
                seeds.push(format!("{} music", m.trim()));
                profile.summary = format!("🎭 **Mood:** \"{}\"", m.trim());
            }
        } else {
            // 2. No custom mood -> Use AI DJ taste commentary if enabled
            if self.ai_client.is_enabled() {
                if let Ok(dj_review) = self.ai_client.review_taste(history).await {
                    profile.summary = format!("{}\n\n*{}*", dj_review, profile.summary);
                }
            }

            // Build list of search seeds based on taste
            for artist in &profile.top_artists {
                seeds.push(format!("{} songs", artist));
                seeds.push(format!("{} hits", artist));
            }

            if profile.dominant_region.contains("J-Pop") {
                seeds.push("J-Pop popular hits".to_string());
                seeds.push("Anime OST popular songs".to_string());
                seeds.push("Vocaloid trending hits".to_string());
            } else if profile.dominant_region.contains("K-Pop") {
                seeds.push("K-Pop trending hits".to_string());
                seeds.push("Korean indie chill".to_string());
            } else if profile.dominant_region.contains("Indonesian") {
                seeds.push("Lagu Indonesia populer hits".to_string());
                seeds.push("Indie Indonesia trending".to_string());
            } else {
                seeds.push("Trending pop hits".to_string());
                seeds.push("Top acoustic hits".to_string());
            }

            for kw in &profile.top_keywords {
                seeds.push(format!("{} music playlist", kw));
            }
        }

        if seeds.is_empty() {
            seeds.push("Popular songs".to_string());
        }

        let is_dup = |cand: &TrackMetadata, existing_batch: &[TrackMetadata]| -> bool {
            let cand_yt_id = Self::extract_youtube_id(&cand.url)
                .or_else(|| Self::extract_youtube_id(&cand.stream_url));

            // Check against history
            for h in history {
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
            }

            // Check against current recommendation batch
            for b in existing_batch {
                if b.title.eq_ignore_ascii_case(&cand.title) {
                    return true;
                }
                if !b.url.is_empty() && b.url == cand.url {
                    return true;
                }
            }

            false
        };

        let mut results = Vec::new();
        let max_loops = target_count * 4;
        let mut loop_count = 0;

        while results.len() < target_count && loop_count < max_loops {
            loop_count += 1;
            let seed_idx = (rand::random::<usize>()) % seeds.len();
            let seed = &seeds[seed_idx];

            // Roll weighted rarity (1..=100)
            // 1..=40: YouTube (40%)
            // 41..=70: Spotify (30%)
            // 71..=100: SoundCloud (30%)
            let roll = (rand::random::<u32>() % 100) + 1;

            let candidate_list = if (41..=70).contains(&roll) {
                // Spotify
                match self.search_spotify(seed, 5).await {
                    Ok(list) if !list.is_empty() => list,
                    _ => self.resolve_single_query(&format!("ytsearch5:{}", seed)).await.unwrap_or_default(),
                }
            } else if roll > 70 {
                // SoundCloud
                match self.search_soundcloud(seed, 5).await {
                    Ok(list) if !list.is_empty() => list,
                    _ => self.resolve_single_query(&format!("ytsearch5:{}", seed)).await.unwrap_or_default(),
                }
            } else {
                // YouTube
                self.resolve_single_query(&format!("ytsearch5:{}", seed)).await.unwrap_or_default()
            };

            for cand in candidate_list {
                if !is_dup(&cand, &results) {
                    results.push(cand);
                    if results.len() >= target_count {
                        break;
                    }
                }
            }
        }

        (profile, results)
    }
}
