# 🎵 Discord Music Bot (Rust + Songbird)

A high-performance, ultra-lightweight, and self-hosted Discord Music Bot built with **Rust**, **Serenity 0.12**, and **Songbird 0.6**.

Supports Discord's **DAVE (End-to-End Encrypted Voice)** protocol natively with direct **`yt-dlp`** streaming, Spotify embed parsing, instant playlist enqueueing, and full audio codec decoding.

---

## ✨ Features

- ⚡ **Ultra Lightweight**: Consumes only **~5-10 MB RAM** and **<0.25% CPU** (compared to Java/Lavalink taking 500MB+).
- 🚀 **Instant Playlist Enqueueing (Just-In-Time Streaming)**:
  - Playlists and mixes (YouTube/Spotify) are enqueued into the queue **instantly (< 1 second)**.
  - Audio streams are extracted **Just-In-Time (JIT)** right when the song's turn arrives, eliminating long loading times and preventing expired stream URLs.
- 🛡️ **Unlimited Playlists**: Playlists and Mixes load fully (no track cap) — Let YouTube Mixes grow as long as they want.
- 🔒 **DAVE / E2EE Compliant**: Fully compatible with Discord's mandatory voice end-to-end encryption protocol.
- 🔓 **No YouTube Login/Session Required**: Works out of the box without cookies, OAuth, or YouTube account sessions.
- 🎼 **Multi-Platform Support**:
  - **YouTube**: Direct video URLs, search queries, playlists, and YouTube Mix/Radio links (`&list=RD...`).
  - **Spotify**: Tracks, albums, and playlists resolved with high-res album art and matched audio streams.
  - **SoundCloud**: Tracks and artist searches supported.
- 🎨 **Dynamic Rich Embeds & Interactive Controls**:
  - Platform-colored embeds (Spotify Green, YouTube Red, SoundCloud Orange) with high-res thumbnails.
  - **Interactive `/queue`**:
    - 📑 **Pagination Buttons**: `◀️ Prev`, `Page X/Y`, `Next ▶️` to browse through large queues smoothly.
    - 🎵 **Direct Song Selection (Select Menu)**: Jump directly to any track in the queue by choosing from a dropdown list without typing commands.
    - 🎛️ **Quick Controls**: Built-in `⏭️ Skip` and `⏹️ Stop` buttons directly on the queue embed.
  - **Interactive `/play` Search**:
    - 🔍 Search returns top 10 YouTube candidates as a numbered embed.
    - 🎵 Dropdown selector lets you pick a track to play directly in Discord.
    - 👁️ View count displayed per result to help identify official versions over covers.
- 🌐 **Bilingual Support (English / Indonesian)**:
  - All user-facing text translated via `BOT_LANG` environment variable.
  - `BOT_LANG=en` (default) — English
  - `BOT_LANG=id` — Indonesian
  - Control buttons stay in English even in Indonesian mode (standard music UI).
- 🛡️ **Race-Condition-Free Multi-User**:
  - Search results keyed by `MessageId` so two users can `/play` in the same guild without interference.
  - Voice channel membership enforced on dropdown selection.
  - TTL cleanup (5 minutes) prevents abandoned search results from leaking memory.
- ⏱️ **Hang Protection**:
  - yt-dlp search subprocess: 30s timeout.
  - Stream URL resolution: 20s timeout.
  - Discord interactions always defer before long work to avoid 3s response timeout.
- 🐳 **Single Standalone Docker Container**: Zero external services needed (Lavalink, NodeLink, Java, and NodeJS eliminated).

---

## 📋 Slash Commands

| Command | Description | Example |
| :--- | :--- | :--- |
| `/play <query>` | Play audio from YouTube, Spotify, SoundCloud, or search keywords | `/play yoasobi idol` or `/play https://open.spotify.com/...` |
| `/playnext <query>` | Add a song to play next (priority queue position 1) | `/playnext kalafina magia` |
| `/pause` | Pause currently playing track | `/pause` |
| `/resume` | Resume playback of paused track | `/resume` |
| `/skip` | Skip to the next track in queue | `/skip` |
| `/replay` | Replay the current track from the beginning | `/replay` |
| `/shuffle` | Toggle random / shuffle mode on or off | `/shuffle` |
| `/repeat <mode>` | Set repeat mode: `off`, `track` (1 song), or `queue` (all songs) | `/repeat mode:track` or `/repeat mode:queue` |
| `/loop <mode>` | Alias for `/repeat` | `/loop mode:queue` |
| `/stop` | Stop playback and clear the queue | `/stop` |
| `/queue` | View current queue, platform sources, repeat mode, and total duration | `/queue` |
| `/nowplaying` | Show details, platform source, thumbnail, and active loop mode | `/nowplaying` |
| `/jump <pos>` | Jump to a specific position in the queue | `/jump 5` |
| `/remove <pos>` | Remove a specific track from the queue | `/remove 3` |
| `/clear` | Clear the entire queue (keeps current track playing) | `/clear` |
| `/volume <0-100>` | Adjust audio playback volume | `/volume 80` |
| `/leave` | Disconnect bot from the voice channel | `/leave` |
| `/ping` | Show bot latency | `/ping` |
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

# Language: en (default) or id (Indonesian)
BOT_LANG=en

# Now Playing behavior: old (default, history) or new (clean channel)
NOW_PLAYING_BEHAVIOR=old
```

### 3. Run with Docker
```bash
docker compose up -d
```

The bot will register 19 global slash commands on first startup (takes ~1 hour for Discord to propagate globally, or use a test guild for instant registration).

---

## 🌐 Multi-Language

| Code | Language | Notes |
|:---|:---|:---|
| `en` (default) | English | All UI text in English |
| `id` | Indonesian (Bahasa) | All UI text translated; control buttons stay English |

The bot reads `BOT_LANG` environment variable at startup. To switch language, change the env var and restart the container:

```bash
# Switch to Indonesian
docker compose down
BOT_LANG=id docker compose up -d
```

### Translating to a new language

Edit `src/lang.rs` and add a new `Lang` static instance following the `EN` / `ID` pattern. Then update the `get_lang()` function to detect your new code. All ~120 user-facing strings are listed in one place for easy translation.

---

## 🎴 Now Playing Card Behavior

The Now Playing card shows the current track with playback controls (skip, stop, loop). As tracks advance, the bot reposts the card — you can choose how it handles the *previous* card via `NOW_PLAYING_BEHAVIOR`:

### `old` (default) — keep history

Every track posts a **new card** in the channel, and all previous cards stay behind it as a scrollable history.

**Good for:** watching what was played, never missing a track, easy to click through past songs.

```
┌───────────────┐   ┌───────────────┐
│ ▶ Track 1     │   │ ▶ Track 2     │  ← both cards remain
└───────────────┘   └───────────────┘
```

### `new` — clean channel

Each new card **deletes the previous one** before posting, so only the latest track is ever visible. When the queue finishes, the last card is edited in-place into a "finished" notice.

**Good for:** a tidy channel, only ever showing what's playing right now.

```
┌───────────────┐   ┌───────────────┐
│ ▶ Track 2     │   │ ⏹ Finished    │  ← single card, keeps updating
└───────────────┘   └───────────────┘
```

### Switching

```bash
# Old behavior (default) — keep card history
docker compose up -d

# New behavior — clean channel
docker compose down
NOW_PLAYING_BEHAVIOR=new docker compose up -d
```

No restart needed to keep `old` (it's the default); the env var is only read at startup, so a restart is required to change it.

---

## 🐛 Self-Hosting Notes

- **First-run slash commands** take ~1 hour to register globally. For development, change the `Command::set_global_commands` call to `Command::create_global_command` on a test guild for instant registration.
- **Voice connections** are 5-min idle auto-disconnect (configurable in `src/utils/voice.rs`).
- **Search results** auto-expire after 5 minutes if the user doesn't pick a track.
- **Multi-user safety** is built-in: two users can `/play` in the same guild without race conditions.

---

## 📜 License

MIT
