package music

import (
	"context"
	"errors"
	"sync"

	"github.com/disgoorg/disgolink/v3/disgolink"
)

var (
	// ErrPlayerNotFound is returned when querying a player that does not exist.
	ErrPlayerNotFound = errors.New("guild player not found")

	// ErrEmptyGuildID is returned when an invalid or empty guild ID is provided.
	ErrEmptyGuildID = errors.New("guild ID cannot be empty")
)

// GuildMusicManager manages isolated music player instances for all connected Discord guilds.
type GuildMusicManager struct {
	mu       sync.RWMutex
	players  map[string]*GuildPlayer
	lavalink disgolink.Client
}

// NewGuildMusicManager creates a new thread-safe GuildMusicManager.
func NewGuildMusicManager(lavalink disgolink.Client) *GuildMusicManager {
	return &GuildMusicManager{
		players:  make(map[string]*GuildPlayer),
		lavalink: lavalink,
	}
}

// SetLavalink updates the Lavalink client reference.
func (m *GuildMusicManager) SetLavalink(lavalink disgolink.Client) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.lavalink = lavalink
	for _, p := range m.players {
		p.lavalink = lavalink
	}
}

// Get retrieves a guild player by guild ID without creating one if it doesn't exist.
func (m *GuildMusicManager) Get(guildID string) (*GuildPlayer, error) {
	if guildID == "" {
		return nil, ErrEmptyGuildID
	}

	m.mu.RLock()
	defer m.mu.RUnlock()

	player, exists := m.players[guildID]
	if !exists {
		return nil, ErrPlayerNotFound
	}
	return player, nil
}

// GetOrCreate gets an existing player or initializes a new isolated player for the guild.
func (m *GuildMusicManager) GetOrCreate(guildID string) (*GuildPlayer, error) {
	if guildID == "" {
		return nil, ErrEmptyGuildID
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	if player, exists := m.players[guildID]; exists {
		return player, nil
	}

	player := NewGuildPlayer(guildID, m.lavalink)
	m.players[guildID] = player
	return player, nil
}

// Remove stops and removes the music player for a guild.
func (m *GuildMusicManager) Remove(guildID string) error {
	if guildID == "" {
		return ErrEmptyGuildID
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	player, exists := m.players[guildID]
	if !exists {
		return ErrPlayerNotFound
	}

	_ = player.Stop(context.Background())
	delete(m.players, guildID)
	return nil
}

// Count returns the number of active guild players.
func (m *GuildMusicManager) Count() int {
	m.mu.RLock()
	defer m.mu.RUnlock()

	return len(m.players)
}

// CloseAll stops all active guild players, resets Lavalink nodes, and clears state.
func (m *GuildMusicManager) CloseAll() {
	m.mu.Lock()
	defer m.mu.Unlock()

	for guildID, player := range m.players {
		_ = player.Stop(context.Background())
		delete(m.players, guildID)
	}
}
