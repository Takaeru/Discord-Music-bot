# 🎵 Discord Music Bot (Rust + Songbird)

A high-performance, ultra-lightweight, and self-hosted Discord Music Bot built with **Rust**, **Serenity 0.12**, and **Songbird 0.6**.

Supports Discord's **DAVE (End-to-End Encrypted Voice)** protocol natively with direct **`yt-dlp`** streaming, Spotify embed parsing, instant playlist enqueueing, and full audio codec decoding.

---

## ✨ Features

- ⚡ **Ultra Lightweight**: Consumes only **~5-10 MB RAM** and **<0.25% CPU** (compared to Java/Lavalink taking 500MB+).
- 🚀 **Instant Playlist Enqueueing (Just-In-Time Streaming)**:
  - Playlists and mixes (YouTube/Spotify) are enqueued into the queue **instantly (< 1 second)**.
  - Audio streams are extracted **Just-In-Time (JIT)** right when the song's turn arrives, eliminating long loading times and preventing expired stream URLs.
- 🔒 **DAVE / E2EE Compliant**: Fully compatible with Discord's mandatory voice end-to-end encryption protocol.
- 🔓 **No YouTube Login/Session Required**: Works out of the box without cookies, OAuth, or YouTube account sessions.
- 🎼 **Multi-Platform Support**:
  - **YouTube**: Direct video URLs, search queries, playlists, and YouTube Mix/Radio links (`&list=RD...`).
  - **Spotify**: Tracks, albums, and playlists resolved with high-res album art and matched audio streams.
  - **SoundCloud**: Tracks and artist searches supported.
- 🎨 **Dynamic Rich Embeds**:
  - Platform-colored embeds (Spotify Green, YouTube Red, SoundCloud Orange).
  - Official platform logo images in Author & Footer icons, plus high-res track thumbnails.
  - Clean `/queue` view with Now Playing highlight, track positions, total duration, and optional custom emoji logos.
- 🔁 **Flexible Repeat Modes**: Loop a single track (`/repeat mode:track`) or cycle the entire queue indefinitely (`/repeat mode:queue`).
- 🛡️ **Smart Playlist Capping**: Playlists and Mixes are capped at 20 tracks to prevent queue overload and keep memory footprint low.
- 🎛️ **Full Audio Codec Support**: AAC, M4A/ISOMP4, MP3, WebM/MKV, Opus, FLAC, Vorbis via pure Rust Symphonia.
- 🐳 **Single Standalone Docker Container**: Zero external services needed (Lavalink, NodeLink, Java, and NodeJS eliminated).

---

## 📋 Slash Commands

| Command | Description | Example |
| :--- | :--- | :--- |
| `/play <query>` | Play audio from YouTube, Spotify, SoundCloud, or search keywords | `/play yoasobi idol` or `/play https://open.spotify.com/...` |
| `/pause` | Pause currently playing track | `/pause` |
| `/resume` | Resume playback of paused track | `/resume` |
| `/skip` | Skip to the next track in queue | `/skip` |
| `/repeat <mode>` | Set repeat mode: `off`, `track` (1 song), or `queue` (all songs) | `/repeat mode:track` or `/repeat mode:queue` |
| `/loop <mode>` | Alias for `/repeat` | `/loop mode:queue` |
| `/stop` | Stop playback and clear the queue | `/stop` |
| `/queue` | View current queue, platform sources, repeat mode, and total duration | `/queue` |
| `/nowplaying` | Show details, platform source, thumbnail, and active loop mode | `/nowplaying` |
| `/volume <0-100>` | Adjust audio playback volume | `/volume 80` |
| `/leave` | Disconnect bot from the voice channel | `/leave` |
| `/help` | Show command overview and usage | `/help` |

---

## 🚀 Getting Started

### 1. Prerequisites
- [Docker](https://www.docker.com/) & [Docker Compose](https://docs.docker.com/compose/)
- Discord Bot Token with the following **Privileged Gateway Intents** enabled in [Discord Developer Portal](https://discord.com/developers/applications):
  - `SERVER MEMBERS INTENT`
  - `MESSAGE CONTENT INTENT`

### 2. Configuration
Copy `.env.example` to `.env`:
```bash
cp .env.example .env
```

Set your configuration in `.env`:
```env
DISCORD_BOT_TOKEN=your_discord_bot_token_here
LOG_LEVEL=INFO

# (Optional) Custom Emojis for inline text rendering (e.g. <:spotify:123456789012345678>)
EMOJI_SPOTIFY=
EMOJI_YOUTUBE=
EMOJI_SOUNDCLOUD=
```

### 3. Run with Docker Compose
```bash
# Build and run container in detached mode
docker compose up -d --build

# Check container status
docker compose ps

# View live logs
docker compose logs -f discord-bot

# Stop container
docker compose down
```

---

## 🌐 VPS / Ubuntu Server Deployment

To run this bot on an Ubuntu VPS:

1. **Clone repository**:
   ```bash
   git clone <your-repo-url> /opt/discord-bot
   cd /opt/discord-bot
   ```
2. **Setup environment**:
   ```bash
   cp .env.example .env
   nano .env # Paste your DISCORD_BOT_TOKEN
   ```
3. **Start the bot**:
   ```bash
   docker compose up -d --build
   ```

---

## 🛠️ Project Structure

```
discord-bot/
├── .env.example
├── .gitignore
├── .dockerignore
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── docker-compose.yml
├── README.md
└── src/
    ├── main.rs            # Bot entrypoint, tracing & Gateway connection
    ├── handler.rs         # Serenity interaction handler & Slash command registration
    ├── queue.rs           # Guild queue state & LoopMode manager
    ├── source.rs          # Metadata extraction (yt-dlp, Spotify Embed API)
    ├── commands/          # Modular slash command handlers
    │   ├── mod.rs         # Command router & register_commands
    │   ├── play.rs        # /play logic (instant enqueue & single tracks)
    │   ├── queue.rs       # /queue & /nowplaying embed renderers
    │   ├── control.rs     # /pause, /resume, /skip, /stop, /repeat, /volume, /leave, /help
    │   └── events.rs      # Songbird TrackEndHandler & Just-In-Time (JIT) stream loader
    └── utils/             # Helper utilities
        ├── mod.rs
        ├── embed.rs       # Platform colors, source icons, duration formatting
        └── response.rs    # Interaction response & followup helpers
```