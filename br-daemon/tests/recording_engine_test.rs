//! Recording Engine Tests
//!
//! Tests for the recording engine, HLS parsing, segment handling, and state management.

use br_daemon::recording::{
    HlsSegment, QueuedSegment, RecordingEvent, RecordingState, SegmentPriority,
};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use tempfile::TempDir;

mod common;

/**
 * Segment Priority Tests
 */

#[test]
fn test_segment_priority_high_before_normal() {
    let high = QueuedSegment {
        segment: HlsSegment {
            sequence: 1,
            uri: "http://example.com/seg1.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    };

    let normal = QueuedSegment {
        segment: HlsSegment {
            sequence: 2,
            uri: "http://example.com/seg2.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    };

    // High priority should be "greater" (processed first in max-heap)
    assert_eq!(high.cmp(&normal), Ordering::Greater);
    assert_eq!(normal.cmp(&high), Ordering::Less);
}

#[test]
fn test_segment_priority_same_priority_by_sequence() {
    let seg1 = QueuedSegment {
        segment: HlsSegment {
            sequence: 100,
            uri: "http://example.com/seg100.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    };

    let seg2 = QueuedSegment {
        segment: HlsSegment {
            sequence: 105,
            uri: "http://example.com/seg105.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    };

    // Higher sequence number should be "greater" (more recent = process first)
    assert_eq!(seg2.cmp(&seg1), Ordering::Greater);
    assert_eq!(seg1.cmp(&seg2), Ordering::Less);
}

#[test]
fn test_segment_priority_heap_ordering() {
    let mut heap = BinaryHeap::new();

    // Add segments in random order
    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 101,
            uri: "http://example.com/seg101.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    });

    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 105,
            uri: "http://example.com/seg105.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High, // Live edge - should come first
    });

    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 103,
            uri: "http://example.com/seg103.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    });

    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 102,
            uri: "http://example.com/seg102.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    });

    // Pop order should be: High(105), Normal(103), Normal(102), Normal(101)
    let first = heap.pop().expect("Should have element");
    assert_eq!(first.priority, SegmentPriority::High);
    assert_eq!(first.segment.sequence, 105);

    let second = heap.pop().expect("Should have element");
    assert_eq!(second.priority, SegmentPriority::Normal);
    assert_eq!(second.segment.sequence, 103);

    let third = heap.pop().expect("Should have element");
    assert_eq!(third.priority, SegmentPriority::Normal);
    assert_eq!(third.segment.sequence, 102);

    let fourth = heap.pop().expect("Should have element");
    assert_eq!(fourth.priority, SegmentPriority::Normal);
    assert_eq!(fourth.segment.sequence, 101);
}

#[test]
fn test_queued_segment_equality_by_sequence() {
    let seg1 = QueuedSegment {
        segment: HlsSegment {
            sequence: 100,
            uri: "http://example.com/seg100.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    };

    let seg2 = QueuedSegment {
        segment: HlsSegment {
            sequence: 100,
            uri: "http://different.com/different.ts".to_string(), // Different URI
            duration: 5.0,                                        // Different duration
        },
        priority: SegmentPriority::Normal, // Different priority
    };

    // Same sequence = equal (for deduplication purposes)
    assert_eq!(seg1, seg2);
}

#[test]
fn test_queued_segment_inequality() {
    let seg1 = QueuedSegment {
        segment: HlsSegment {
            sequence: 100,
            uri: "http://example.com/seg100.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    };

    let seg2 = QueuedSegment {
        segment: HlsSegment {
            sequence: 101,
            uri: "http://example.com/seg101.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    };

    // Different sequence = not equal
    assert_ne!(seg1, seg2);
}

/**
 * Recording State Tests
 */

#[test]
fn test_recording_state_new() {
    let state = RecordingState::new("streamer123", "twitch", "1080p60");

    assert_eq!(state.channel, "streamer123");
    assert_eq!(state.platform, "twitch");
    assert_eq!(state.quality, "1080p60");
    assert_eq!(state.last_segment, 0);
    assert_eq!(state.segments_downloaded, 0);
    assert_eq!(state.bytes_downloaded, 0);
}

#[tokio::test]
async fn test_recording_state_save_and_load() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("state.json");

    // Create and save state
    let mut state = RecordingState::new("test_channel", "youtube", "720p");
    state.last_segment = 12345;
    state.segments_downloaded = 100;
    state.bytes_downloaded = 50_000_000;

    state.save(&state_path).await.expect("Failed to save state");

    // Load state back
    let loaded = RecordingState::load(&state_path)
        .await
        .expect("Failed to load state");

    assert_eq!(loaded.channel, "test_channel");
    assert_eq!(loaded.platform, "youtube");
    assert_eq!(loaded.quality, "720p");
    assert_eq!(loaded.last_segment, 12345);
    assert_eq!(loaded.segments_downloaded, 100);
    assert_eq!(loaded.bytes_downloaded, 50_000_000);
}

#[tokio::test]
async fn test_recording_state_load_missing_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("nonexistent.json");

    let result = RecordingState::load(&state_path).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_recording_state_load_invalid_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("invalid.json");

    // Write invalid JSON
    tokio::fs::write(&state_path, "not valid json {{{")
        .await
        .expect("Failed to write test file");

    let result = RecordingState::load(&state_path).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_recording_state_atomic_save() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("state.json");
    let temp_path = temp_dir.path().join("state.json.tmp");

    let state = RecordingState::new("channel", "kick", "source");
    state.save(&state_path).await.expect("Failed to save state");

    // Temp file should be cleaned up after save
    assert!(!temp_path.exists(), "Temp file should be deleted");
    assert!(state_path.exists(), "State file should exist");
}

/**
 * Recording Event Tests
 */

#[test]
fn test_recording_event_debug_format() {
    let events = vec![
        RecordingEvent::InitSegmentDownloaded { size_bytes: 1024 },
        RecordingEvent::SegmentDownloaded {
            sequence: 100,
            size_bytes: 500_000,
        },
        RecordingEvent::PlaylistRefreshed { new_segments: 3 },
        RecordingEvent::StreamEnded,
        RecordingEvent::Error {
            message: "Connection timeout".to_string(),
        },
    ];

    for event in events {
        // Just ensure Debug is implemented and doesn't panic
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }
}

#[test]
fn test_recording_event_clone() {
    let event = RecordingEvent::SegmentDownloaded {
        sequence: 42,
        size_bytes: 1_000_000,
    };

    let cloned = event.clone();

    match (&event, &cloned) {
        (
            RecordingEvent::SegmentDownloaded {
                sequence: s1,
                size_bytes: b1,
            },
            RecordingEvent::SegmentDownloaded {
                sequence: s2,
                size_bytes: b2,
            },
        ) => {
            assert_eq!(s1, s2);
            assert_eq!(b1, b2);
        }
        _ => panic!("Clone should preserve variant"),
    }
}

/**
 * HLS Segment Tests
 */

#[test]
fn test_hls_segment_clone() {
    let segment = HlsSegment {
        sequence: 12345,
        uri: "https://cdn.example.com/stream/seg-12345.ts".to_string(),
        duration: 2.5,
    };

    let cloned = segment.clone();

    assert_eq!(cloned.sequence, 12345);
    assert_eq!(cloned.uri, "https://cdn.example.com/stream/seg-12345.ts");
    assert!((cloned.duration - 2.5).abs() < f32::EPSILON);
}

#[test]
fn test_hls_segment_debug() {
    let segment = HlsSegment {
        sequence: 1,
        uri: "test.ts".to_string(),
        duration: 2.0,
    };

    let debug_str = format!("{:?}", segment);
    assert!(debug_str.contains("sequence"));
    assert!(debug_str.contains("1"));
    assert!(debug_str.contains("test.ts"));
}

/**
 * Integration Test: Priority Queue Behavior Under Load
 */

#[test]
fn test_priority_queue_mixed_priorities() {
    let mut heap = BinaryHeap::new();

    // Simulate a realistic scenario:
    // - Initial backfill of segments 1-5 (Normal priority)
    // - Live edge at segment 10 (High priority)
    // - Then more backfill 6-9 (Normal priority)

    // Backfill batch 1
    for seq in 1..=5 {
        heap.push(QueuedSegment {
            segment: HlsSegment {
                sequence: seq,
                uri: format!("http://example.com/seg{}.ts", seq),
                duration: 2.0,
            },
            priority: SegmentPriority::Normal,
        });
    }

    // Live edge segment arrives
    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 10,
            uri: "http://example.com/seg10.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    });

    // More backfill
    for seq in 6..=9 {
        heap.push(QueuedSegment {
            segment: HlsSegment {
                sequence: seq,
                uri: format!("http://example.com/seg{}.ts", seq),
                duration: 2.0,
            },
            priority: SegmentPriority::Normal,
        });
    }

    // First should be the high-priority live edge
    let first = heap.pop().expect("Should have element");
    assert_eq!(first.segment.sequence, 10);
    assert_eq!(first.priority, SegmentPriority::High);

    // Remaining should be Normal, ordered by sequence descending
    let mut remaining: Vec<_> = heap.into_iter().collect();
    remaining.sort_by(|a, b| b.cmp(a)); // Max-heap order

    let sequences: Vec<u64> = remaining.iter().map(|q| q.segment.sequence).collect();
    assert_eq!(sequences, vec![9, 8, 7, 6, 5, 4, 3, 2, 1]);
}

#[test]
fn test_priority_queue_multiple_high_priority() {
    let mut heap = BinaryHeap::new();

    // Two high-priority segments (could happen with burst of live edge updates)
    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 100,
            uri: "seg100.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    });

    heap.push(QueuedSegment {
        segment: HlsSegment {
            sequence: 101,
            uri: "seg101.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::High,
    });

    // Higher sequence should come first among high priority
    let first = heap.pop().expect("Should have element");
    assert_eq!(first.segment.sequence, 101);

    let second = heap.pop().expect("Should have element");
    assert_eq!(second.segment.sequence, 100);
}

/**
 * Edge Case Tests
 */

#[test]
fn test_segment_priority_equal_sequence_same_priority() {
    let seg1 = QueuedSegment {
        segment: HlsSegment {
            sequence: 50,
            uri: "a.ts".to_string(),
            duration: 2.0,
        },
        priority: SegmentPriority::Normal,
    };

    let seg2 = QueuedSegment {
        segment: HlsSegment {
            sequence: 50,
            uri: "b.ts".to_string(),
            duration: 3.0,
        },
        priority: SegmentPriority::Normal,
    };

    // Same sequence and priority = equal ordering
    assert_eq!(seg1.cmp(&seg2), Ordering::Equal);
}

#[tokio::test]
async fn test_recording_state_preserves_started_at() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("state.json");

    let state = RecordingState::new("channel", "twitch", "1080p");
    let original_started_at = state.started_at;

    state.save(&state_path).await.expect("Failed to save");

    let loaded = RecordingState::load(&state_path)
        .await
        .expect("Failed to load");

    assert_eq!(loaded.started_at, original_started_at);
}

#[test]
fn test_segment_priority_derives_copy() {
    let priority = SegmentPriority::High;
    let copied = priority; // Copy, not move

    // Both should work since SegmentPriority is Copy
    assert_eq!(priority, SegmentPriority::High);
    assert_eq!(copied, SegmentPriority::High);
}
