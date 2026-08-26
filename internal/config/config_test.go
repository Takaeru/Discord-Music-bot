package config

import (
	"os"
	"testing"
)

func TestConfigLoadDefaults(t *testing.T) {
	// Clear relevant env vars
	os.Unsetenv("DISCORD_BOT_TOKEN")
	os.Unsetenv("LOG_LEVEL")
	os.Unsetenv("FFMPEG_PATH")
	os.Unsetenv("YTDLP_PATH")

	cfg, err := Load()
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}

	if cfg.LogLevel != "INFO" {
		t.Errorf("expected default LogLevel to be 'INFO', got '%s'", cfg.LogLevel)
	}
	if cfg.FFmpegPath != "ffmpeg" {
		t.Errorf("expected default FFmpegPath to be 'ffmpeg', got '%s'", cfg.FFmpegPath)
	}
	if cfg.YtDlpPath != "yt-dlp" {
		t.Errorf("expected default YtDlpPath to be 'yt-dlp', got '%s'", cfg.YtDlpPath)
	}
	if cfg.DiscordBotToken != "" {
		t.Errorf("expected empty token by default, got '%s'", cfg.DiscordBotToken)
	}
}

func TestConfigValidate(t *testing.T) {
	cfg := &Config{
		DiscordBotToken: "",
	}
	if err := cfg.Validate(); err == nil {
		t.Error("expected error when DiscordBotToken is empty, got nil")
	}

	cfg.DiscordBotToken = "dummy_token"
	if err := cfg.Validate(); err != nil {
		t.Errorf("expected no error when DiscordBotToken is set, got %v", err)
	}
}
