use std::sync::LazyLock;

pub struct Lang {
    // === Slash command descriptions ===
    pub cmd_play: &'static str,
    pub cmd_pause: &'static str,
    pub cmd_resume: &'static str,
    pub cmd_skip: &'static str,
    pub cmd_replay: &'static str,
    pub cmd_stop: &'static str,
    pub cmd_queue: &'static str,
    pub cmd_nowplaying: &'static str,
    pub cmd_clear: &'static str,
    pub cmd_remove: &'static str,
    pub cmd_jump: &'static str,
    pub cmd_shuffle: &'static str,
    pub cmd_repeat: &'static str,
    pub cmd_volume: &'static str,
    pub cmd_playnext: &'static str,
    pub cmd_leave: &'static str,
    pub cmd_ping: &'static str,
    pub cmd_help: &'static str,
    pub cmd_seek: &'static str,
    pub cmd_lyrics: &'static str,
    pub cmd_filter: &'static str,
    pub cmd_autoplay: &'static str,
    pub cmd_playlist: &'static str,
    pub cmd_history: &'static str,

    // === Command responses ===
    pub playback_paused: &'static str,
    pub playback_resumed: &'static str,
    pub skipped_current: &'static str,
    pub stopped_and_cleared: &'static str,
    pub volume_set: &'static str,        // has {}
    pub disconnected: &'static str,
    pub shuffle_enabled: &'static str,
    pub shuffle_disabled: &'static str,
    pub cleared_n_tracks: &'static str,  // has {}
    pub removed_track: &'static str,     // has #{} {} {}
    pub jumped_to: &'static str,         // has #{} {} {}
    pub repeat_mode_set: &'static str,   // has {} {}
    pub seek_success: &'static str,      // has {}
    pub lyrics_title: &'static str,      // has {} {}
    pub lyrics_footer: &'static str,     // has {}
    pub filter_set: &'static str,        // has {}
    pub filter_disabled: &'static str,
    pub autoplay_enabled: &'static str,
    pub autoplay_disabled: &'static str,
    pub autoplay_requester: &'static str,
    pub playlist_saved: &'static str,       // has {} {}
    pub playlist_loaded: &'static str,      // has {} {}
    pub playlist_deleted: &'static str,     // has {}
    pub playlist_not_found: &'static str,   // has {}
    pub playlist_empty_queue: &'static str,
    pub playlist_list_title: &'static str,
    pub playlist_list_empty: &'static str,
    pub playlist_show_title: &'static str,  // has {}
    pub history_title: &'static str,
    pub history_empty: &'static str,
    pub history_footer: &'static str,       // has {}
    pub history_cleared: &'static str,
    pub cmd_recommend: &'static str,
    pub recommend_title: &'static str,
    pub recommend_taste_header: &'static str,
    pub recommend_songs_header: &'static str,
    pub recommend_play_all: &'static str,
    pub recommend_select_placeholder: &'static str,
    pub recommend_empty: &'static str,
    pub recommend_enqueued_all: &'static str, // has {}
    pub recommend_enqueued_one: &'static str, // has {}

    // === Error messages ===
    pub nothing_playing: &'static str,
    pub not_connected: &'static str,
    pub not_in_voice: &'static str,
    pub nothing_to_skip: &'static str,
    pub nothing_playing_now: &'static str,
    pub no_upcoming_to_clear: &'static str,
    pub track0_already_playing: &'static str,
    pub invalid_position: &'static str,     // has #{}
    pub no_track_at_position: &'static str, // has #{}
    pub position_must_be_1: &'static str,
    pub failed_leave_voice: &'static str, // has {:?}
    pub unknown_command: &'static str,
    pub server_only: &'static str,
    pub invalid_query: &'static str,
    pub provide_query: &'static str,
    pub failed_connect_voice: &'static str, // has {:?}
    pub could_not_find: &'static str,       // has {}
    pub could_not_extract: &'static str,    // has {}
    pub selection_expired: &'static str,
    pub not_connected_vc: &'static str,
    pub invalid_time_format: &'static str,
    pub seek_exceeds_duration: &'static str, // has {}
    pub lyrics_not_found: &'static str,      // has {}
    pub lyrics_no_track: &'static str,

    // === Embed: Now Playing ===
    pub now_playing_title: &'static str,
    pub field_duration: &'static str,
    pub field_artist: &'static str,
    pub field_requested_by: &'static str,
    pub field_queue: &'static str,
    pub queue_upcoming_count: &'static str, // has {}
    pub field_loop: &'static str,
    pub unknown_artist: &'static str,
    pub unknown: &'static str,

    // === Embed: Play search ===
    pub search_title: &'static str, // has {}
    pub field_first_track: &'static str,
    pub field_queue_total: &'static str,
    pub field_requested_by_play: &'static str,
    pub tracks_count: &'static str, // has {}
    pub search_placeholder: &'static str,

    // === Queue view ===
    pub queue_empty_title: &'static str,
    pub queue_empty_desc: &'static str,
    pub queue_title: &'static str,
    pub now_playing_label: &'static str,
    pub up_next_label: &'static str,
    pub queue_page_header: &'static str, // has {}/{}
    pub currently_playing: &'static str, // has {}
    pub playing_indicator: &'static str,
    pub more_tracks: &'static str, // has {}
    pub field_total_tracks: &'static str,
    pub field_total_duration: &'static str,
    pub field_repeat_mode: &'static str,
    pub field_random_mode: &'static str,
    pub shuffle_on: &'static str,
    pub shuffle_off: &'static str,
    pub queue_footer_shuffle_active: &'static str,
    pub queue_footer_shuffle_inactive: &'static str,
    pub queue_footer: &'static str, // has {}/{} {} {} {} {}
    pub queue_empty_msg: &'static str,
    pub queue_finished_playing: &'static str,
    pub btn_prev: &'static str,
    pub btn_next: &'static str,
    pub btn_random_on: &'static str,
    pub btn_random_off: &'static str,
    pub btn_skip_queue: &'static str,
    pub queue_select_placeholder: &'static str,

    // === Help guide ===
    pub help_title: &'static str,
    pub help_description: &'static str,
    pub help_play: &'static str,
    pub help_pause_resume: &'static str,
    pub help_skip_replay: &'static str,
    pub help_shuffle: &'static str,
    pub help_repeat: &'static str,
    pub help_queue_nowplaying: &'static str,
    pub help_remove_clear: &'static str,
    pub help_jump: &'static str,
    pub help_volume: &'static str,
    pub help_playnext: &'static str,
    pub help_stop_leave: &'static str,
    pub help_ping: &'static str,
    pub help_seek: &'static str,
    pub help_lyrics: &'static str,
    pub help_filter: &'static str,
    pub help_autoplay: &'static str,
    pub help_playlist: &'static str,
    pub help_history: &'static str,

    // === Ping ===
    pub ping_title: &'static str,
    pub ping_gateway_status: &'static str,
    pub ping_gateway_value: &'static str,
    pub ping_audio_engine: &'static str,
    pub ping_audio_value: &'static str,
    pub ping_configuration: &'static str,
    pub ping_config_llm: &'static str,
    pub ping_config_playlists: &'static str,
    pub ping_config_spotify: &'static str,

    // === Replay ===
    pub replaying: &'static str, // has {} {}

    // === Playlist ===
    pub playlist_enqueued: &'static str, // has {}
    pub playlist_added: &'static str,    // has {}
    pub platform_label: &'static str,    // has {}

    // === Search ===
    pub search_results_desc: &'static str, // has {} {}

    // === Queue view extras ===
    pub queue_author_label: &'static str, // has {}
    pub queue_track_jump_desc: &'static str, // has #{}

    // === Button labels (control buttons) ===
    pub btn_resume: &'static str,
    pub btn_pause: &'static str,
    pub btn_skip: &'static str,
    pub btn_loop: &'static str,
    pub btn_stop: &'static str,

    // === Music control hint ===
    pub music_control_hint: &'static str,

    // === LoopMode ===
    pub loop_off: &'static str,
    pub loop_track: &'static str,
    pub loop_queue: &'static str,
}

static EN: LazyLock<Lang> = LazyLock::new(|| Lang {
    cmd_play: "Play audio from YouTube, Spotify, SoundCloud, or search query",
    cmd_pause: "Pause the currently playing track",
    cmd_resume: "Resume the paused track",
    cmd_skip: "Skip the current track and play next",
    cmd_replay: "Replay the current track from the beginning",
    cmd_stop: "Stop playback and clear the queue",
    cmd_queue: "View the current music queue",
    cmd_nowplaying: "Show details of the currently playing track",
    cmd_clear: "Clear all upcoming tracks from the queue",
    cmd_remove: "Remove a specific track from the queue by position",
    cmd_jump: "Jump to a specific track in the queue",
    cmd_shuffle: "Toggle shuffle mode for the queue",
    cmd_repeat: "Set repeat mode (off, track, queue)",
    cmd_volume: "Set playback volume (0 - 100)",
    cmd_playnext: "Add a song to play next (priority queue)",
    cmd_leave: "Disconnect the bot from the voice channel",
    cmd_ping: "Check bot latency and audio pipeline status",
    cmd_help: "Show available music commands",
    cmd_seek: "Seek to a specific time in the current track (e.g. 1:30 or 90)",
    cmd_lyrics: "Display lyrics for the current song or a search query",
    cmd_filter: "Apply an audio filter (bassboost, nightcore, vaporwave, 8d, karaoke, off)",
    cmd_autoplay: "Toggle automatic music recommendations when the queue ends",
    cmd_playlist: "Manage personal saved music playlists (save, load, list, show, delete)",
    cmd_history: "View or clear the playback history log used for Autoplay (/history [clear])",
    cmd_recommend: "Get AI music recommendations based on your server taste (40% YT, 30% Spotify, 30% SoundCloud)",

    playback_paused: "⏸️ Playback paused.",
    playback_resumed: "▶️ Playback resumed.",
    skipped_current: "⏭️ Skipped current track.",
    stopped_and_cleared: "⏹️ Playback stopped and queue cleared.",
    volume_set: "🔊 Volume set to **{}%**",
    disconnected: "👋 Disconnected from voice channel.",
    shuffle_enabled: "🔀 Shuffle mode **enabled**! Upcoming tracks have been randomized.",
    shuffle_disabled: "➡️ Shuffle mode **disabled**.",
    cleared_n_tracks: "🗑️ Cleared **{}** upcoming track(s) from the queue.",
    removed_track: "🗑️ Removed **#{}** [**{}**]({}) from the queue.",
    jumped_to: "⏭️ Jumped to **#{}**: [**{}**]({})",
    repeat_mode_set: "{} Repeat mode set to **{}**",
    seek_success: "⏩ Seeked to **{}**",
    lyrics_title: "📝 Lyrics: {} - {}",
    lyrics_footer: "Powered by LRCLIB • Requested by {}",
    filter_set: "🎛️ Audio filter set to **{}**",
    filter_disabled: "➡️ Audio filter **disabled**.",
    autoplay_enabled: "📻 Autoplay **enabled**! Bot will keep playing related songs when the queue ends.",
    autoplay_disabled: "➡️ Autoplay **disabled**.",
    autoplay_requester: "Autoplay 📻",
    playlist_saved: "💾 Saved **{}** tracks to playlist **{}**!",
    playlist_loaded: "📂 Loaded **{}** tracks from playlist **{}** into the queue!",
    playlist_deleted: "🗑️ Playlist **{}** has been deleted.",
    playlist_not_found: "⚠️ Playlist **{}** was not found.",
    playlist_empty_queue: "⚠️ The current queue is empty. Nothing to save!",
    playlist_list_title: "📋 Your Saved Playlists",
    playlist_list_empty: "ℹ️ You don't have any saved playlists yet. Use `/playlist save <name>` to save your current queue!",
    playlist_show_title: "📋 Playlist: {}",
    history_title: "📜 Playback History Log",
    history_empty: "ℹ️ No tracks in playback history log yet.",
    history_footer: "Total: {} unique song(s) • Autoplay Reference Active",
    history_cleared: "🗑️ Playback history log has been cleared.",
    recommend_title: "✨ AI Music Recommendations",
    recommend_taste_header: "📊 Server Taste Profile",
    recommend_songs_header: "🎵 Recommended Songs for You",
    recommend_play_all: "▶️ Enqueue All",
    recommend_select_placeholder: "Choose a song to play immediately...",
    recommend_empty: "⚠️ Could not generate recommendations. Try playing some more songs first!",
    recommend_enqueued_all: "✅ Enqueued **{}** recommended tracks into the queue!",
    recommend_enqueued_one: "✅ Enqueued **{}** from recommendations!",

    nothing_playing: "⚠️ Nothing is currently playing.",
    not_connected: "⚠️ Bot is not connected to a voice channel.",
    not_in_voice: "⚠️ Bot is not in a voice channel.",
    nothing_to_skip: "⚠️ Nothing is playing to skip.",
    nothing_playing_now: "⚠️ Nothing is playing right now.",
    no_upcoming_to_clear: "⚠️ The queue has no upcoming tracks to clear.",
    track0_already_playing: "⚠️ Track #0 is already playing. Use `/replay` to restart it.",
    invalid_position: "⚠️ Invalid track position **#{}**.",
    no_track_at_position: "⚠️ No track found at position **#{}**.",
    position_must_be_1: "⚠️ Position must be 1 or greater. Use `/skip` to skip the currently playing song (#0).",
    failed_leave_voice: "❌ Failed to leave voice: {:?}",
    unknown_command: "⚠️ Unknown command.",
    server_only: "❌ This command can only be used in a server.",
    invalid_query: "❌ Invalid query parameter.",
    provide_query: "❌ Please provide a song query or URL.",
    failed_connect_voice: "❌ Failed to connect to voice channel: {:?}",
    could_not_find: "❌ Could not find: {}",
    could_not_extract: "❌ Could not find or extract audio: {}",
    selection_expired: "❌ Selection expired or invalid. Try `/play` again.",
    not_connected_vc: "❌ Not connected to a voice channel.",
    invalid_time_format: "❌ Invalid time format. Use `mm:ss` (e.g. `1:30`) or seconds (e.g. `90`).",
    seek_exceeds_duration: "⚠️ Cannot seek beyond track duration (total: **{}**).",
    lyrics_not_found: "❌ No lyrics found for **{}**.",
    lyrics_no_track: "⚠️ Nothing is currently playing. Please specify a song name: `/lyrics <song>`",

    now_playing_title: "🎵 Now Playing",
    field_duration: "⏱️ Duration",
    field_artist: "🎶 Artist",
    field_requested_by: "👤 Requested by",
    field_queue: "📌 Queue",
    queue_upcoming_count: "{} songs upcoming",
    field_loop: "🔁 Loop",
    unknown_artist: "Unknown Artist",
    unknown: "Unknown",

    search_title: "🔍 Search: {}",
    field_first_track: "📌 First Track",
    field_queue_total: "📊 Queue Total",
    field_requested_by_play: "🙋 Requested By",
    tracks_count: "{} tracks",
    search_placeholder: "Select a track to play",

    queue_empty_title: "📭 Queue is empty",
    queue_empty_desc: "No tracks in the queue.",
    queue_title: "📋 Music Queue",
    now_playing_label: "**▶️ Now Playing:**\n",
    up_next_label: "**Up Next:**\n",
    queue_page_header: "**📋 Queue (Page {}/{}):**\n",
    currently_playing: "▶️ Currently Playing ({})",
    playing_indicator: "▶️ #0 (Playing)",
    more_tracks: "\n*...and {} more tracks in queue*",
    field_total_tracks: "📊 Total Tracks",
    field_total_duration: "⏱️ Total Duration",
    field_repeat_mode: "🔁 Repeat Mode",
    field_random_mode: "🔀 Random Mode",
    shuffle_on: "🔀 Shuffled (On)",
    shuffle_off: "➡️ Sequential (Off)",
    queue_footer_shuffle_active: "Active",
    queue_footer_shuffle_inactive: "Inactive",
    queue_footer: "Page {}/{} | Platform: {} | Loop: {} | Random: {}",
    queue_empty_msg: "📭 The queue is currently empty.",
    queue_finished_playing: "📭 Queue finished playing.",
    btn_prev: "◀️ Previous",
    btn_next: "Next ▶️",
    btn_random_on: "🔀 Random: ON",
    btn_random_off: "🔀 Random: OFF",
    btn_skip_queue: "⏭️ Skip",
    queue_select_placeholder: "🎵 Choose a song from the list to jump & play directly...",

    help_title: "📖 Discord Music Bot - Help Guide",
    help_description: "A lightweight, high-performance music bot built with Rust & Songbird.",
    help_play: "Play from YouTube / Spotify / SoundCloud / search query",
    help_pause_resume: "Pause or resume playback",
    help_skip_replay: "Next track or replay from beginning",
    help_shuffle: "Toggle shuffle mode",
    help_repeat: "Repeat mode: `off`, `track`, or `queue`",
    help_queue_nowplaying: "View interactive queue or current track info",
    help_remove_clear: "Remove specific track or clear queue",
    help_jump: "Jump to a track in the queue",
    help_volume: "Set volume level",
    help_playnext: "Add a song to play next (priority)",
    help_stop_leave: "Stop music or disconnect bot from voice",
    help_ping: "Check bot latency and audio engine status",
    help_seek: "Seek to a specific timestamp in the current track",
    help_lyrics: "Show lyrics for the current song or specified title",
    help_filter: "Apply audio filters (bassboost, nightcore, etc.)",
    help_autoplay: "Toggle autoplay (automatic related recommendations)",
    help_playlist: "Save, load, list, show, or delete personal playlists",
    help_history: "Show server playback history log (used as Autoplay reference)",

    ping_title: "🏓 Pong!",
    ping_gateway_status: "⚡ Bot Gateway Status",
    ping_gateway_value: "🟢 Connected & Operational",
    ping_audio_engine: "📻 Audio Engine",
    ping_audio_value: "Songbird 48kHz Stereo Opus (96kbps)",
    ping_configuration: "🔧 Configuration",
    ping_config_llm: "🤖 AI DJ",
    ping_config_playlists: "💾 Playlists",
    ping_config_spotify: "🎵 Spotify",

    replaying: "🔄 Replaying: [**{}**]({})",

    playlist_enqueued: "{} Playlist Enqueued",
    playlist_added: "Added {} tracks to queue",
    platform_label: "Platform: {}",

    search_results_desc: "Found {} results — select below or click a link:\n\n{}",

    queue_author_label: "Current Music Queue ({})",
    queue_track_jump_desc: "Jump to track #{} ({})",

    btn_resume: "Resume",
    btn_pause: "Pause",
    btn_skip: "Skip",
    btn_loop: "Loop",
    btn_stop: "Stop",

    music_control_hint: "🎶 Use the buttons below to control music playback",

    loop_off: "Off",
    loop_track: "1 Track",
    loop_queue: "All Queue",
});

static ID: LazyLock<Lang> = LazyLock::new(|| Lang {
    cmd_play: "Putar audio dari YouTube, Spotify, SoundCloud, atau kata kunci",
    cmd_pause: "Jeda lagu yang sedang diputar",
    cmd_resume: "Lanjutkan pemutaran lagu yang dijeda",
    cmd_skip: "Lewati lagu saat ini dan putar berikutnya",
    cmd_replay: "Ulangi lagu saat ini dari awal",
    cmd_stop: "Hentikan pemutaran dan bersihkan antrean",
    cmd_queue: "Lihat antrean musik saat ini",
    cmd_nowplaying: "Tampilkan detail lagu yang sedang diputar",
    cmd_clear: "Bersihkan semua lagu berikutnya dari antrean",
    cmd_remove: "Hapus lagu tertentu dari antrean berdasarkan posisi",
    cmd_jump: "Langsung lompat ke lagu di antrean",
    cmd_shuffle: "Aktifkan atau matikan mode acak untuk antrean",
    cmd_repeat: "Atur mode ulang (off, track, queue)",
    cmd_volume: "Atur volume pemutaran (0 - 100)",
    cmd_playnext: "Tambah lagu untuk diputar berikutnya (antrean prioritas)",
    cmd_leave: "Putuskan bot dari voice channel",
    cmd_ping: "Cek latensi bot dan status pipeline audio",
    cmd_help: "Tampilkan perintah musik yang tersedia",
    cmd_seek: "Lompat ke menit/detik tertentu pada lagu saat ini (contoh: 1:30 atau 90)",
    cmd_lyrics: "Tampilkan lirik lagu saat ini atau cari berdasarkan judul",
    cmd_filter: "Terapkan efek suara (bassboost, nightcore, vaporwave, 8d, karaoke, off)",
    cmd_autoplay: "Aktifkan/nonaktifkan rekomendasi musik otomatis saat antrean habis",
    cmd_playlist: "Kelola playlist musik pribadi (save, load, list, show, delete)",
    cmd_history: "Lihat atau bersihkan log riwayat lagu yang diputar (/history [clear])",
    cmd_recommend: "Rekomendasi lagu berdasarkan selera server (40% YT, 30% Spotify, 30% SoundCloud)",

    playback_paused: "⏸️ Pemutaran dijeda.",
    playback_resumed: "▶️ Pemutaran dilanjutkan.",
    skipped_current: "⏭️ Lagu saat ini dilewati.",
    stopped_and_cleared: "⏹️ Pemutaran dihentikan dan antrean dibersihkan.",
    volume_set: "🔊 Volume diatur ke **{}%**",
    disconnected: "👋 Terputus dari voice channel.",
    shuffle_enabled: "🔀 Mode acak **diaktifkan**! Lagu berikutnya telah diacak.",
    shuffle_disabled: "➡️ Mode acak **dinonaktifkan**.",
    cleared_n_tracks: "🗑️ Dihapus **{}** lagu berikutnya dari antrean.",
    removed_track: "🗑️ Dihapus **#{}** [**{}**]({}) dari antrean.",
    jumped_to: "⏭️ Melompat ke **#{}**: [**{}**]({})",
    repeat_mode_set: "{} Mode ulang diatur ke **{}**",
    seek_success: "⏩ Berhasil melompat ke **{}**",
    lyrics_title: "📝 Lirik: {} - {}",
    lyrics_footer: "Didukung oleh LRCLIB • Diminta oleh {}",
    filter_set: "🎛️ Filter audio diatur ke **{}**",
    filter_disabled: "➡️ Filter audio **dinonaktifkan**.",
    autoplay_enabled: "📻 Autoplay **diaktifkan**! Bot akan terus memutar lagu rekomendasi saat antrean habis.",
    autoplay_disabled: "➡️ Autoplay **dinonaktifkan**.",
    autoplay_requester: "Autoplay 📻",
    playlist_saved: "💾 Berhasil menyimpan **{}** lagu ke playlist **{}**!",
    playlist_loaded: "📂 Berhasil memuat **{}** lagu dari playlist **{}** ke dalam antrean!",
    playlist_deleted: "🗑️ Playlist **{}** berhasil dihapus.",
    playlist_not_found: "⚠️ Playlist **{}** tidak ditemukan.",
    playlist_empty_queue: "⚠️ Antrean musik saat ini kosong. Tidak ada lagu untuk disimpan!",
    playlist_list_title: "📋 Daftar Playlist Tersimpan Kamu",
    playlist_list_empty: "ℹ️ Kamu belum memiliki playlist tersimpan. Gunakan `/playlist save <nama>` untuk menyimpan antrean saat ini!",
    playlist_show_title: "📋 Playlist: {}",
    history_title: "📜 Log Riwayat Pemutaran Lagu",
    history_empty: "ℹ️ Belum ada riwayat lagu yang diputar.",
    history_footer: "Total: {} lagu unik • Acuan Autoplay Aktif",
    history_cleared: "🗑️ Log riwayat pemutaran musik berhasil dibersihkan.",
    recommend_title: "✨ Rekomendasi Musik Spesial",
    recommend_taste_header: "📊 Profil Selera Musik Server",
    recommend_songs_header: "🎵 Lagu Rekomendasi untuk Kamu",
    recommend_play_all: "▶️ Putar Semua ke Antrean",
    recommend_select_placeholder: "Pilih lagu untuk langsung diputar...",
    recommend_empty: "⚠️ Belum bisa meracik rekomendasi. Coba putar beberapa lagu terlebih dahulu!",
    recommend_enqueued_all: "✅ Berhasil memasukkan **{}** lagu rekomendasi ke dalam antrean!",
    recommend_enqueued_one: "✅ Berhasil memutar **{}** dari rekomendasi!",

    nothing_playing: "⚠️ Tidak ada yang sedang diputar.",
    not_connected: "⚠️ Bot tidak terhubung ke voice channel.",
    not_in_voice: "⚠️ Bot tidak berada di voice channel.",
    nothing_to_skip: "⚠️ Tidak ada yang sedang diputar untuk dilewati.",
    nothing_playing_now: "⚠️ Tidak ada yang sedang diputar saat ini.",
    no_upcoming_to_clear: "⚠️ Antrean tidak ada lagu berikutnya untuk dibersihkan.",
    track0_already_playing: "⚠️ Lagu #0 sedang diputar. Gunakan `/replay` untuk mengulanginya.",
    invalid_position: "⚠️ Posisi lagu tidak valid **#{}**.",
    no_track_at_position: "⚠️ Lagu tidak ditemukan di posisi **#{}**.",
    position_must_be_1: "⚠️ Posisi harus 1 atau lebih. Gunakan `/skip` untuk melewati lagu yang sedang diputar (#0).",
    failed_leave_voice: "❌ Gagal keluar dari voice: {:?}",
    unknown_command: "⚠️ Perintah tidak dikenal.",
    server_only: "❌ Perintah ini hanya bisa digunakan di server.",
    invalid_query: "❌ Parameter query tidak valid.",
    provide_query: "❌ Harap berikan query lagu atau URL.",
    failed_connect_voice: "❌ Gagal terhubung ke voice channel: {:?}",
    could_not_find: "❌ Tidak ditemukan: {}",
    could_not_extract: "❌ Tidak dapat menemukan atau mengekstrak audio: {}",
    selection_expired: "❌ Pilihan sudah kedaluwarsa atau tidak valid. Coba `/play` lagi.",
    not_connected_vc: "❌ Tidak terhubung ke voice channel.",
    invalid_time_format: "❌ Format waktu salah. Gunakan `mm:ss` (contoh: `1:30`) atau detik (contoh: `90`).",
    seek_exceeds_duration: "⚠️ Tidak bisa melompat melebihi durasi lagu (total: **{}**).",
    lyrics_not_found: "❌ Lirik tidak ditemukan untuk **{}**.",
    lyrics_no_track: "⚠️ Tidak ada lagu yang sedang diputar. Harap sebutkan judul lagu: `/lyrics <judul>`",

    now_playing_title: "🎵 Sekarang Diputar",
    field_duration: "⏱️ Durasi",
    field_artist: "🎶 Artis",
    field_requested_by: "👤 Diminta oleh",
    field_queue: "📌 Antrian",
    queue_upcoming_count: "{} lagu berikutnya",
    field_loop: "🔁 Loop",
    unknown_artist: "Artis Tidak Diketahui",
    unknown: "Tidak Diketahui",

    search_title: "🔍 Pencarian: {}",
    field_first_track: "📌 Lagu Pertama",
    field_queue_total: "📊 Total Antrean",
    field_requested_by_play: "🙋 Diminta Oleh",
    tracks_count: "{} lagu",
    search_placeholder: "Pilih lagu untuk diputar",

    queue_empty_title: "📭 Antrean kosong",
    queue_empty_desc: "Tidak ada lagu dalam antrean.",
    queue_title: "📋 Antrean Musik",
    now_playing_label: "**▶️ Sedang Diputar:**\n",
    up_next_label: "**Berikutnya:**\n",
    queue_page_header: "**📋 Antrean (Halaman {}/{}):**\n",
    currently_playing: "▶️ Sedang Diputar ({})",
    playing_indicator: "▶️ #0 (Diputar)",
    more_tracks: "\n*...dan {} lagu lagi dalam antrean*",
    field_total_tracks: "📊 Total Lagu",
    field_total_duration: "⏱️ Total Durasi",
    field_repeat_mode: "🔁 Mode Ulang",
    field_random_mode: "🔀 Mode Acak",
    shuffle_on: "🔀 Acak (On)",
    shuffle_off: "➡️ Berurutan (Off)",
    queue_footer_shuffle_active: "Aktif",
    queue_footer_shuffle_inactive: "Nonaktif",
    queue_footer: "Halaman {}/{} | Platform: {} | Ulang: {} | Acak: {}",
    queue_empty_msg: "📭 Antrean saat ini kosong.",
    queue_finished_playing: "📭 Antrean telah selesai diputar.",
    btn_prev: "◀️ Sebelumnya",
    btn_next: "Berikutnya ▶️",
    btn_random_on: "🔀 Acak: ON",
    btn_random_off: "🔀 Acak: OFF",
    btn_skip_queue: "⏭️ Lewati",
    queue_select_placeholder: "🎵 Pilih lagu dari daftar untuk lompat & putar langsung...",

    help_title: "📖 Discord Music Bot - Panduan Bantuan",
    help_description: "Bot musik ringan berperforma tinggi berbasis Rust & Songbird.",
    help_play: "Putar dari YouTube / Spotify / SoundCloud / kata kunci",
    help_pause_resume: "Jeda atau lanjutkan pemutaran",
    help_skip_replay: "Lagu berikutnya atau ulangi dari awal",
    help_shuffle: "Aktifkan atau matikan mode acak",
    help_repeat: "Mode ulang: `off`, `track`, atau `queue`",
    help_queue_nowplaying: "Lihat antrean interaktif atau info lagu saat ini",
    help_remove_clear: "Hapus lagu tertentu atau bersihkan antrean",
    help_jump: "Langsung lompat ke lagu di antrean",
    help_volume: "Atur level volume",
    help_playnext: "Tambah lagu untuk diputar berikutnya (prioritas)",
    help_stop_leave: "Hentikan musik atau putuskan bot dari voice",
    help_ping: "Cek latensi bot dan status mesin audio",
    help_seek: "Lompat ke menit/detik tertentu pada lagu yang sedang diputar",
    help_lyrics: "Tampilkan lirik untuk lagu saat ini atau lagu yang dicari",
    help_filter: "Terapkan efek suara (bassboost, nightcore, dll.)",
    help_autoplay: "Atur mode autoplay (rekomendasi lagu otomatis)",
    help_playlist: "Simpan, muat, lihat, atau hapus playlist pribadi",
    help_history: "Lihat log riwayat lagu yang diputar (acuan Autoplay)",

    ping_title: "🏓 Pong!",
    ping_gateway_status: "⚡ Status Gateway Bot",
    ping_gateway_value: "🟢 Terhubung & Beroperasi",
    ping_audio_engine: "📻 Mesin Audio",
    ping_audio_value: "Songbird 48kHz Stereo Opus (96kbps)",
    ping_configuration: "🔧 Konfigurasi",
    ping_config_llm: "🤖 AI DJ",
    ping_config_playlists: "💾 Playlist",
    ping_config_spotify: "🎵 Spotify",

    replaying: "🔄 Replaying: [**{}**]({})",

    playlist_enqueued: "{} Playlist Enqueued",
    playlist_added: "Added {} tracks to queue",
    platform_label: "Platform: {}",

    search_results_desc: "Found {} results — select below or click a link:\n\n{}",

    queue_author_label: "Current Music Queue ({})",
    queue_track_jump_desc: "Jump to track #{} ({})",

    btn_resume: "Resume",
    btn_pause: "Pause",
    btn_skip: "Skip",
    btn_loop: "Loop",
    btn_stop: "Stop",

    music_control_hint: "🎶 Gunakan tombol di bawah untuk kontrol musik",

    loop_off: "Nonaktif",
    loop_track: "1 Lagu",
    loop_queue: "Semua Antrean",
});

pub static ACTIVE_LANG: LazyLock<&'static Lang> = LazyLock::new(|| {
    match std::env::var("BOT_LANG")
        .unwrap_or_else(|_| "en".to_string())
        .as_str()
    {
        "id" => &ID,
        _ => &EN,
    }
});

pub fn get_lang() -> &'static Lang {
    &ACTIVE_LANG
}

pub fn is_id() -> bool {
    std::env::var("BOT_LANG")
        .map(|s| s.eq_ignore_ascii_case("id"))
        .unwrap_or(false)
}

/// Format a lang string with positional {} placeholders.
/// Usage: fmt(get_lang().volume_set, &[&volume_level])
pub fn fmt(s: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = s.to_string();
    for arg in args {
        result = result.replacen("{}", &arg.to_string(), 1);
    }
    result
}
