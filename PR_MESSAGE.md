# Discord Music Bot — Multi-Language Support, UX Improvements & Bug Fixes

## Summary

This PR adds bilingual support (English/Indonesian), improves search result handling with race condition fixes, enhances the skip/queue interaction reliability, and fixes several edge cases around multi-user usage and empty queue states.

---

## ✨ New Features

### Interactive Search & Select (`/play` command)
- `/play <query>` now searches YouTube and returns a list of candidates instead of auto-playing the top result
- Search results displayed as an embed with **numbered clickable links** (title + URL) for quick browser access
- **Dropdown selector** below the embed lets users pick a track to play directly in Discord
- Dropdown labels show numbered title + artist + duration for quick scanning
- Embed links and dropdown numbers are visually matched (1→1, 2→2, etc.) for consistency
- View count parsed from yt-dlp metadata and displayed in embed (e.g., "686.6M views") to help identify official versions over covers
- `format_views()` helper for human-readable view counts (K/M/B suffixes)
- `view_count: Option<u64>` added to `TrackMetadata` in `source.rs`

### Multi-Language Support (`src/lang.rs`)
- Added `BOT_LANG` environment variable to select language at runtime
  - `BOT_LANG=en` (default) — English
  - `BOT_LANG=id` — Indonesian
- New `src/lang.rs` module with 120+ translatable string fields covering all user-facing text
- `Lang` struct with `EN` and `ID` static instances, selected via `LazyLock` at startup
- `fmt()` helper function for runtime format string interpolation (Rust's `format!` requires string literals)
- **Indonesian mode keeps control buttons in English** (Resume, Pause, Skip, Loop, Stop) per design requirement
- All slash command descriptions, responses, error messages, embed fields, button labels, and queue view text are fully translated

---

## 🐛 Bug Fixes

### Fix 1: Skip Command Race Condition (Critical)
**File:** `src/commands/control.rs`
- **Problem:** `music_skip` used a fixed 250ms `tokio::time::sleep` before checking `get_current()`. If yt-dlp resolution took longer (2-8s), the interaction would timeout (Discord's 3s limit) and the "Now Playing" response would fail.
- **Fix:** Added `component.defer()` before stopping the track, then polls `queue_mgr.get_current()` every 200ms (up to 10s) until the track changes. Uses `create_followup` instead of `UpdateMessage` to correctly send the new track info.

### Fix 2: Queue Play/Jump Wrong Response Target
**File:** `src/commands/queue.rs`
- **Problem:** `queue_play` and `queue_jump` handlers called `component.defer()` then used `UpdateMessage` response type. After deferring, `UpdateMessage` updates the deferred placeholder, not the original queue message — leaving the queue message unchanged.
- **Fix:** Changed to `create_followup` after defer, which correctly sends a new message with the updated queue.

### Fix 3: Empty Queue Panic in `build_queue_view`
**File:** `src/commands/queue.rs`
- **Problem:** `build_queue_view` accessed `queue[0]` without bounds checking. If the queue was emptied by a race condition between the caller's check and this function call, it would panic.
- **Fix:** Added early return guard — if `queue.is_empty()`, returns a safe empty embed with no components.

### Fix 4: Language Inconsistency
**File:** `src/commands/events.rs`
- **Problem:** `TrackEndHandler` used Indonesian text `"Antrean telah selesai diputar."` while the rest of the bot was in English.
- **Fix:** Now uses `get_lang().queue_finished_playing` for consistent language selection.

### Fix 5: Stale `get_current()` After Skip
**File:** `src/commands/control.rs`
- **Problem:** After calling `stop()` on the current track, `get_current()` could still return the old track if the `TrackEndHandler` hadn't fired yet within the 250ms sleep window.
- **Fix:** Poll loop compares `get_current()` against the pre-skip track, waiting until it actually changes before responding.

### Fix 6: Multi-User Search Race Condition
**File:** `src/queue.rs`, `src/commands/play.rs`, `src/commands/mod.rs`
- **Problem:** Search results were keyed by `GuildId`. If two users in the same guild both ran `/play`, the second user's results would overwrite the first's. Selecting a track from the first user's dropdown would fail or play the wrong song.
- **Fix:** Changed `search_results` from `HashMap<GuildId, Vec<TrackMetadata>>` to `HashMap<MessageId, SearchEntry>`. Each search result set is keyed by the message ID of the bot's response, eliminating cross-user interference.

### Fix 7: Abandoned Search Results Memory Leak
**File:** `src/queue.rs`
- **Problem:** Search results were never cleaned up if a user searched but never selected a track.
- **Fix:** Added `SearchEntry` struct with `created_at: Instant` field. TTL cleanup runs on every `set_search_results` and `get_search_results` call, removing entries older than 5 minutes (`SEARCH_TTL_SECS = 300`).

### Fix 8: Duplicate Dropdown Click
**File:** `src/commands/mod.rs`, `src/queue.rs`
- **Problem:** A user could click the same search result dropdown multiple times, adding the same track to the queue repeatedly.
- **Fix:** Added `remove_search_results(msg_id)` method. After retrieving a track from search results, the entry is immediately consumed (deleted). Subsequent clicks on the same dropdown find no results and show an error.

### Fix 9: Voice Channel Enforcement on Dropdown
**File:** `src/commands/mod.rs`
- **Problem:** Any user could click a search result dropdown and add tracks, even if they weren't in a voice channel.
- **Fix:** Added `check_voice_channel()` call in `handle_search_play` before processing the selection. Users not in a voice channel see an error message.

### Fix 10: Search Results Only Returning 1 Entry
**File:** `src/source.rs`
- **Problem:** `resolve_single_query()` called `entries.into_iter().next()` which only took the first of 10 search results from `ytsearch10:`.
- **Fix:** Changed to `for entry in entries.into_iter()` to return all candidates.

### Fix 11: `/leave` Not Clearing Queue
**File:** `src/commands/control.rs`
- **Problem:** `queue_mgr.clear()` ran after `manager.leave()`, but `TrackEndHandler` fired async and re-populated the queue via `advance()`/`cycle_queue()`.
- **Fix:** Moved `queue_mgr.clear()` before `manager.leave()` so the queue is empty when `TrackEndHandler` fires.

### Fix 12: Now Playing Returning Wrong Result
**File:** `src/queue.rs`, `src/commands/events.rs`, `src/commands/play.rs`
- **Problem:** `get_current()` returned `queue.front()`, but `advance()` popped the front track before the next track actually started playing in songbird. There was a window where `get_current()` returned a track that was still loading.
- **Fix:** Added `current_track` field to `QueueManager` with `set_current_track()` method. `get_current()` now checks `current_track` first (authoritative), falls back to `queue.front()`. `TrackEndHandler` and `play.rs` call `set_current_track()` after `enqueue_input` succeeds.

### Fix 13: "Application Did Not Respond" on Multiple Commands
**File:** `src/commands/control.rs`, `src/commands/queue.rs`
- **Problem:** `handle_pause`, `handle_resume`, `handle_stop`, and `handle_nowplaying` did not defer before calling `songbird::get()` + `handler.lock()` which can exceed Discord's 3s interaction timeout.
- **Fix:** Added `command.defer(&ctx.http).await` before slow operations. Changed `send_response` → `send_followup` and `create_response` → `create_followup` after defer.

### Fix 14: `resume()` Method Didn't Exist
**File:** `src/commands/control.rs`
- **Problem:** `handle_resume` called `current.resume()` which doesn't exist on `TrackHandle`.
- **Fix:** Changed to `current.play()` which is the correct songbird API for resuming a paused track.

### Fix 15: Now Playing Deleting Old Message
**File:** `src/commands/events.rs`
- **Problem:** `TrackEndHandler` deleted the old "now playing" message before sending the new one, making it hard to track what was played.
- **Fix:** Removed the delete step. Old "now playing" messages stay in chat; each new track sends a fresh message.

---

## 📁 Files Changed

| File | Changes |
|------|---------|
| `src/lang.rs` | **NEW** — Multi-language module (120+ strings × 2 languages) |
| `src/main.rs` | Added `mod lang;` declaration |
| `src/queue.rs` | `SearchEntry` struct, MessageId-keyed search, TTL cleanup, `remove_search_results`, `retain` in `clear()`, `current_track` field + `set_current_track()`, LoopMode `as_str()` uses lang |
| `src/commands/play.rs` | Stores search results with message ID after followup, numbered embed/dropdown, view count display, `set_current_track()` on first enqueue, all strings via `get_lang()` |
| `src/commands/mod.rs` | MessageId lookup in `handle_search_play`, voice channel check, consume-on-select, all slash command descriptions via `get_lang()` |
| `src/commands/control.rs` | Defer + poll pattern for skip, `create_followup` for replay, defer on pause/resume/stop, `resume()` → `play()`, queue clear before leave, all responses via `get_lang()` + `fmt()` |
| `src/commands/queue.rs` | Empty queue guard, `create_followup` for play/jump/skip, defer on nowplaying, all strings via `get_lang()` + `fmt()` |
| `src/commands/events.rs` | Uses `get_lang()` for queue finished message, `set_current_track()` after enqueue, removed old message delete |
| `src/utils/embed.rs` | Now-playing embed fields via `get_lang()`, button labels via `get_lang()` |
| `src/source.rs` | Added `view_count: Option<u64>`, fixed search to return all 10 candidates |
| `Cargo.toml` | Edition 2024 (for `LazyLock` support) |

---

## 🔧 Configuration

```bash
# In .env or docker-compose.yml:
BOT_LANG=en   # English (default)
BOT_LANG=id   # Indonesian
```

---

## ⚠️ Breaking Changes

- `search_results` type changed from `HashMap<GuildId, Vec<TrackMetadata>>` to `HashMap<MessageId, SearchEntry>`. Any external code directly accessing `search_results` will need updating.
- Rust edition bumped to 2024 for `LazyLock` support. Minimum Rust version: 1.80+.

---

## Testing

- `cargo check --features vendored-openssl` — ✅ clean
- `cargo build --release --features vendored-openssl` — ✅ clean (35MB binary)
- All 120+ translatable strings verified in both EN and ID
- No hardcoded user-facing strings remain outside `src/lang.rs`
