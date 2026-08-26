package music

import (
	"errors"
	"fmt"
	"sync"
	"testing"
	"time"
)

func TestQueueOperations(t *testing.T) {
	q := NewQueue()

	if l := q.Length(); l != 0 {
		t.Fatalf("expected initial length 0, got %d", l)
	}

	// Edge case: Empty queue operations
	if _, err := q.Next(); !errors.Is(err, ErrQueueEmpty) {
		t.Fatalf("expected ErrQueueEmpty on Next(), got %v", err)
	}
	if _, err := q.Peek(); !errors.Is(err, ErrQueueEmpty) {
		t.Fatalf("expected ErrQueueEmpty on Peek(), got %v", err)
	}

	// Edge case: Nil track addition
	if err := q.Add(nil); !errors.Is(err, ErrNilTrack) {
		t.Fatalf("expected ErrNilTrack on Add(nil), got %v", err)
	}

	track1 := &Track{ID: "1", Title: "Track 1", URL: "https://example.com/1", Duration: 3 * time.Minute}
	track2 := &Track{ID: "2", Title: "Track 2", URL: "https://example.com/2", Duration: 4 * time.Minute}
	track3 := &Track{ID: "3", Title: "Track 3", URL: "https://example.com/3", Duration: 5 * time.Minute}

	// Add tracks
	if err := q.Add(track1); err != nil {
		t.Fatalf("failed to add track1: %v", err)
	}
	if err := q.Add(track2); err != nil {
		t.Fatalf("failed to add track2: %v", err)
	}
	if err := q.Add(track3); err != nil {
		t.Fatalf("failed to add track3: %v", err)
	}

	if l := q.Length(); l != 3 {
		t.Fatalf("expected length 3, got %d", l)
	}

	// Peek check
	peeked, err := q.Peek()
	if err != nil {
		t.Fatalf("unexpected error on Peek: %v", err)
	}
	if peeked.ID != "1" {
		t.Fatalf("expected peeked track ID '1', got '%s'", peeked.ID)
	}
	if l := q.Length(); l != 3 {
		t.Fatalf("expected length 3 after Peek, got %d", l)
	}

	// List check & ensure copy immutability
	list := q.List()
	if len(list) != 3 {
		t.Fatalf("expected list length 3, got %d", len(list))
	}
	list[0] = nil
	if listCheck := q.List(); listCheck[0] == nil {
		t.Fatal("mutating returned List() affected internal queue slice")
	}

	// FIFO Next checks
	next1, err := q.Next()
	if err != nil || next1.ID != "1" {
		t.Fatalf("expected track 1, got %v (err: %v)", next1, err)
	}

	next2, err := q.Next()
	if err != nil || next2.ID != "2" {
		t.Fatalf("expected track 2, got %v (err: %v)", next2, err)
	}

	if l := q.Length(); l != 1 {
		t.Fatalf("expected length 1, got %d", l)
	}

	// Clear check
	q.Clear()
	if l := q.Length(); l != 0 {
		t.Fatalf("expected length 0 after Clear, got %d", l)
	}
	if _, err := q.Next(); !errors.Is(err, ErrQueueEmpty) {
		t.Fatalf("expected ErrQueueEmpty after Clear, got %v", err)
	}
}

func TestQueueConcurrency(t *testing.T) {
	q := NewQueue()
	numWorkers := 20
	itemsPerWorker := 50

	var wg sync.WaitGroup

	// Concurrently add tracks
	for w := 0; w < numWorkers; w++ {
		wg.Add(1)
		go func(workerID int) {
			defer wg.Done()
			for i := 0; i < itemsPerWorker; i++ {
				track := &Track{
					ID:    fmt.Sprintf("w%d-i%d", workerID, i),
					Title: fmt.Sprintf("Worker %d Track %d", workerID, i),
				}
				_ = q.Add(track)
			}
		}(w)
	}

	wg.Wait()

	totalExpected := numWorkers * itemsPerWorker
	if l := q.Length(); l != totalExpected {
		t.Fatalf("expected length %d, got %d", totalExpected, l)
	}

	// Concurrently pop tracks, read peek and list
	var popWg sync.WaitGroup
	poppedCount := 0
	var popMu sync.Mutex

	for w := 0; w < numWorkers; w++ {
		popWg.Add(1)
		go func() {
			defer popWg.Done()
			for {
				// Interleave reads
				_ = q.Length()
				_ = q.List()
				_, _ = q.Peek()

				track, err := q.Next()
				if errors.Is(err, ErrQueueEmpty) {
					break
				}
				if track != nil {
					popMu.Lock()
					poppedCount++
					popMu.Unlock()
				}
			}
		}()
	}

	popWg.Wait()

	if poppedCount != totalExpected {
		t.Fatalf("expected %d total popped tracks, got %d", totalExpected, poppedCount)
	}
	if l := q.Length(); l != 0 {
		t.Fatalf("expected empty queue at the end, got length %d", l)
	}
}
