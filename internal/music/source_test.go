package music

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"
)

type mockCommandRunner struct {
	output []byte
	err    error
	called bool
	args   []string
}

func (m *mockCommandRunner) Run(ctx context.Context, name string, args ...string) ([]byte, error) {
	m.called = true
	m.args = args
	if ctx.Err() != nil {
		return nil, ctx.Err()
	}
	return m.output, m.err
}

func TestYtDlpSourceResolveSuccess(t *testing.T) {
	mockJSON := `{
		"id": "dQw4w9WgXcQ",
		"title": "Never Gonna Give You Up",
		"webpage_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
		"duration": 213.0,
		"thumbnail": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
		"extractor": "youtube"
	}`

	runner := &mockCommandRunner{output: []byte(mockJSON)}
	source := NewYtDlpSourceWithRunner("custom-ytdlp", runner)

	ctx := context.Background()
	res, err := source.Resolve(ctx, "https://www.youtube.com/watch?v=dQw4w9WgXcQ", "user123")
	if err != nil {
		t.Fatalf("unexpected error resolving track: %v", err)
	}
	if len(res.Tracks) == 0 {
		t.Fatal("expected at least 1 track in result")
	}
	track := res.Tracks[0]

	if !runner.called {
		t.Fatal("expected runner to be called")
	}
	if track.ID != "dQw4w9WgXcQ" {
		t.Errorf("expected track ID dQw4w9WgXcQ, got %s", track.ID)
	}
	if track.Title != "Never Gonna Give You Up" {
		t.Errorf("expected track title Never Gonna Give You Up, got %s", track.Title)
	}
	if track.Duration != 213*time.Second {
		t.Errorf("expected duration 213s, got %v", track.Duration)
	}
	if track.RequestedBy != "user123" {
		t.Errorf("expected requested by user123, got %s", track.RequestedBy)
	}
	if track.Source != "youtube" {
		t.Errorf("expected source youtube, got %s", track.Source)
	}
}

func TestYtDlpSourceSearchQuery(t *testing.T) {
	mockJSON := `{
		"id": "abc123xyz",
		"title": "Lofi Hip Hop Beat",
		"webpage_url": "https://www.youtube.com/watch?v=abc123xyz",
		"duration": 180.0,
		"thumbnail": "https://example.com/thumb.jpg",
		"extractor": "youtube"
	}`

	runner := &mockCommandRunner{output: []byte(mockJSON)}
	source := NewYtDlpSourceWithRunner("yt-dlp", runner)

	ctx := context.Background()
	res, err := source.Resolve(ctx, "lofi hip hop", "requester")
	if err != nil {
		t.Fatalf("unexpected error resolving search query: %v", err)
	}
	if len(res.Tracks) == 0 {
		t.Fatal("expected at least 1 track in result")
	}
	track := res.Tracks[0]

	// Verify ytsearch1: prefix was used in target argument
	lastArg := runner.args[len(runner.args)-1]
	if lastArg != "ytsearch1:lofi hip hop" {
		t.Errorf("expected target argument 'ytsearch1:lofi hip hop', got '%s'", lastArg)
	}
	if track.Title != "Lofi Hip Hop Beat" {
		t.Errorf("expected title 'Lofi Hip Hop Beat', got '%s'", track.Title)
	}
}

func TestYtDlpSourceErrors(t *testing.T) {
	// 1. Empty query
	source := NewYtDlpSource("yt-dlp")
	_, err := source.Resolve(context.Background(), "   ", "user")
	if !errors.Is(err, ErrEmptyQuery) {
		t.Errorf("expected ErrEmptyQuery, got %v", err)
	}

	// 2. Command execution failure
	runner := &mockCommandRunner{err: errors.New("exit status 1")}
	source = NewYtDlpSourceWithRunner("yt-dlp", runner)
	_, err = source.Resolve(context.Background(), "https://youtube.com/invalid", "user")
	if !errors.Is(err, ErrExtractionFailed) {
		t.Errorf("expected ErrExtractionFailed, got %v", err)
	}

	// 3. Empty output
	runner = &mockCommandRunner{output: []byte("")}
	source = NewYtDlpSourceWithRunner("yt-dlp", runner)
	_, err = source.Resolve(context.Background(), "https://youtube.com/empty", "user")
	if !errors.Is(err, ErrTrackNotFound) {
		t.Errorf("expected ErrTrackNotFound, got %v", err)
	}

	// 4. Invalid JSON
	runner = &mockCommandRunner{output: []byte("invalid json output")}
	source = NewYtDlpSourceWithRunner("yt-dlp", runner)
	_, err = source.Resolve(context.Background(), "https://youtube.com/badjson", "user")
	if !errors.Is(err, ErrExtractionFailed) {
		t.Errorf("expected ErrExtractionFailed on bad JSON, got %v", err)
	}
}

func TestYtDlpSourceContextCancellation(t *testing.T) {
	runner := &mockCommandRunner{}
	source := NewYtDlpSourceWithRunner("yt-dlp", runner)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	_, err := source.Resolve(ctx, "query", "user")
	if err == nil || !strings.Contains(err.Error(), "context canceled") {
		t.Errorf("expected context cancellation error, got %v", err)
	}
}
