use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::room_cleanup::{now_secs, start_cleanup_task};

const MAX_PARTICIPANTS: u8 = 2;
const CHANNEL_CAPACITY: usize = 64;

/// A broadcast message tagged with the sender's peer ID.
#[derive(Clone, Debug)]
pub struct TaggedSignal {
    pub from: u64,
    pub payload: String,
}

/// A single call room with a broadcast channel and participant counter.
pub struct Room {
    pub tx: broadcast::Sender<TaggedSignal>,
    pub count: AtomicU8,
    next_peer_id: AtomicU64,
    pub connected_at: AtomicI64,
    /// Timestamp when the room became empty (0 = not empty).
    pub empty_since: AtomicI64,
    /// Whether call_ended has already been fired for this room.
    pub ended: AtomicBool,
}

/// Thread-safe room registry backed by DashMap.
#[derive(Clone)]
pub struct Rooms {
    pub(crate) inner: Arc<DashMap<String, Arc<Room>>>,
}

impl Default for Rooms {
    fn default() -> Self {
        Self::new()
    }
}

impl Rooms {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Start a background task that removes rooms empty for longer than the grace period.
    pub fn start_cleanup_task(&self) {
        start_cleanup_task(Arc::clone(&self.inner));
    }

    /// Join a room by ID. Returns `(sender, polite, peer_id)` or `None` if the room is full.
    ///
    /// The first participant gets `polite = false`, the second gets `polite = true`.
    pub fn join(&self, room_id: &str) -> Option<(broadcast::Sender<TaggedSignal>, bool, u64)> {
        let room = self
            .inner
            .entry(room_id.to_owned())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
                Arc::new(Room {
                    tx,
                    count: AtomicU8::new(0),
                    next_peer_id: AtomicU64::new(1),
                    connected_at: AtomicI64::new(0),
                    empty_since: AtomicI64::new(0),
                    ended: AtomicBool::new(false),
                })
            })
            .clone();

        let prev = room.count.fetch_add(1, Ordering::SeqCst);
        if prev >= MAX_PARTICIPANTS {
            room.count.fetch_sub(1, Ordering::SeqCst);
            return None;
        }

        // Clear empty_since — room is no longer empty
        room.empty_since.store(0, Ordering::Relaxed);

        let peer_id = room.next_peer_id.fetch_add(1, Ordering::Relaxed);
        let polite = prev > 0;
        Some((room.tx.clone(), polite, peer_id))
    }

    /// Leave a room. If empty, marks the room for deferred cleanup instead of
    /// removing immediately — this allows participants to rejoin within the grace period.
    pub fn leave(&self, room_id: &str) {
        if let Some(room) = self.inner.get(room_id) {
            let prev = room.count.fetch_sub(1, Ordering::SeqCst);
            if prev <= 1 {
                // Room is now empty — mark timestamp for deferred cleanup
                room.empty_since.store(now_secs(), Ordering::Relaxed);
            }
        }
    }

    pub fn mark_connected(&self, room_id: &str) {
        if let Some(room) = self.inner.get(room_id) {
            let now = now_secs();
            room.connected_at
                .compare_exchange(0, now, Ordering::SeqCst, Ordering::Relaxed)
                .ok();
        }
    }

    pub fn connected_at(&self, room_id: &str) -> i64 {
        self.inner
            .get(room_id)
            .map(|r| r.connected_at.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Atomically mark a room as ended. Returns `true` if this was the first
    /// call (i.e., the room was not already ended).
    pub fn try_mark_ended(&self, room_id: &str) -> bool {
        self.inner
            .get(room_id)
            .map(|r| !r.ended.swap(true, Ordering::SeqCst))
            .unwrap_or(false)
    }
}
