package main

import (
	"log/slog"
	"os"
	"os/exec"
	"os/signal"
	"syscall"

	"discord-music-bot/internal/config"
	"discord-music-bot/internal/discord"
)

func setupLogger(levelStr string) *slog.Logger {
	var level slog.Level
	switch levelStr {
	case "DEBUG":
		level = slog.LevelDebug
	case "WARN":
		level = slog.LevelWarn
	case "ERROR":
		level = slog.LevelError
	default:
		level = slog.LevelInfo
	}

	opts := &slog.HandlerOptions{
		Level: level,
	}

	logger := slog.New(slog.NewTextHandler(os.Stdout, opts))
	slog.SetDefault(logger)
	return logger
}

func checkDependencies(cfg *config.Config, logger *slog.Logger) {
	if _, err := exec.LookPath(cfg.FFmpegPath); err != nil {
		logger.Warn("FFmpeg executable was not found on PATH or specified location", "ffmpeg_path", cfg.FFmpegPath, "error", err)
	} else {
		logger.Info("FFmpeg dependency verified", "ffmpeg_path", cfg.FFmpegPath)
	}

	if _, err := exec.LookPath(cfg.YtDlpPath); err != nil {
		logger.Warn("yt-dlp executable was not found on PATH or specified location", "ytdlp_path", cfg.YtDlpPath, "error", err)
	} else {
		logger.Info("yt-dlp dependency verified", "ytdlp_path", cfg.YtDlpPath)
	}
}

func main() {
	cfg, err := config.Load()
	if err != nil {
		slog.Error("Failed to load configuration", "error", err)
		os.Exit(1)
	}

	logger := setupLogger(cfg.LogLevel)

	logger.Info("Starting Discord Music Bot...",
		"log_level", cfg.LogLevel,
		"ffmpeg_path", cfg.FFmpegPath,
		"ytdlp_path", cfg.YtDlpPath,
	)

	checkDependencies(cfg, logger)

	bot, err := discord.New(cfg, logger)
	if err != nil {
		logger.Error("Failed to initialize bot", "error", err)
		os.Exit(1)
	}

	if err := bot.Start(); err != nil {
		logger.Error("Failed to start Discord bot", "error", err)
		os.Exit(1)
	}

	logger.Info("Bot is running. Press CTRL+C to stop.")

	// Wait for interrupt signal for graceful shutdown
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)
	<-stop

	logger.Info("Shutdown signal received, closing session...")

	if err := bot.Close(); err != nil {
		logger.Error("Error during bot shutdown", "error", err)
		os.Exit(1)
	}

	logger.Info("Bot stopped successfully.")
}
