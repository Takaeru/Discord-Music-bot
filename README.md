# 🎵 Discord Music Bot (Rust + Songbird)

A high-performance, ultra-lightweight, and self-hosted Discord Music Bot built with **Rust**, **Serenity 0.12**, and **Songbird 0.6**.

Supports Discord's **DAVE (End-to-End Encrypted Voice)** protocol natively with direct **`yt-dlp`** streaming and full audio codec decoding.

---

## ✨ Features

- ⚡ **Ultra Lightweight**: Consumes only **~5-10 MB RAM** and **<0.25% CPU** (compared to Java/Lavalink taking 500MB+).
- 🔒 **DAVE / E2EE Compliant**: Fully compatible with Discord's mandatory voice end-to-end encryption protocol.
- 🔓 **No YouTube Login/Session Required**: Works out of the box without cookies, OAuth, or YouTube account sessions.
- 🔁 **Flexible Repeat Modes**: Loop a single track (`/repeat mode:track`) or cycle the entire queue indefinitely (`/repeat mode:queue`).
- 🎼 **Multi-Source & Smart Playlists**:
  - Direct YouTube links, searches, mixes, and playlists.
  - Smart `--no-playlist` protection: single video links with YouTube Mix (`&list=RD...`) attached will only queue the targeted song.
  - Official playlists (`/playlist?list=PL...`) automatically capped at 50 songs to prevent queue overflow.
- 🎛️ **Full Audio Codec Support**: AAC, M4A/ISOMP4, MP3, WebM/MKV, Opus, FLAC, Vorbis via pure Rust Symphonia.
- 🐳 **Single Standalone Docker Container**: Zero external services needed (Lavalink, NodeLink, Java, and NodeJS eliminated).

---

## 📋 Slash Commands

| Command | Description | Example |
| :--- | :--- | :--- |
| `/play <query>` | Play audio from URL or search keyword | `/play yoasobi idol` or `/play https://...` |
| `/pause` | Pause currently playing track | `/pause` |
| `/resume` | Resume paused track | `/resume` |
| `/skip` | Skip to the next track in queue | `/skip` |
| `/repeat <mode>` | Set repeat mode: `off`, `track` (1 song), or `queue` (all songs) | `/repeat mode:track` or `/repeat mode:queue` |
| `/loop <mode>` | Alias for `/repeat` | `/loop mode:queue` |
| `/stop` | Stop playback and clear the queue | `/stop` |
| `/queue` | View current queue list, repeat mode, and total duration | `/queue` |
| `/nowplaying` | Show details, playback status, and active loop mode | `/nowplaying` |
| `/volume <1-200>` | Adjust audio playback volume | `/volume 80` |
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

Set your bot token in `.env`:
```env
DISCORD_BOT_TOKEN=your_discord_bot_token_here
LOG_LEVEL=INFO
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
    ├── main.rs       # Bot entrypoint & Gateway connection
    ├── handler.rs    # Serenity interaction handler & Slash command registration
    ├── commands.rs   # Music slash commands (/play, /repeat, /queue, /volume, etc.)
    ├── source.rs     # Metadata extraction, yt-dlp parsing & playlist filters
    └── queue.rs      # Guild queue state & LoopMode manager
```