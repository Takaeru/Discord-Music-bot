# Discord Music Bot

A production-quality Discord Music Bot written in Go, powered by [DiscordGo](https://github.com/bwmarrin/discordgo), [FFmpeg](https://ffmpeg.org/), and [yt-dlp](https://github.com/yt-dlp/yt-dlp).

---

## 1. Project Overview

`discord-music-bot` is a lightweight, concurrent audio streaming bot designed for Discord servers. It streams audio directly through external transcoding tools (`yt-dlp` and `FFmpeg`) into Discord voice channels with zero unnecessary temporary disk files. Each Discord guild maintains an isolated player, queue, and volume state.

---

## 2. Architecture

```
Discord User (Slash Commands)
    │
    ▼
Discord Bot Gateway (DiscordGo)
    │
    ├── Interaction Router (/play, /pause, /resume, /skip, /stop, /queue, /nowplaying, /volume, /join, /leave)
    │
    └── Guild Music Manager (sync.RWMutex per-guild state isolation)
            │
            ├── Track Queue (Thread-Safe FIFO Queue)
            │
            ├── Audio Source Layer (yt-dlp metadata extraction)
            │       │
            │       ▼
            ├── Transcoding Pipeline (FFmpeg DCA/PCM streaming)
            │       │
            │       ▼
            └── Discord Voice Connection (Opus packet transmission)
```

---

## 3. Requirements

### Native / Local Development
- **Go**: Version `1.22` or later (tested on Go `1.24`)
- **FFmpeg**: Installed and available on `PATH` (or configured via `FFMPEG_PATH`)
- **yt-dlp**: Installed and available on `PATH` (or configured via `YTDLP_PATH`)

### Containerized Deployment
- **Docker** and **Docker Compose** (Compose v2 supported)

---

## 4. Discord Developer Portal Setup

1. Open the [Discord Developer Portal](https://discord.com/developers/applications).
2. Click **New Application** and enter a name (e.g., `Discord Music Bot`).
3. Navigate to the **Bot** tab on the left sidebar:
   - Click **Reset Token** to generate a new bot token.
   - Copy the token and save it for your `.env` file (`DISCORD_BOT_TOKEN`).
4. Under **Privileged Gateway Intents**:
   - The bot requires only standard intents (`Guilds` and `Guild Voice States`). Privileged intents (such as *Message Content Intent*) are **not required**.
5. Navigate to **OAuth2** -> **URL Generator**:
   - Under **Scopes**, check:
     - `bot`
     - `applications.commands`
   - Under **Bot Permissions**, select:
     - `Connect` (Voice)
     - `Speak` (Voice)
     - `Send Messages` (Text)
     - `Embed Links` (Text)
     - `View Channels` (General)
6. Copy the generated URL at the bottom and paste it into your browser to invite the bot to your server.

---

## 5. Bot Permissions

| Permission | Category | Purpose |
| :--- | :--- | :--- |
| `Connect` | Voice | Allows the bot to join voice channels |
| `Speak` | Voice | Allows the bot to transmit audio into voice channels |
| `Send Messages` | Text | Allows the bot to respond to slash command interactions |
| `Embed Links` | Text | Allows formatting rich embeds for `/queue` and `/nowplaying` |
| `Use Application Commands` | General | Allows members to see and execute slash commands |

---

## 6. Environment Variables

Create a `.env` file in the root directory (copied from `.env.example`):

```bash
cp .env.example .env
```

| Variable | Required | Default | Description |
| :--- | :---: | :---: | :--- |
| `DISCORD_BOT_TOKEN` | **Yes** | - | Authentication token from Discord Developer Portal |
| `LOG_LEVEL` | No | `INFO` | Logging verbosity (`DEBUG`, `INFO`, `WARN`, `ERROR`) |
| `FFMPEG_PATH` | No | `ffmpeg` | Path to FFmpeg executable |
| `YTDLP_PATH` | No | `yt-dlp` | Path to yt-dlp executable |

---

## 7. Local Installation

```bash
# Clone the repository
git clone <repository_url>
cd discord-bot

# Download and verify Go dependencies
go mod download
go mod tidy

# Copy environment template
cp .env.example .env
```

Edit `.env` and set your `DISCORD_BOT_TOKEN`.

---

## 8. FFmpeg Installation

### Windows
```powershell
# Using winget
winget install Gyan.FFmpeg

# Or using Chocolatey
choco install ffmpeg
```

### macOS
```bash
brew install ffmpeg
```

### Linux (Ubuntu / Debian)
```bash
sudo apt update
sudo apt install -y ffmpeg
```

Verify installation:
```bash
ffmpeg -version
```

---

## 9. yt-dlp Installation

### Windows
```powershell
# Using winget
winget install yt-dlp.yt-dlp

# Or using Python pip
python -m pip install -U yt-dlp
```

### macOS
```bash
brew install yt-dlp
```

### Linux (Ubuntu / Debian)
```bash
sudo apt update
sudo apt install -y yt-dlp
# Or install standalone binary
sudo wget https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp
```

Verify installation:
```bash
yt-dlp --version
```

---

## 10. Running Locally

```bash
# Run the application
go run ./cmd/bot
```

To build a standalone executable:
```bash
# Build binary
go build -o bin/bot.exe ./cmd/bot

# Run binary
./bin/bot.exe
```

---

## 11. Docker Deployment

The application includes a multi-stage `Dockerfile` and `docker-compose.yml` pre-configured with `ffmpeg`, `yt-dlp`, and a non-root runtime container.

```bash
# 1. Validate Docker Compose configuration
docker compose config

# 2. Build and launch container in detached mode
docker compose up -d --build

# 3. View live structured logs
docker compose logs -f

# 4. Stop and remove container
docker compose down
```

---

## 12. Commands

All commands are registered as Discord Slash Commands:

| Command | Option | Description |
| :--- | :--- | :--- |
| `/join` | - | Connects the bot to your current voice channel |
| `/leave` | - | Disconnects the bot from voice and clears the queue |
| `/play` | `query` *(required string)* | Plays audio from a YouTube URL or searches keywords |
| `/pause` | - | Pauses the currently playing track |
| `/resume` | - | Resumes the paused track |
| `/skip` | - | Skips the current track and starts the next queued song |
| `/stop` | - | Stops playback and clears the guild queue (remains in channel) |
| `/queue` | - | Displays current track and upcoming song queue |
| `/nowplaying` | - | Displays detailed track progress, requester, and thumbnail |
| `/volume` | `volume` *(required integer 0-100)* | Adjusts playback volume for the current server |

---

## 13. Troubleshooting

### 1. Bot doesn't join voice channel
- Ensure you are inside an active voice channel in the same server.
- Verify the bot has `Connect` and `Speak` permissions for that specific voice channel.

### 2. "yt-dlp executable was not found" or "FFmpeg was not found"
- Verify that `ffmpeg` and `yt-dlp` are installed and added to your system's `PATH` environment variable.
- Alternatively, specify the full path directly in `.env`:
  ```env
  FFMPEG_PATH=C:\ffmpeg\bin\ffmpeg.exe
  YTDLP_PATH=C:\yt-dlp\yt-dlp.exe
  ```

### 3. "DISCORD_BOT_TOKEN is required"
- Ensure `.env` exists in the root directory and contains `DISCORD_BOT_TOKEN=<your_token>` without quotes.

### 4. Audio stutters or drops
- Ensure adequate network bandwidth for outbound UDP voice streaming.
- Docker containers have network reconnection parameters enabled by default (`-reconnect 1`).

---

## 14. Development Guide

### Project Structure
```
discord-music-bot/
├── cmd/
│   └── bot/
│       └── main.go          # Application startup & signal handling
├── internal/
│   ├── config/
│   │   ├── config.go        # Environment loader & validator
│   │   └── config_test.go   # Config unit tests
│   ├── discord/
│   │   ├── bot.go           # Discord gateway lifecycle
│   │   ├── bot_test.go      # Bot lifecycle unit tests
│   │   ├── commands.go      # Slash command definitions & handlers
│   │   ├── commands_test.go # Command option & handler tests
│   │   ├── voice.go         # Voice state & permission checks
│   │   └── voice_test.go    # Voice channel unit tests
│   └── music/
│       ├── ffmpeg.go        # FFmpeg audio transcoding pipeline
│       ├── ffmpeg_test.go   # FFmpeg pipeline unit tests
│       ├── manager.go       # Multi-guild music state isolation
│       ├── manager_test.go  # Guild isolation & concurrency tests
│       ├── player.go        # Playback loop, streaming & controls
│       ├── player_test.go   # Player state & control unit tests
│       ├── queue.go         # Thread-safe FIFO track queue
│       ├── queue_test.go    # Queue concurrency & edge tests
│       ├── source.go        # Audio source abstraction (yt-dlp)
│       ├── source_test.go   # Source resolver mock tests
│       ├── track.go         # Domain model for audio tracks
│       └── track_test.go    # Track serialization unit tests
├── .dockerignore
├── .env.example
├── .gitignore
├── Dockerfile
├── docker-compose.yml
├── go.mod
├── go.sum
└── README.md
```

### Running Tests and Static Analysis
```bash
# Format codebase
go fmt ./...

# Run static analysis
go vet ./...

# Run complete unit test suite
go test -v ./...
```