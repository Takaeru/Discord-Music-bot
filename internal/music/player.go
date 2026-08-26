package music

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/disgoorg/disgolink/v3/disgolink"
	"github.com/disgoorg/disgolink/v3/lavalink"
	"github.com/disgoorg/snowflake/v2"
)

// GuildPlayer manages the music playback, queue, and Lavalink player for a specific guild.
type GuildPlayer struct {
	mu           sync.RWMutex
	GuildID      string
	Queue        *Queue
	currentTrack *Track
	isPlaying    bool
	isPaused     bool
	volume       int
	lavalink     disgolink.Client
}

// NewGuildPlayer creates a new player instance for a guild with an isolated queue and default volume.
func NewGuildPlayer(guildID string, lavalink disgolink.Client) *GuildPlayer {
	return &GuildPlayer{
		GuildID:  guildID,
		Queue:    NewQueue(),
		volume:   100,
		lavalink: lavalink,
	}
}

// CurrentTrack returns the currently playing track.
func (p *GuildPlayer) CurrentTrack() *Track {
	p.mu.RLock()
	defer p.mu.RUnlock()
	return p.currentTrack
}

// SetCurrentTrack sets the currently playing track.
func (p *GuildPlayer) SetCurrentTrack(t *Track) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.currentTrack = t
}

// IsPlaying returns whether audio is currently playing.
func (p *GuildPlayer) IsPlaying() bool {
	p.mu.RLock()
	defer p.mu.RUnlock()
	return p.isPlaying
}

// SetPlaying updates the playing status.
func (p *GuildPlayer) SetPlaying(playing bool) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.isPlaying = playing
}

// IsPaused returns whether audio is currently paused.
func (p *GuildPlayer) IsPaused() bool {
	p.mu.RLock()
	defer p.mu.RUnlock()
	return p.isPaused
}

// Pause pauses the currently playing track.
func (p *GuildPlayer) Pause(ctx context.Context) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	if !p.isPlaying || p.currentTrack == nil {
		return errors.New("nothing is currently playing")
	}
	if p.isPaused {
		return errors.New("playback is already paused")
	}

	p.isPaused = true
	if p.lavalink != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			if player := p.lavalink.ExistingPlayer(guildSnowflake); player != nil {
				_ = player.Update(ctx, lavalink.WithPaused(true))
			}
		}
	}
	return nil
}

// Resume resumes the currently paused track.
func (p *GuildPlayer) Resume(ctx context.Context) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	if !p.isPlaying || p.currentTrack == nil {
		return errors.New("nothing is currently playing")
	}
	if !p.isPaused {
		return errors.New("playback is not paused")
	}

	p.isPaused = false
	if p.lavalink != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			if player := p.lavalink.ExistingPlayer(guildSnowflake); player != nil {
				_ = player.Update(ctx, lavalink.WithPaused(false))
			}
		}
	}
	return nil
}

// Volume returns the current volume setting (0-100).
func (p *GuildPlayer) Volume() int {
	p.mu.RLock()
	defer p.mu.RUnlock()
	return p.volume
}

// SetVolume updates the playback volume bounded between 0 and 100.
func (p *GuildPlayer) SetVolume(ctx context.Context, vol int) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if vol < 0 {
		vol = 0
	} else if vol > 100 {
		vol = 100
	}
	p.volume = vol

	if p.lavalink != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			if player := p.lavalink.ExistingPlayer(guildSnowflake); player != nil {
				_ = player.Update(ctx, lavalink.WithVolume(vol))
			}
		}
	}
}

// PlaybackPosition returns the current playback progress of the playing track.
func (p *GuildPlayer) PlaybackPosition() time.Duration {
	p.mu.RLock()
	defer p.mu.RUnlock()

	if p.lavalink != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			if player := p.lavalink.ExistingPlayer(guildSnowflake); player != nil {
				return time.Duration(player.Position()) * time.Millisecond
			}
		}
	}
	return 0
}

// EnqueueAndPlay adds the track to the queue or starts immediate playback on the Lavalink player.
func (p *GuildPlayer) EnqueueAndPlay(ctx context.Context, track *Track) (bool, error) {
	if track == nil {
		return false, ErrNilTrack
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	if p.isPlaying && p.currentTrack != nil {
		if err := p.Queue.Add(track); err != nil {
			return false, err
		}
		return true, nil
	}

	p.currentTrack = track
	p.isPlaying = true
	p.isPaused = false

	if p.lavalink != nil && track.LavalinkTrack != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			player := p.lavalink.Player(guildSnowflake)
			if player != nil {
				_ = player.Update(ctx,
					lavalink.WithTrack(*track.LavalinkTrack),
					lavalink.WithVolume(p.volume),
				)
			}
		}
	}

	return false, nil
}

// PlayNext plays the next track in the queue, or resets to idle if the queue is empty.
func (p *GuildPlayer) PlayNext(ctx context.Context) (*Track, error) {
	p.mu.Lock()
	defer p.mu.Unlock()

	nextTrack, err := p.Queue.Next()
	if err != nil {
		p.isPlaying = false
		p.isPaused = false
		p.currentTrack = nil
		if p.lavalink != nil {
			if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
				if player := p.lavalink.ExistingPlayer(guildSnowflake); player != nil {
					_ = player.Update(ctx, lavalink.WithNullTrack())
				}
			}
		}
		return nil, err
	}

	p.currentTrack = nextTrack
	p.isPlaying = true
	p.isPaused = false

	if p.lavalink != nil && nextTrack.LavalinkTrack != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			player := p.lavalink.Player(guildSnowflake)
			if player != nil {
				_ = player.Update(ctx,
					lavalink.WithTrack(*nextTrack.LavalinkTrack),
					lavalink.WithVolume(p.volume),
				)
			}
		}
	}

	return nextTrack, nil
}

// Skip terminates the current track and starts the next queued track.
func (p *GuildPlayer) Skip(ctx context.Context) (*Track, error) {
	p.mu.Lock()
	if !p.isPlaying || p.currentTrack == nil {
		p.mu.Unlock()
		return nil, errors.New("nothing is currently playing to skip")
	}
	skipped := p.currentTrack
	p.mu.Unlock()

	_, _ = p.PlayNext(ctx)
	return skipped, nil
}

// Stop stops playback, clears the queue, and resets the player.
func (p *GuildPlayer) Stop(ctx context.Context) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	if !p.isPlaying && p.Queue.Length() == 0 {
		return errors.New("nothing is currently playing and the queue is empty")
	}

	p.Queue.Clear()
	p.isPlaying = false
	p.isPaused = false
	p.currentTrack = nil

	if p.lavalink != nil {
		if guildSnowflake, err := snowflake.Parse(p.GuildID); err == nil {
			if player := p.lavalink.ExistingPlayer(guildSnowflake); player != nil {
				_ = player.Update(ctx, lavalink.WithNullTrack())
			}
		}
	}
	return nil
}
