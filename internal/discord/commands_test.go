package discord

import (
	"log/slog"
	"os"
	"testing"

	"github.com/bwmarrin/discordgo"

	"discord-music-bot/internal/config"
)

func TestCommandsDefinition(t *testing.T) {
	expectedCommands := map[string]bool{
		"join":       false,
		"leave":      false,
		"play":       false,
		"pause":      false,
		"resume":     false,
		"skip":       false,
		"stop":       false,
		"queue":      false,
		"nowplaying": false,
		"volume":     false,
	}

	for _, cmd := range Commands {
		if _, ok := expectedCommands[cmd.Name]; !ok {
			t.Errorf("unexpected command defined: %s", cmd.Name)
		}
		expectedCommands[cmd.Name] = true

		if cmd.Name == "play" {
			if len(cmd.Options) != 1 {
				t.Fatalf("expected 1 option for play command, got %d", len(cmd.Options))
			}
			opt := cmd.Options[0]
			if opt.Name != "query" || opt.Type != discordgo.ApplicationCommandOptionString || !opt.Required {
				t.Errorf("invalid option configuration for play command: %+v", opt)
			}
		}

		if cmd.Name == "volume" {
			if len(cmd.Options) != 1 {
				t.Fatalf("expected 1 option for volume command, got %d", len(cmd.Options))
			}
			opt := cmd.Options[0]
			if opt.Name != "volume" || opt.Type != discordgo.ApplicationCommandOptionInteger || !opt.Required {
				t.Errorf("invalid option configuration for volume command: %+v", opt)
			}
			if opt.MinValue == nil || *opt.MinValue != 0 || opt.MaxValue != 100 {
				t.Errorf("invalid min/max value for volume command: min=%v, max=%v", opt.MinValue, opt.MaxValue)
			}
		}
	}

	for name, found := range expectedCommands {
		if !found {
			t.Errorf("command %s was not found in Commands definition", name)
		}
	}
}

func TestCommandHandlersExist(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, nil))
	cfg := &config.Config{DiscordBotToken: "test_token"}
	bot, err := New(cfg, logger)
	if err != nil {
		t.Fatalf("unexpected error creating bot: %v", err)
	}

	handlers := bot.defaultCommandHandlers()
	for _, cmd := range Commands {
		if _, exists := handlers[cmd.Name]; !exists {
			t.Errorf("missing handler for command: %s", cmd.Name)
		}
	}
}

func TestExtractUser(t *testing.T) {
	// 1. From Guild Member
	i1 := &discordgo.InteractionCreate{
		Interaction: &discordgo.Interaction{
			Member: &discordgo.Member{
				User: &discordgo.User{
					ID:       "user-member-1",
					Username: "MemberOne",
				},
			},
		},
	}
	uID, uName := extractUser(i1)
	if uID != "user-member-1" || uName != "MemberOne" {
		t.Errorf("expected user-member-1 / MemberOne, got %s / %s", uID, uName)
	}

	// 2. From Direct User (DM)
	i2 := &discordgo.InteractionCreate{
		Interaction: &discordgo.Interaction{
			User: &discordgo.User{
				ID:       "user-dm-2",
				Username: "DMUser",
			},
		},
	}
	uID2, uName2 := extractUser(i2)
	if uID2 != "user-dm-2" || uName2 != "DMUser" {
		t.Errorf("expected user-dm-2 / DMUser, got %s / %s", uID2, uName2)
	}

	// 3. Nil User
	i3 := &discordgo.InteractionCreate{
		Interaction: &discordgo.Interaction{},
	}
	uID3, uName3 := extractUser(i3)
	if uID3 != "" || uName3 != "" {
		t.Errorf("expected empty strings for nil user, got %s / %s", uID3, uName3)
	}
}
