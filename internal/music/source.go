package music

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"github.com/disgoorg/disgolink/v3/disgolink"
	"github.com/disgoorg/disgolink/v3/lavalink"
)

var (
	// ErrTrackNotFound is returned when audio resolver fails to find any matching track.
	ErrTrackNotFound = errors.New("no audio track found for query")

	// ErrExtractionFailed is returned when track extraction or parsing fails.
	ErrExtractionFailed = errors.New("failed to extract audio metadata")

	// ErrEmptyQuery is returned when an empty query is passed for resolution.
	ErrEmptyQuery = errors.New("query cannot be empty")
)

// ResolveResult contains the resolved track(s) and playlist name if applicable.
type ResolveResult struct {
	Tracks       []*Track
	PlaylistName string
}

// AudioSource defines the interface for resolving queries and URLs into playable tracks.
type AudioSource interface {
	Resolve(ctx context.Context, query, requestedBy string) (*ResolveResult, error)
}

// LavalinkSource resolves tracks using a DisGoLink Lavalink v4 node.
type LavalinkSource struct {
	client disgolink.Client
}

// NewLavalinkSource creates a new LavalinkSource backed by a DisGoLink client.
func NewLavalinkSource(client disgolink.Client) *LavalinkSource {
	return &LavalinkSource{client: client}
}

// Resolve resolves a URL or search keywords using Lavalink's track loader with automatic retries.
func (s *LavalinkSource) Resolve(ctx context.Context, query, requestedBy string) (*ResolveResult, error) {
	trimmed := strings.TrimSpace(query)
	if trimmed == "" {
		return nil, ErrEmptyQuery
	}

	node := s.client.BestNode()
	if node == nil {
		return nil, errors.New("no Lavalink audio nodes available")
	}

	identifier := trimmed
	if !strings.HasPrefix(trimmed, "http://") && !strings.HasPrefix(trimmed, "https://") {
		identifier = lavalink.SearchTypeYouTube.Apply(trimmed)
	}

	var result *lavalink.LoadResult
	var err error

	// Retry up to 3 times with exponential backoff to handle cold-start worker spinup
	for attempt := 0; attempt < 3; attempt++ {
		if attempt > 0 {
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(time.Duration(attempt*600) * time.Millisecond):
			}
		}

		result, err = node.LoadTracks(ctx, identifier)
		if err == nil && result != nil && result.LoadType != lavalink.LoadTypeEmpty {
			break
		}

		// Fallback searches for plain queries
		if !strings.HasPrefix(trimmed, "http://") && !strings.HasPrefix(trimmed, "https://") {
			result, err = node.LoadTracks(ctx, lavalink.SearchTypeYouTubeMusic.Apply(trimmed))
			if err == nil && result != nil && result.LoadType != lavalink.LoadTypeEmpty {
				break
			}
			result, err = node.LoadTracks(ctx, lavalink.SearchTypeSoundCloud.Apply(trimmed))
			if err == nil && result != nil && result.LoadType != lavalink.LoadTypeEmpty {
				break
			}
		}
	}

	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrExtractionFailed, err)
	}
	if result == nil {
		return nil, ErrTrackNotFound
	}

	switch result.LoadType {
	case lavalink.LoadTypeTrack:
		track, ok := result.Data.(lavalink.Track)
		if !ok {
			return nil, ErrTrackNotFound
		}
		return &ResolveResult{
			Tracks: []*Track{mapLavalinkTrack(track, requestedBy)},
		}, nil

	case lavalink.LoadTypeSearch:
		tracks, ok := result.Data.(lavalink.Search)
		if !ok || len(tracks) == 0 {
			return nil, ErrTrackNotFound
		}
		return &ResolveResult{
			Tracks: []*Track{mapLavalinkTrack(tracks[0], requestedBy)},
		}, nil

	case lavalink.LoadTypePlaylist:
		playlist, ok := result.Data.(lavalink.Playlist)
		if !ok || len(playlist.Tracks) == 0 {
			return nil, ErrTrackNotFound
		}
		tracks := make([]*Track, 0, len(playlist.Tracks))
		for _, t := range playlist.Tracks {
			tracks = append(tracks, mapLavalinkTrack(t, requestedBy))
		}
		name := playlist.Info.Name
		if name == "" {
			name = "YouTube Mix / Playlist"
		}
		return &ResolveResult{
			Tracks:       tracks,
			PlaylistName: name,
		}, nil

	default:
		return nil, ErrTrackNotFound
	}
}

func mapLavalinkTrack(t lavalink.Track, requestedBy string) *Track {
	url := ""
	if t.Info.URI != nil {
		url = *t.Info.URI
	}
	thumb := ""
	if t.Info.ArtworkURL != nil {
		thumb = *t.Info.ArtworkURL
	}
	return &Track{
		ID:            t.Info.Identifier,
		Title:         t.Info.Title,
		URL:           url,
		Duration:      time.Duration(t.Info.Length) * time.Millisecond,
		Thumbnail:     thumb,
		RequestedBy:   requestedBy,
		Source:        t.Info.SourceName,
		LavalinkTrack: &t,
	}
}

// CommandRunner is an interface for executing external processes to enable unit test mocking.
type CommandRunner interface {
	Run(ctx context.Context, name string, args ...string) ([]byte, error)
}

// DefaultCommandRunner executes system commands directly via os/exec without shell interpolation.
type DefaultCommandRunner struct{}

// Run executes the command with context cancellation.
func (r *DefaultCommandRunner) Run(ctx context.Context, name string, args ...string) ([]byte, error) {
	cmd := exec.CommandContext(ctx, name, args...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		errMsg := strings.TrimSpace(stderr.String())
		if errMsg == "" {
			errMsg = err.Error()
		}
		return nil, fmt.Errorf("%w: %s", err, errMsg)
	}
	return stdout.Bytes(), nil
}

// YtDlpSource implements AudioSource using the external yt-dlp binary.
type YtDlpSource struct {
	binaryPath string
	runner     CommandRunner
}

// NewYtDlpSource creates a new YtDlpSource with a given binary path.
func NewYtDlpSource(binaryPath string) *YtDlpSource {
	if binaryPath == "" {
		binaryPath = "yt-dlp"
	}
	return &YtDlpSource{
		binaryPath: binaryPath,
		runner:     &DefaultCommandRunner{},
	}
}

// NewYtDlpSourceWithRunner creates a YtDlpSource with a custom CommandRunner for testing.
func NewYtDlpSourceWithRunner(binaryPath string, runner CommandRunner) *YtDlpSource {
	if binaryPath == "" {
		binaryPath = "yt-dlp"
	}
	return &YtDlpSource{
		binaryPath: binaryPath,
		runner:     runner,
	}
}

type ytdlpMetadata struct {
	ID         string  `json:"id"`
	Title      string  `json:"title"`
	WebpageURL string  `json:"webpage_url"`
	URL        string  `json:"url"`
	Duration   float64 `json:"duration"`
	Thumbnail  string  `json:"thumbnail"`
	Extractor  string  `json:"extractor"`
}

// Resolve resolves a URL or search keyword into a Track object without downloading media files.
func (s *YtDlpSource) Resolve(ctx context.Context, query, requestedBy string) (*ResolveResult, error) {
	trimmedQuery := strings.TrimSpace(query)
	if trimmedQuery == "" {
		return nil, ErrEmptyQuery
	}

	target := trimmedQuery
	if !strings.HasPrefix(trimmedQuery, "http://") && !strings.HasPrefix(trimmedQuery, "https://") {
		target = "ytsearch1:" + trimmedQuery
	}

	args := []string{
		"--dump-json",
		"--no-playlist",
		"--format", "bestaudio/best",
		"--no-warnings",
		"--skip-download",
		target,
	}

	output, err := s.runner.Run(ctx, s.binaryPath, args...)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrExtractionFailed, err)
	}

	track, err := s.parseMetadata(output, requestedBy)
	if err != nil {
		return nil, err
	}
	return &ResolveResult{
		Tracks: []*Track{track},
	}, nil
}

func (s *YtDlpSource) parseMetadata(data []byte, requestedBy string) (*Track, error) {
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 {
		return nil, ErrTrackNotFound
	}

	var meta ytdlpMetadata
	if err := json.Unmarshal(trimmed, &meta); err != nil {
		return nil, fmt.Errorf("%w: %v", ErrExtractionFailed, err)
	}

	if meta.Title == "" && meta.ID == "" {
		return nil, ErrTrackNotFound
	}

	playURL := meta.WebpageURL
	if playURL == "" {
		playURL = meta.URL
	}

	sourceType := meta.Extractor
	if sourceType == "" {
		sourceType = "youtube"
	}

	return &Track{
		ID:          meta.ID,
		Title:       meta.Title,
		URL:         playURL,
		Duration:    time.Duration(meta.Duration * float64(time.Second)),
		Thumbnail:   meta.Thumbnail,
		RequestedBy: requestedBy,
		Source:      sourceType,
	}, nil
}
