package music

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"testing"
	"time"
)

func TestGuildIsolation(t *testing.T) {
	mgr := NewGuildMusicManager(nil)

	guildA := "guild-111"
	guildB := "guild-222"

	playerA, err := mgr.GetOrCreate(guildA)
	if err != nil {
		t.Fatalf("failed to get/create player A: %v", err)
	}

	playerB, err := mgr.GetOrCreate(guildB)
	if err != nil {
		t.Fatalf("failed to get/create player B: %v", err)
	}

	if playerA == playerB {
		t.Fatal("expected player A and player B to be separate instances")
	}

	// Add track to Guild A
	trackA := &Track{ID: "track-a", Title: "Guild A Song", Duration: 3 * time.Minute}
	if err := playerA.Queue.Add(trackA); err != nil {
		t.Fatalf("failed to add track to Guild A: %v", err)
	}

	// Verify Guild B queue remains empty
	if playerA.Queue.Length() != 1 {
		t.Errorf("expected Guild A queue length 1, got %d", playerA.Queue.Length())
	}
	if playerB.Queue.Length() != 0 {
		t.Errorf("expected Guild B queue length 0 (isolated), got %d", playerB.Queue.Length())
	}

	// Change volume and state on Guild A
	playerA.SetVolume(context.Background(), 60)
	playerA.SetPlaying(true)
	playerA.SetCurrentTrack(trackA)

	if playerA.Volume() != 60 {
		t.Errorf("expected Guild A volume 60, got %d", playerA.Volume())
	}
	if playerB.Volume() != 100 {
		t.Errorf("expected Guild B volume 100 default, got %d", playerB.Volume())
	}
	if !playerA.IsPlaying() {
		t.Error("expected Guild A to be playing")
	}
	if playerB.IsPlaying() {
		t.Error("expected Guild B not to be playing")
	}
	if playerB.CurrentTrack() != nil {
		t.Error("expected Guild B current track to be nil")
	}
}

func TestGuildManagerLifecycle(t *testing.T) {
	mgr := NewGuildMusicManager(nil)

	// Empty guild ID edge cases
	if _, err := mgr.Get(""); !errors.Is(err, ErrEmptyGuildID) {
		t.Fatalf("expected ErrEmptyGuildID on Get(''), got %v", err)
	}
	if _, err := mgr.GetOrCreate(""); !errors.Is(err, ErrEmptyGuildID) {
		t.Fatalf("expected ErrEmptyGuildID on GetOrCreate(''), got %v", err)
	}
	if err := mgr.Remove(""); !errors.Is(err, ErrEmptyGuildID) {
		t.Fatalf("expected ErrEmptyGuildID on Remove(''), got %v", err)
	}

	// Non-existent guild
	if _, err := mgr.Get("nonexistent"); !errors.Is(err, ErrPlayerNotFound) {
		t.Fatalf("expected ErrPlayerNotFound on non-existent guild, got %v", err)
	}

	// Create and count
	p1, _ := mgr.GetOrCreate("guild-1")
	p2, _ := mgr.GetOrCreate("guild-2")
	if mgr.Count() != 2 {
		t.Fatalf("expected 2 players, got %d", mgr.Count())
	}

	// Getting existing returns exact same instance
	p1Again, err := mgr.Get("guild-1")
	if err != nil || p1Again != p1 {
		t.Fatalf("expected to retrieve same instance for guild-1")
	}

	// Remove
	if err := mgr.Remove("guild-1"); err != nil {
		t.Fatalf("failed to remove guild-1: %v", err)
	}
	if mgr.Count() != 1 {
		t.Fatalf("expected 1 player after removal, got %d", mgr.Count())
	}
	if _, err := mgr.Get("guild-1"); !errors.Is(err, ErrPlayerNotFound) {
		t.Fatalf("expected ErrPlayerNotFound after removal, got %v", err)
	}

	// Remove nonexistent returns error
	if err := mgr.Remove("guild-1"); !errors.Is(err, ErrPlayerNotFound) {
		t.Fatalf("expected ErrPlayerNotFound on second removal, got %v", err)
	}

	_ = p2
}

func TestCloseAll(t *testing.T) {
	mgr := NewGuildMusicManager(nil)

	p1, _ := mgr.GetOrCreate("guild-1")
	p2, _ := mgr.GetOrCreate("guild-2")

	_ = p1.Queue.Add(&Track{ID: "t1", Title: "Track 1"})
	_ = p2.Queue.Add(&Track{ID: "t2", Title: "Track 2"})
	p1.SetPlaying(true)
	p2.SetPlaying(true)

	if mgr.Count() != 2 {
		t.Fatalf("expected 2 active players before CloseAll, got %d", mgr.Count())
	}

	mgr.CloseAll()

	if mgr.Count() != 0 {
		t.Fatalf("expected 0 active players after CloseAll, got %d", mgr.Count())
	}
	if p1.IsPlaying() || p1.Queue.Length() != 0 {
		t.Error("expected p1 queue to be cleared and playing state reset")
	}
	if p2.IsPlaying() || p2.Queue.Length() != 0 {
		t.Error("expected p2 queue to be cleared and playing state reset")
	}
}

func TestGuildManagerConcurrency(t *testing.T) {
	mgr := NewGuildMusicManager(nil)
	numGuilds := 50
	numRoutines := 20

	var wg sync.WaitGroup

	for r := 0; r < numRoutines; r++ {
		wg.Add(1)
		go func(routineID int) {
			defer wg.Done()
			for g := 0; g < numGuilds; g++ {
				guildID := fmt.Sprintf("concurrent-guild-%d", g)
				player, err := mgr.GetOrCreate(guildID)
				if err != nil {
					t.Errorf("error getting or creating player: %v", err)
					return
				}

				// Perform thread-safe operations on player
				player.SetVolume(context.Background(), 50+(g%50))
				_ = player.Queue.Add(&Track{
					ID:    fmt.Sprintf("r%d-g%d", routineID, g),
					Title: "Test Track",
				})

				_ = player.Queue.Length()
				_ = player.Volume()
				_ = mgr.Count()
			}
		}(r)
	}

	wg.Wait()

	if mgr.Count() != numGuilds {
		t.Fatalf("expected %d total created guilds, got %d", numGuilds, mgr.Count())
	}

	// Verify each guild has items added by all routines
	for g := 0; g < numGuilds; g++ {
		guildID := fmt.Sprintf("concurrent-guild-%d", g)
		player, err := mgr.Get(guildID)
		if err != nil {
			t.Fatalf("failed to get guild %s: %v", guildID, err)
		}
		if player.Queue.Length() != numRoutines {
			t.Fatalf("expected queue length %d for %s, got %d", numRoutines, guildID, player.Queue.Length())
		}
	}
}
