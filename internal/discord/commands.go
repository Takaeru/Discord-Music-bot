package discord

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"strings"
	"time"

	"github.com/bwmarrin/discordgo"
)

var (
	minVolume = 0.0
	maxVolume = 100.0
)

// Commands contains the definition of all slash commands supported by the bot.
var Commands = []*discordgo.ApplicationCommand{
	{
		Name:        "join",
		Description: "Join your current voice channel",
	},
	{
		Name:        "leave",
		Description: "Leave the voice channel and clear the queue",
	},
	{
		Name:        "play",
		Description: "Play a song from YouTube URL or search query",
		Options: []*discordgo.ApplicationCommandOption{
			{
				Type:        discordgo.ApplicationCommandOptionString,
				Name:        "query",
				Description: "YouTube URL or search keywords",
				Required:    true,
			},
		},
	},
	{
		Name:        "pause",
		Description: "Pause the currently playing track",
	},
	{
		Name:        "resume",
		Description: "Resume the paused track",
	},
	{
		Name:        "skip",
		Description: "Skip the current track and play the next in queue",
	},
	{
		Name:        "stop",
		Description: "Stop playback and clear the queue",
	},
	{
		Name:        "queue",
		Description: "Display the current track queue",
	},
	{
		Name:        "nowplaying",
		Description: "Display details about the currently playing track",
	},
	{
		Name:        "volume",
		Description: "Set playback volume (0-100)",
		Options: []*discordgo.ApplicationCommandOption{
			{
				Type:        discordgo.ApplicationCommandOptionInteger,
				Name:        "volume",
				Description: "Volume percentage (0-100)",
				Required:    true,
				MinValue:    &minVolume,
				MaxValue:    maxVolume,
			},
		},
	},
}

// respondText sends an immediate standard text response to an interaction.
func respondText(s *discordgo.Session, i *discordgo.InteractionCreate, content string) {
	err := s.InteractionRespond(i.Interaction, &discordgo.InteractionResponse{
		Type: discordgo.InteractionResponseChannelMessageWithSource,
		Data: &discordgo.InteractionResponseData{
			Content: content,
		},
	})
	if err != nil {
		slog.Error("Failed to send interaction response",
			"command", i.ApplicationCommandData().Name,
			"error", err,
		)
	}
}

// deferResponse acknowledges the interaction to prevent Discord timeout.
func deferResponse(s *discordgo.Session, i *discordgo.InteractionCreate) {
	_ = s.InteractionRespond(i.Interaction, &discordgo.InteractionResponse{
		Type: discordgo.InteractionResponseDeferredChannelMessageWithSource,
	})
}

// editResponse edits the deferred interaction response with final content.
func editResponse(s *discordgo.Session, i *discordgo.InteractionCreate, content string) {
	_, err := s.InteractionResponseEdit(i.Interaction, &discordgo.WebhookEdit{
		Content: &content,
	})
	if err != nil {
		slog.Error("Failed to edit interaction response",
			"command", i.ApplicationCommandData().Name,
			"error", err,
		)
	}
}

// formatDuration formats time.Duration into MM:SS format.
func formatDuration(d time.Duration) string {
	d = d.Round(time.Second)
	m := d / time.Minute
	s := (d % time.Minute) / time.Second
	return fmt.Sprintf("%02d:%02d", m, s)
}

// respondEmbed sends an embed response to an interaction.
func respondEmbed(s *discordgo.Session, i *discordgo.InteractionCreate, embed *discordgo.MessageEmbed) {
	err := s.InteractionRespond(i.Interaction, &discordgo.InteractionResponse{
		Type: discordgo.InteractionResponseChannelMessageWithSource,
		Data: &discordgo.InteractionResponseData{
			Embeds: []*discordgo.MessageEmbed{embed},
		},
	})
	if err != nil {
		slog.Error("Failed to send interaction embed response",
			"command", i.ApplicationCommandData().Name,
			"error", err,
		)
	}
}

func extractUser(i *discordgo.InteractionCreate) (userID, userName string) {
	if i.Member != nil && i.Member.User != nil {
		return i.Member.User.ID, i.Member.User.Username
	}
	if i.User != nil {
		return i.User.ID, i.User.Username
	}
	return "", ""
}

// defaultCommandHandlers returns the map of command handlers.
func (b *Bot) defaultCommandHandlers() map[string]func(s *discordgo.Session, i *discordgo.InteractionCreate) {
	return map[string]func(s *discordgo.Session, i *discordgo.InteractionCreate){
		"join": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			// Acknowledge interaction immediately to prevent Discord 3-second timeout
			deferResponse(s, i)

			channelID, err := FindUserVoiceChannel(s, i.GuildID, userID)
			if err != nil {
				b.logger.Info("Join command rejected: user not in voice", "guild_id", i.GuildID, "user_id", userID, "command", "join")
				editResponse(s, i, "❌ You must be in a voice channel to use this command.")
				return
			}

			err = b.JoinVoiceChannel(i.GuildID, channelID)
			if err != nil {
				b.logger.Warn("Failed to join voice channel", "guild_id", i.GuildID, "user_id", userID, "command", "join", "error", err)
				if errors.Is(err, ErrMissingConnectPermission) {
					editResponse(s, i, "❌ I don't have permission to **Connect** to that voice channel.")
					return
				}
				if errors.Is(err, ErrMissingSpeakPermission) {
					editResponse(s, i, "❌ I don't have permission to **Speak** in that voice channel.")
					return
				}
				editResponse(s, i, "❌ Failed to connect to the voice channel. Please check channel permissions.")
				return
			}

			b.logger.Info("Joined voice channel", "guild_id", i.GuildID, "user_id", userID, "channel_id", channelID, "command", "join")
			editResponse(s, i, fmt.Sprintf("🔊 Joined <#%s>!", channelID))
		},
		"leave": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			err := b.LeaveVoiceChannel(i.GuildID)
			if err != nil {
				if errors.Is(err, ErrBotNotInVoice) {
					respondText(s, i, "❌ I am not connected to a voice channel in this server.")
					return
				}
				b.logger.Error("Failed to leave voice channel", "guild_id", i.GuildID, "user_id", userID, "command", "leave", "error", err)
				respondText(s, i, "❌ An error occurred while disconnecting.")
				return
			}

			b.logger.Info("Left voice channel and reset state", "guild_id", i.GuildID, "user_id", userID, "command", "leave")
			respondText(s, i, "👋 Left the voice channel and cleared the queue.")
		},
		"play": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, userName := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			options := i.ApplicationCommandData().Options
			query := ""
			if len(options) > 0 {
				query = options[0].StringValue()
			}
			if strings.TrimSpace(query) == "" {
				respondText(s, i, "❌ Please provide a song title or YouTube URL.")
				return
			}

			// Acknowledge interaction immediately
			deferResponse(s, i)

			channelID, err := FindUserVoiceChannel(s, i.GuildID, userID)
			if err != nil {
				b.logger.Info("Play command rejected: user not in voice", "guild_id", i.GuildID, "user_id", userID, "command", "play")
				editResponse(s, i, "❌ You must be in a voice channel to use `/play`.")
				return
			}

			err = b.JoinVoiceChannel(i.GuildID, channelID)
			if err != nil {
				b.logger.Warn("Failed to join voice on play", "guild_id", i.GuildID, "user_id", userID, "command", "play", "error", err)
				if errors.Is(err, ErrMissingConnectPermission) {
					editResponse(s, i, "❌ I don't have permission to **Connect** to that voice channel.")
					return
				}
				if errors.Is(err, ErrMissingSpeakPermission) {
					editResponse(s, i, "❌ I don't have permission to **Speak** in that voice channel.")
					return
				}
				editResponse(s, i, "❌ Failed to connect to the voice channel.")
				return
			}

			res, err := b.audioSource.Resolve(context.Background(), query, userName)
			if err != nil || res == nil || len(res.Tracks) == 0 {
				b.logger.Warn("Failed to resolve track", "guild_id", i.GuildID, "user_id", userID, "command", "play", "query", query, "error", err)
				editResponse(s, i, "❌ Could not find or extract audio for the requested track. Please verify your query or URL.")
				return
			}

			player, err := b.musicMgr.GetOrCreate(i.GuildID)
			if err != nil {
				b.logger.Error("Failed to get guild player", "guild_id", i.GuildID, "user_id", userID, "command", "play", "error", err)
				editResponse(s, i, "❌ Internal error accessing guild player.")
				return
			}

			firstTrack := res.Tracks[0]
			queued, err := player.EnqueueAndPlay(context.Background(), firstTrack)
			if err != nil {
				b.logger.Error("Failed to queue track", "guild_id", i.GuildID, "user_id", userID, "command", "play", "track_id", firstTrack.ID, "error", err)
				editResponse(s, i, "❌ Failed to enqueue track.")
				return
			}

			// If there are additional tracks (e.g. from a playlist or mix), queue them all
			for _, t := range res.Tracks[1:] {
				_ = player.Queue.Add(t)
			}

			b.logger.Info("Track(s) queued/playing", "guild_id", i.GuildID, "user_id", userID, "command", "play", "total_tracks", len(res.Tracks), "playlist", res.PlaylistName)

			if len(res.Tracks) > 1 {
				if queued {
					editResponse(s, i, fmt.Sprintf("📚 Added playlist **%s** (**%d tracks**) to queue!", res.PlaylistName, len(res.Tracks)))
				} else {
					editResponse(s, i, fmt.Sprintf("▶️ Now playing: **[%s](%s)** (`%s`)\n📚 Queued **%d more track(s)** from playlist **%s**", firstTrack.Title, firstTrack.URL, formatDuration(firstTrack.Duration), len(res.Tracks)-1, res.PlaylistName))
				}
			} else {
				if queued {
					pos := player.Queue.Length()
					editResponse(s, i, fmt.Sprintf("📝 Added to queue (**#%d**): **[%s](%s)** (`%s`)", pos, firstTrack.Title, firstTrack.URL, formatDuration(firstTrack.Duration)))
				} else {
					editResponse(s, i, fmt.Sprintf("▶️ Now playing: **[%s](%s)** (`%s`)", firstTrack.Title, firstTrack.URL, formatDuration(firstTrack.Duration)))
				}
			}
		},
		"pause": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.Get(i.GuildID)
			if err != nil {
				respondText(s, i, "❌ Nothing is currently playing in this server.")
				return
			}

			if err := player.Pause(context.Background()); err != nil {
				b.logger.Info("Pause failed", "guild_id", i.GuildID, "user_id", userID, "command", "pause", "error", err)
				respondText(s, i, fmt.Sprintf("❌ %s", err.Error()))
				return
			}

			b.logger.Info("Playback paused", "guild_id", i.GuildID, "user_id", userID, "command", "pause")
			respondText(s, i, "⏸️ Playback paused.")
		},
		"resume": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.Get(i.GuildID)
			if err != nil {
				respondText(s, i, "❌ Nothing is currently playing in this server.")
				return
			}

			if err := player.Resume(context.Background()); err != nil {
				b.logger.Info("Resume failed", "guild_id", i.GuildID, "user_id", userID, "command", "resume", "error", err)
				respondText(s, i, fmt.Sprintf("❌ %s", err.Error()))
				return
			}

			b.logger.Info("Playback resumed", "guild_id", i.GuildID, "user_id", userID, "command", "resume")
			respondText(s, i, "▶️ Playback resumed.")
		},
		"skip": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.Get(i.GuildID)
			if err != nil {
				respondText(s, i, "❌ Nothing is currently playing in this server.")
				return
			}

			skipped, err := player.Skip(context.Background())
			if err != nil {
				b.logger.Info("Skip failed", "guild_id", i.GuildID, "user_id", userID, "command", "skip", "error", err)
				respondText(s, i, "❌ Nothing is currently playing to skip.")
				return
			}

			b.logger.Info("Track skipped", "guild_id", i.GuildID, "user_id", userID, "command", "skip", "track_id", skipped.ID)
			respondText(s, i, fmt.Sprintf("⏭️ Skipped: **%s**.", skipped.Title))
		},
		"stop": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.Get(i.GuildID)
			if err != nil {
				respondText(s, i, "❌ Nothing is currently playing in this server.")
				return
			}

			if err := player.Stop(context.Background()); err != nil {
				b.logger.Info("Stop failed", "guild_id", i.GuildID, "user_id", userID, "command", "stop", "error", err)
				respondText(s, i, "❌ Nothing is currently playing and the queue is already empty.")
				return
			}

			b.logger.Info("Playback stopped and queue cleared", "guild_id", i.GuildID, "user_id", userID, "command", "stop")
			respondText(s, i, "⏹️ Playback stopped and queue cleared.")
		},
		"queue": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.Get(i.GuildID)
			if err != nil || (!player.IsPlaying() && player.Queue.Length() == 0 && player.CurrentTrack() == nil) {
				respondText(s, i, "📜 The queue is currently empty.")
				return
			}

			current := player.CurrentTrack()
			queueItems := player.Queue.List()

			if current == nil && len(queueItems) == 0 {
				respondText(s, i, "📜 The queue is currently empty.")
				return
			}

			var desc strings.Builder
			if current != nil {
				status := "▶️"
				if player.IsPaused() {
					status = "⏸️"
				}
				pos := player.PlaybackPosition()
				desc.WriteString(fmt.Sprintf("**Now Playing:**\n%s [%s](%s) (`%s / %s`) — *Requested by %s*\n\n",
					status, current.Title, current.URL, formatDuration(pos), formatDuration(current.Duration), current.RequestedBy))
			}

			if len(queueItems) > 0 {
				desc.WriteString("**Up Next:**\n")
				limit := 10
				if len(queueItems) < limit {
					limit = len(queueItems)
				}

				for idx := 0; idx < limit; idx++ {
					t := queueItems[idx]
					desc.WriteString(fmt.Sprintf("`#%d.` [%s](%s) (`%s`) — *%s*\n",
						idx+1, t.Title, t.URL, formatDuration(t.Duration), t.RequestedBy))
				}

				if len(queueItems) > limit {
					remaining := len(queueItems) - limit
					desc.WriteString(fmt.Sprintf("\n*... and %d more track(s) in queue*", remaining))
				}
			} else {
				desc.WriteString("*No upcoming tracks in queue.*")
			}

			b.logger.Info("Queue requested", "guild_id", i.GuildID, "user_id", userID, "command", "queue", "queue_len", len(queueItems))

			embed := &discordgo.MessageEmbed{
				Title:       "📜 Music Queue",
				Description: desc.String(),
				Color:       0x5865F2,
				Footer: &discordgo.MessageEmbedFooter{
					Text: fmt.Sprintf("Total tracks in queue: %d", len(queueItems)),
				},
			}

			respondEmbed(s, i, embed)
		},
		"nowplaying": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.Get(i.GuildID)
			if err != nil || (!player.IsPlaying() && player.CurrentTrack() == nil) {
				respondText(s, i, "🎶 Nothing is currently playing in this server.")
				return
			}

			track := player.CurrentTrack()
			if track == nil {
				respondText(s, i, "🎶 Nothing is currently playing in this server.")
				return
			}

			status := "▶️ Playing"
			if player.IsPaused() {
				status = "⏸️ Paused"
			}

			pos := player.PlaybackPosition()
			progressStr := fmt.Sprintf("`%s / %s`", formatDuration(pos), formatDuration(track.Duration))

			b.logger.Info("Nowplaying requested", "guild_id", i.GuildID, "user_id", userID, "command", "nowplaying", "track_id", track.ID)

			embed := &discordgo.MessageEmbed{
				Title:       track.Title,
				URL:         track.URL,
				Description: fmt.Sprintf("**Status:** %s\n**Progress:** %s", status, progressStr),
				Color:       0x5865F2,
				Fields: []*discordgo.MessageEmbedField{
					{
						Name:   "Requested By",
						Value:  track.RequestedBy,
						Inline: true,
					},
					{
						Name:   "Source",
						Value:  track.Source,
						Inline: true,
					},
					{
						Name:   "Volume",
						Value:  fmt.Sprintf("%d%%", player.Volume()),
						Inline: true,
					},
				},
			}

			if track.Thumbnail != "" {
				embed.Thumbnail = &discordgo.MessageEmbedThumbnail{
					URL: track.Thumbnail,
				}
			}

			respondEmbed(s, i, embed)
		},
		"volume": func(s *discordgo.Session, i *discordgo.InteractionCreate) {
			userID, _ := extractUser(i)
			if i.GuildID == "" {
				respondText(s, i, "❌ This command can only be used inside a Discord server.")
				return
			}

			player, err := b.musicMgr.GetOrCreate(i.GuildID)
			if err != nil {
				b.logger.Error("Failed to get/create guild player", "guild_id", i.GuildID, "user_id", userID, "command", "volume", "error", err)
				respondText(s, i, "❌ Internal error accessing guild player.")
				return
			}

			options := i.ApplicationCommandData().Options
			if len(options) == 0 {
				respondText(s, i, fmt.Sprintf("🔊 Current volume is **%d%%**.", player.Volume()))
				return
			}

			vol := options[0].IntValue()
			if vol < 0 || vol > 100 {
				respondText(s, i, "❌ Volume must be an integer between 0 and 100.")
				return
			}

			player.SetVolume(context.Background(), int(vol))
			b.logger.Info("Volume changed", "guild_id", i.GuildID, "user_id", userID, "command", "volume", "volume", vol)

			if vol == 0 {
				respondText(s, i, "🔇 Volume set to **0%** (Muted).")
			} else {
				respondText(s, i, fmt.Sprintf("🔊 Volume set to **%d%%**.", vol))
			}
		},
	}
}

// RegisterCommands registers slash commands with Discord globally.
func (b *Bot) RegisterCommands() error {
	appID := b.Session.State.User.ID
	b.logger.Info("Registering slash commands...", "app_id", appID, "commands_count", len(Commands))

	_, err := b.Session.ApplicationCommandBulkOverwrite(appID, "", Commands)
	if err != nil {
		return fmt.Errorf("failed to bulk overwrite slash commands: %w", err)
	}

	b.logger.Info("Slash commands registered successfully")
	return nil
}
