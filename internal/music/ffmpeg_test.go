package music

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestFFmpegBuildArgs(t *testing.T) {
	pipeline := NewFFmpegPipeline("ffmpeg")

	args := pipeline.BuildArgs("https://example.com/audio.mp3", 75)

	expectedArgs := []string{
		"-reconnect", "1",
		"-reconnect_streamed", "1",
		"-reconnect_delay_max", "5",
		"-i", "https://example.com/audio.mp3",
		"-vn",
		"-filter:a", "volume=0.75",
		"-f", "s16le",
		"-ar", "48000",
		"-ac", "2",
		"pipe:1",
	}

	if len(args) != len(expectedArgs) {
		t.Fatalf("expected %d args, got %d", len(expectedArgs), len(args))
	}

	for i, arg := range args {
		if arg != expectedArgs[i] {
			t.Errorf("arg[%d]: expected '%s', got '%s'", i, expectedArgs[i], arg)
		}
	}
}

func TestFFmpegVolumeBounds(t *testing.T) {
	pipeline := NewFFmpegPipeline("")

	argsNegative := pipeline.BuildArgs("url", -10)
	foundVol0 := false
	for _, a := range argsNegative {
		if a == "volume=0.00" {
			foundVol0 = true
		}
	}
	if !foundVol0 {
		t.Error("expected volume=0.00 for negative volume")
	}

	argsOver := pipeline.BuildArgs("url", 150)
	foundVol1 := false
	for _, a := range argsOver {
		if a == "volume=1.00" {
			foundVol1 = true
		}
	}
	if !foundVol1 {
		t.Error("expected volume=1.00 for volume > 100")
	}
}

func TestFFmpegErrorsAndCancellation(t *testing.T) {
	pipeline := NewFFmpegPipeline("invalid-ffmpeg-path-xyz")

	// 1. Empty URL
	_, err := pipeline.NewStream(context.Background(), "", 100)
	if !errors.Is(err, ErrEmptySourceURL) {
		t.Errorf("expected ErrEmptySourceURL, got %v", err)
	}

	// 2. Invalid binary
	_, err = pipeline.NewStream(context.Background(), "https://example.com/audio", 100)
	if !errors.Is(err, ErrStreamFailed) {
		t.Errorf("expected ErrStreamFailed on invalid executable, got %v", err)
	}

	// 3. AudioStream Close idempotency
	stream := &AudioStream{}
	if err := stream.Close(); err != nil {
		t.Errorf("unexpected error on Close: %v", err)
	}
	// Second close should be safe and return nil
	if err := stream.Close(); err != nil {
		t.Errorf("unexpected error on second Close: %v", err)
	}
}

func TestAudioStreamContextCancellation(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	pipeline := NewFFmpegPipeline("invalid-binary")

	_, err := pipeline.NewStream(ctx, "https://example.com/audio", 100)
	if err == nil {
		t.Fatal("expected error with invalid binary")
	}

	cancel()
	// Allow any goroutines to terminate
	time.Sleep(10 * time.Millisecond)
}
