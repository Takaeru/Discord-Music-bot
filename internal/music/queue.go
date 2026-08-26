package music

import (
	"errors"
	"sync"
)

var (
	// ErrQueueEmpty is returned when attempting to pop or peek an empty queue.
	ErrQueueEmpty = errors.New("music queue is empty")

	// ErrNilTrack is returned when attempting to add a nil track.
	ErrNilTrack = errors.New("cannot add nil track to queue")
)

// Queue represents a thread-safe FIFO music track queue.
type Queue struct {
	mu    sync.RWMutex
	items []*Track
}

// NewQueue creates an initialized, empty Queue.
func NewQueue() *Queue {
	return &Queue{
		items: make([]*Track, 0),
	}
}

// Add appends a track to the end of the queue in FIFO order.
func (q *Queue) Add(track *Track) error {
	if track == nil {
		return ErrNilTrack
	}

	q.mu.Lock()
	defer q.mu.Unlock()

	q.items = append(q.items, track)
	return nil
}

// Next pops and returns the next track from the front of the queue.
// Returns ErrQueueEmpty if there are no tracks in the queue.
func (q *Queue) Next() (*Track, error) {
	q.mu.Lock()
	defer q.mu.Unlock()

	if len(q.items) == 0 {
		return nil, ErrQueueEmpty
	}

	track := q.items[0]
	q.items = q.items[1:]
	return track, nil
}

// Peek returns the track at the front of the queue without removing it.
// Returns ErrQueueEmpty if the queue is empty.
func (q *Queue) Peek() (*Track, error) {
	q.mu.RLock()
	defer q.mu.RUnlock()

	if len(q.items) == 0 {
		return nil, ErrQueueEmpty
	}

	return q.items[0], nil
}

// Clear removes all tracks from the queue.
func (q *Queue) Clear() {
	q.mu.Lock()
	defer q.mu.Unlock()

	q.items = make([]*Track, 0)
}

// List returns a safe snapshot copy of all tracks currently queued.
func (q *Queue) List() []*Track {
	q.mu.RLock()
	defer q.mu.RUnlock()

	result := make([]*Track, len(q.items))
	copy(result, q.items)
	return result
}

// Length returns the current number of tracks in the queue.
func (q *Queue) Length() int {
	q.mu.RLock()
	defer q.mu.RUnlock()

	return len(q.items)
}
