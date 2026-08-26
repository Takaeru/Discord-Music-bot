package discord

import (
	"errors"
	"log/slog"
	"os"
	"testing"

	"github.com/bwmarrin/discordgo"

	"discord-music-bot/internal/config"
)

func TestFindUserVoiceChannel(t *testing.T) {
	session, err := discordgo.New("Bot test_token")
	if err != nil {
		t.Fatalf("failed to create session: %v", err)
	}

	// Empty guild ID
	_, err = FindUserVoiceChannel(session, "", "user-1")
	if !errors.Is(err, ErrNotInGuild) {
		t.Errorf("expected ErrNotInGuild, got %v", err)
	}

	// Mock state with a guild and voice states
	guild := &discordgo.Guild{
		ID: "guild-1",
		VoiceStates: []*discordgo.VoiceState{
			{
				UserID:    "user-1",
				ChannelID: "voice-chan-1",
			},
		},
	}
	session.State.GuildAdd(guild)

	// User present in voice
	chanID, err := FindUserVoiceChannel(session, "guild-1", "user-1")
	if err != nil {
		t.Fatalf("unexpected error finding voice channel: %v", err)
	}
	if chanID != "voice-chan-1" {
		t.Errorf("expected voice-chan-1, got %s", chanID)
	}

	// User not present in voice
	_, err = FindUserVoiceChannel(session, "guild-1", "user-2")
	if !errors.Is(err, ErrUserNotInVoice) {
		t.Errorf("expected ErrUserNotInVoice, got %v", err)
	}
}

func TestLeaveVoiceChannelNotInVoice(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, nil))
	cfg := &config.Config{DiscordBotToken: "test_token"}
	bot, err := New(cfg, logger)
	if err != nil {
		t.Fatalf("failed to create bot: %v", err)
	}

	err = bot.LeaveVoiceChannel("guild-1")
	if !errors.Is(err, ErrBotNotInVoice) {
		t.Errorf("expected ErrBotNotInVoice, got %v", err)
	}
}

func TestCheckVoicePermissions(t *testing.T) {
	session, err := discordgo.New("Bot test_token")
	if err != nil {
		t.Fatalf("failed to create session: %v", err)
	}

	// Without state, CheckVoicePermissions returns nil (safe fallback)
	err = CheckVoicePermissions(session, "channel-1")
	if err != nil {
		t.Errorf("expected nil error on missing state cache, got %v", err)
	}
}
