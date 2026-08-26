package music

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestGuildPlayerStateAndVolume(t *testing.T) {
	ctx := context.Background()
	player := NewGuildPlayer("guild-100", nil)

	if player.Volume() != 100 {
		t.Fatalf("expected default volume 100, got %d", player.Volume())
	}

	player.SetVolume(ctx, 150)
	if player.Volume() != 100 {
		t.Errorf("expected volume clamped to 100, got %d", player.Volume())
	}

	player.SetVolume(ctx, -20)
	if player.Volume() != 0 {
		t.Errorf("expected volume clamped to 0, got %d", player.Volume())
	}

	player.SetVolume(ctx, 80)
	if player.Volume() != 80 {
		t.Errorf("expected volume 80, got %d", player.Volume())
	}

	// Playing state
	player.SetPlaying(true)
	player.SetCurrentTrack(&Track{ID: "t1", Title: "Track 1"})
	if !player.IsPlaying() {
		t.Error("expected IsPlaying to be true")
	}

	// Paused state
	_ = player.Pause(ctx)
	if !player.IsPaused() {
		t.Error("expected IsPaused to be true")
	}

	// Stop resets
	_ = player.Stop(ctx)
	if player.IsPlaying() || player.IsPaused() || player.CurrentTrack() != nil {
		t.Error("expected Stop() to reset playing, paused, and currentTrack")
	}
}

func TestPlaybackControlsValidation(t *testing.T) {
	ctx := context.Background()
	player := NewGuildPlayer("guild-controls", nil)

	// 1. Pause when not playing
	if err := player.Pause(ctx); err == nil {
		t.Error("expected error pausing when nothing is playing")
	}

	// 2. Resume when not playing
	if err := player.Resume(ctx); err == nil {
		t.Error("expected error resuming when nothing is playing")
	}

	// 3. Skip when not playing
	if _, err := player.Skip(ctx); err == nil {
		t.Error("expected error skipping when nothing is playing")
	}

	// 4. Stop when queue is empty and not playing
	if err := player.Stop(ctx); err == nil {
		t.Error("expected error stopping when nothing is playing and queue is empty")
	}

	// Simulate active track
	track := &Track{ID: "track1", Title: "Title 1"}
	player.SetCurrentTrack(track)
	player.SetPlaying(true)

	// Pause
	if err := player.Pause(ctx); err != nil {
		t.Fatalf("unexpected error pausing: %v", err)
	}
	if !player.IsPaused() {
		t.Error("expected IsPaused to be true")
	}

	// Pause again should error
	if err := player.Pause(ctx); err == nil {
		t.Error("expected error on redundant pause")
	}

	// Resume
	if err := player.Resume(ctx); err != nil {
		t.Fatalf("unexpected error resuming: %v", err)
	}
	if player.IsPaused() {
		t.Error("expected IsPaused to be false")
	}

	// Resume again should error
	if err := player.Resume(ctx); err == nil {
		t.Error("expected error on redundant resume")
	}

	// Skip
	skipped, err := player.Skip(ctx)
	if err != nil {
		t.Fatalf("unexpected error on Skip(): %v", err)
	}
	if skipped.ID != "track1" {
		t.Errorf("expected skipped ID track1, got %s", skipped.ID)
	}

	// Stop
	_ = player.Queue.Add(&Track{ID: "2", Title: "Title 2"})
	if err := player.Stop(ctx); err != nil {
		t.Fatalf("unexpected error on Stop(): %v", err)
	}
	if player.Queue.Length() != 0 {
		t.Errorf("expected queue length 0 after Stop(), got %d", player.Queue.Length())
	}
}

func TestEnqueueNilTrack(t *testing.T) {
	ctx := context.Background()
	player := NewGuildPlayer("guild-1", nil)
	_, err := player.EnqueueAndPlay(ctx, nil)
	if !errors.Is(err, ErrNilTrack) {
		t.Errorf("expected ErrNilTrack, got %v", err)
	}
}

func TestQueueTransitionsOnEmpty(t *testing.T) {
	player := NewGuildPlayer("guild-1", nil)
	track1 := &Track{ID: "1", Title: "Song 1", URL: "https://example.com/1", Duration: 3 * time.Minute}
	track2 := &Track{ID: "2", Title: "Song 2", URL: "https://example.com/2", Duration: 4 * time.Minute}

	_ = player.Queue.Add(track1)
	_ = player.Queue.Add(track2)

	if player.Queue.Length() != 2 {
		t.Fatalf("expected queue length 2, got %d", player.Queue.Length())
	}

	// Next pops FIFO
	next, _ := player.Queue.Next()
	if next.ID != "1" {
		t.Errorf("expected track 1 first, got %s", next.ID)
	}

	next2, _ := player.Queue.Next()
	if next2.ID != "2" {
		t.Errorf("expected track 2 second, got %s", next2.ID)
	}

	// Queue is now empty
	_, err := player.Queue.Next()
	if !errors.Is(err, ErrQueueEmpty) {
		t.Errorf("expected ErrQueueEmpty, got %v", err)
	}
}
