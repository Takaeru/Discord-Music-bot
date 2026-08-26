package discord

import (
	"log/slog"
	"os"
	"testing"

	"discord-music-bot/internal/config"
	"discord-music-bot/internal/music"
)

func TestNewBotValidation(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, nil))

	// Missing token should fail validation
	cfg := &config.Config{
		DiscordBotToken: "",
	}

	bot, err := New(cfg, logger)
	if err == nil {
		t.Fatal("expected error when DiscordBotToken is empty, got nil")
	}
	if bot != nil {
		t.Fatal("expected bot to be nil on error")
	}

	// Valid token should initialize bot instance
	cfg.DiscordBotToken = "dummy_token"
	bot, err = New(cfg, logger)
	if err != nil {
		t.Fatalf("unexpected error got %v", err)
	}
	if bot == nil || bot.Session == nil {
		t.Fatal("expected non-nil bot and session")
	}
}

func TestBotGracefulShutdown(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, nil))
	cfg := &config.Config{DiscordBotToken: "test_token"}

	bot, err := New(cfg, logger)
	if err != nil {
		t.Fatalf("unexpected error creating bot: %v", err)
	}

	// Add guild player with a track
	p, _ := bot.musicMgr.GetOrCreate("guild-shutdown-test")
	_ = p.Queue.Add(&music.Track{ID: "track-1", Title: "Song 1"})
	p.SetPlaying(true)

	// Close bot should stop all players and close session cleanly
	err = bot.Close()
	if err != nil {
		t.Fatalf("unexpected error closing bot: %v", err)
	}

	if bot.musicMgr.Count() != 0 {
		t.Errorf("expected 0 active players after Close, got %d", bot.musicMgr.Count())
	}
	if !bot.isStopping {
		t.Error("expected bot.isStopping to be true after Close")
	}
}
