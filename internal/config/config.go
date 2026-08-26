package config

import (
	"errors"
	"os"
	"strconv"
	"strings"

	"github.com/joho/godotenv"
)

// Config holds all configuration values required by the application.
type Config struct {
	DiscordBotToken  string
	LogLevel         string
	FFmpegPath       string
	YtDlpPath        string
	LavalinkHost     string
	LavalinkPort     int
	LavalinkPassword string
	LavalinkSecure   bool
}

// Load reads configuration from environment variables and an optional .env file.
func Load() (*Config, error) {
	// Attempt to load .env file if present, ignore if not found
	_ = godotenv.Load()

	port, _ := strconv.Atoi(getEnv("LAVALINK_PORT", "2333"))
	if port <= 0 {
		port = 2333
	}

	secure, _ := strconv.ParseBool(getEnv("LAVALINK_SECURE", "false"))

	cfg := &Config{
		DiscordBotToken:  getEnv("DISCORD_BOT_TOKEN", ""),
		LogLevel:         strings.ToUpper(getEnv("LOG_LEVEL", "INFO")),
		FFmpegPath:       getEnv("FFMPEG_PATH", "ffmpeg"),
		YtDlpPath:        getEnv("YTDLP_PATH", "yt-dlp"),
		LavalinkHost:     getEnv("LAVALINK_HOST", "localhost"),
		LavalinkPort:     port,
		LavalinkPassword: getEnv("LAVALINK_PASSWORD", "youshallnotpass"),
		LavalinkSecure:   secure,
	}

	return cfg, nil
}

// Validate checks whether the mandatory configuration values are present.
func (c *Config) Validate() error {
	if strings.TrimSpace(c.DiscordBotToken) == "" {
		return errors.New("DISCORD_BOT_TOKEN is required")
	}
	return nil
}

// getEnv retrieves an environment variable or returns a default fallback value.
func getEnv(key, defaultValue string) string {
	if val, ok := os.LookupEnv(key); ok && val != "" {
		return val
	}
	return defaultValue
}
