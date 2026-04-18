use std::sync::atomic::Ordering;
use std::sync::Arc;

use dashmap::DashMap;

use crate::metrics::SignalingMetrics;
use crate::rooms::Room;

/// How long an empty room stays alive before cleanup (seconds).
pub(crate) const ROOM_GRACE_PERIOD_SECS: i64 = 600; // 10 minutes

/// How often the cleanup task runs (seconds).
pub(crate) const CLEANUP_INTERVAL_SECS: u64 = 60;

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Start a background task that removes rooms empty for longer than the grace period.
pub(crate) fn start_cleanup_task(
    inner: Arc<DashMap<String, Arc<Room>>>,
    metrics: Arc<dyn SignalingMetrics>,
) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(CLEANUP_INTERVAL_SECS));
        loop {
            interval.tick().await;
            cleanup_expired(&inner, &*metrics);
        }
    });
}

/// Remove rooms that have been empty longer than the grace period.
pub(crate) fn cleanup_expired(inner: &DashMap<String, Arc<Room>>, metrics: &dyn SignalingMetrics) {
    let now = now_secs();
    let mut to_remove = Vec::new();
    for entry in inner.iter() {
        let empty_since = entry.value().empty_since.load(Ordering::Relaxed);
        if empty_since > 0 && (now - empty_since) > ROOM_GRACE_PERIOD_SECS {
            to_remove.push(entry.key().clone());
        }
    }
    for key in to_remove {
        // Double-check: only remove if still empty (no one rejoined)
        if let Some(room) = inner.get(&key) {
            if room.count.load(Ordering::SeqCst) == 0 {
                drop(room);
                if inner.remove(&key).is_some() {
                    metrics.on_room_closed();
                }
                tracing::debug!(room_id = %key, "room_expired");
            }
        }
    }
}
