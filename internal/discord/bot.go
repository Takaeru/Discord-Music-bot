package discord

import (
	"context"
	"fmt"
	"log/slog"
	"sync"

	"github.com/bwmarrin/discordgo"
	"github.com/disgoorg/disgolink/v3/disgolink"
	"github.com/disgoorg/disgolink/v3/lavalink"
	"github.com/disgoorg/snowflake/v2"

	"discord-music-bot/internal/config"
	"discord-music-bot/internal/music"
)

// Bot represents the Discord bot instance and manages its session lifecycle.
type Bot struct {
	Session     *discordgo.Session
	Lavalink    disgolink.Client
	cfg         *config.Config
	logger      *slog.Logger
	musicMgr    *music.GuildMusicManager
	audioSource music.AudioSource
	handlers    map[string]func(s *discordgo.Session, i *discordgo.InteractionCreate)
	mu          sync.RWMutex
	isStopping  bool
}

// New creates and configures a new Discord Bot instance.
func New(cfg *config.Config, logger *slog.Logger) (*Bot, error) {
	if err := cfg.Validate(); err != nil {
		return nil, fmt.Errorf("invalid configuration: %w", err)
	}

	session, err := discordgo.New("Bot " + cfg.DiscordBotToken)
	if err != nil {
		return nil, fmt.Errorf("failed to create discord session: %w", err)
	}

	// Minimal required intents: Guilds and GuildVoiceStates
	session.Identify.Intents = discordgo.IntentsGuilds | discordgo.IntentsGuildVoiceStates

	musicMgr := music.NewGuildMusicManager(nil)

	b := &Bot{
		Session:     session,
		cfg:         cfg,
		logger:      logger,
		musicMgr:    musicMgr,
		audioSource: music.NewYtDlpSource(cfg.YtDlpPath),
	}

	b.handlers = b.defaultCommandHandlers()
	b.registerHandlers()

	return b, nil
}

// registerHandlers registers lifecycle and interaction event listeners.
func (b *Bot) registerHandlers() {
	// Ready event handler
	b.Session.AddHandler(func(s *discordgo.Session, r *discordgo.Ready) {
		tag := r.User.Username
		if r.User.Discriminator != "0" && r.User.Discriminator != "" {
			tag += "#" + r.User.Discriminator
		}

		b.logger.Info("Discord bot is ready and connected",
			"username", tag,
			"user_id", r.User.ID,
			"guilds_count", len(r.Guilds),
		)

		b.initLavalink(r.User.ID)

		if err := b.RegisterCommands(); err != nil {
			b.logger.Error("Failed to register slash commands", "error", err)
		}
	})

	// Forward VoiceServerUpdate to Lavalink
	b.Session.AddHandler(func(s *discordgo.Session, event *discordgo.VoiceServerUpdate) {
		if b.Lavalink == nil {
			return
		}
		guildID, err := snowflake.Parse(event.GuildID)
		if err != nil {
			return
		}
		b.Lavalink.OnVoiceServerUpdate(context.Background(), guildID, event.Token, event.Endpoint)
	})

	// Forward VoiceStateUpdate to Lavalink
	b.Session.AddHandler(func(s *discordgo.Session, event *discordgo.VoiceStateUpdate) {
		if b.Lavalink == nil || s.State == nil || s.State.User == nil || event.UserID != s.State.User.ID {
			return
		}
		guildID, err := snowflake.Parse(event.GuildID)
		if err != nil {
			return
		}
		var channelID *snowflake.ID
		if event.ChannelID != "" {
			if id, err := snowflake.Parse(event.ChannelID); err == nil {
				channelID = &id
			}
		}
		b.Lavalink.OnVoiceStateUpdate(context.Background(), guildID, channelID, event.SessionID)
	})

	// Interaction event handler for slash commands
	b.Session.AddHandler(func(s *discordgo.Session, i *discordgo.InteractionCreate) {
		if i.Type != discordgo.InteractionApplicationCommand {
			return
		}

		b.mu.RLock()
		stopping := b.isStopping
		b.mu.RUnlock()

		if stopping {
			respondText(s, i, "⚠️ Bot is currently shutting down. Please try again in a moment.")
			return
		}

		data := i.ApplicationCommandData()
		cmdName := data.Name

		userID := ""
		if i.Member != nil && i.Member.User != nil {
			userID = i.Member.User.ID
		} else if i.User != nil {
			userID = i.User.ID
		}

		b.logger.Info("Received slash command",
			"command", cmdName,
			"guild_id", i.GuildID,
			"user_id", userID,
		)

		if handler, exists := b.handlers[cmdName]; exists {
			handler(s, i)
		} else {
			b.logger.Warn("Unhandled slash command received", "command", cmdName)
			respondText(s, i, fmt.Sprintf("⚠️ Unknown command `/%s`.", cmdName))
		}
	})
}

// initLavalink initializes the DisGoLink client and connects to the configured Lavalink node.
func (b *Bot) initLavalink(userID string) {
	botID, err := snowflake.Parse(userID)
	if err != nil {
		b.logger.Error("Failed to parse bot user ID as snowflake", "error", err)
		return
	}

	b.Lavalink = disgolink.New(botID,
		disgolink.WithListenerFunc(func(p disgolink.Player, e lavalink.TrackEndEvent) {
			b.logger.Info("Lavalink track ended", "guild_id", p.GuildID().String(), "reason", string(e.Reason), "may_start_next", e.Reason.MayStartNext())
			if !e.Reason.MayStartNext() {
				return
			}
			guildID := p.GuildID().String()
			if player, err := b.musicMgr.Get(guildID); err == nil {
				next, err := player.PlayNext(context.Background())
				if err != nil {
					b.logger.Info("Queue finished or empty", "guild_id", guildID)
				} else {
					b.logger.Info("Playing next track in queue", "guild_id", guildID, "track_title", next.Title)
				}
			}
		}),
		disgolink.WithListenerFunc(func(p disgolink.Player, e lavalink.TrackExceptionEvent) {
			b.logger.Warn("Lavalink track exception encountered, advancing queue", "guild_id", p.GuildID().String(), "error", e.Exception.Message)
			guildID := p.GuildID().String()
			if player, err := b.musicMgr.Get(guildID); err == nil {
				_, _ = player.PlayNext(context.Background())
			}
		}),
		disgolink.WithListenerFunc(func(p disgolink.Player, e lavalink.TrackStuckEvent) {
			b.logger.Warn("Lavalink track stuck, advancing queue", "guild_id", p.GuildID().String(), "threshold", e.Threshold)
			guildID := p.GuildID().String()
			if player, err := b.musicMgr.Get(guildID); err == nil {
				_, _ = player.PlayNext(context.Background())
			}
		}),
	)

	b.musicMgr.SetLavalink(b.Lavalink)
	b.audioSource = music.NewLavalinkSource(b.Lavalink)

	nodeConfig := disgolink.NodeConfig{
		Name:     "lavalink-main",
		Address:  fmt.Sprintf("%s:%d", b.cfg.LavalinkHost, b.cfg.LavalinkPort),
		Password: b.cfg.LavalinkPassword,
		Secure:   b.cfg.LavalinkSecure,
	}

	b.logger.Info("Connecting to Lavalink node...", "host", b.cfg.LavalinkHost, "port", b.cfg.LavalinkPort)
	node, err := b.Lavalink.AddNode(context.Background(), nodeConfig)
	if err != nil {
		b.logger.Warn("Initial connection to Lavalink node failed (will retry in background)", "error", err)
	} else {
		b.logger.Info("Connected to Lavalink node successfully", "node", node.Config().Name)
	}
}

// Start opens the Discord websocket gateway connection.
func (b *Bot) Start() error {
	b.logger.Info("Connecting to Discord gateway...")
	if err := b.Session.Open(); err != nil {
		return fmt.Errorf("failed to open discord session: %w", err)
	}
	return nil
}

// Close gracefully stops all playback streams, closes Lavalink, and closes gateway session.
func (b *Bot) Close() error {
	b.mu.Lock()
	b.isStopping = true
	b.mu.Unlock()

	b.logger.Info("Terminating active guild playback streams and voice connections...")
	b.musicMgr.CloseAll()

	if b.Lavalink != nil {
		b.Lavalink.Close()
	}

	b.logger.Info("Closing Discord gateway connection...")
	if err := b.Session.Close(); err != nil {
		return fmt.Errorf("failed to close discord session: %w", err)
	}
	return nil
}
