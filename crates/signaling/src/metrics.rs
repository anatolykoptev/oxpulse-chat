//! Metrics hooks for the signaling crate.
//!
//! Signaling stays dep-free of `prometheus` by exposing a `SignalingMetrics`
//! trait with no-op defaults. The server crate (`oxpulse-chat`) implements
//! this trait over its `prometheus::Registry`-backed `Metrics`.
//!
//! This keeps the signaling crate light and testable in isolation (tests
//! use [`NoMetrics`]), while the production server sees real Prometheus
//! counters without the crates depending on each other's internals.

/// Callbacks invoked by the signaling hot paths. Each method has a no-op
/// default so a crate (or test) that doesn't care about metrics can use
/// [`NoMetrics`] and pay zero cost.
pub trait SignalingMetrics: Send + Sync {
    /// Fired at the start of `handle_call_ws` after socket upgrade.
    fn on_ws_connect(&self) {}

    /// Fired at the end of `handle_call_ws` — for every disconnect
    /// (clean close, error, or drop).
    fn on_ws_disconnect(&self) {}

    /// Fired once per successful `Joined` ACK sent to the client.
    fn on_ws_join_ok(&self) {}

    /// Fired on every terminal failure path that *didn't* reach `Joined`
    /// (join timeout, room-full, sink.send failure before `Joined`).
    fn on_ws_join_err(&self) {}

    /// Fired on WS handshake/transport failures before `Joined` —
    /// denominator for the `signaling_ws_availability` SLI.
    fn on_ws_handshake_failed(&self) {}

    /// Fired once per call that reached `connected` state with a real
    /// duration. `secs` is the observed duration, seconds.
    fn on_call_ended(&self, _secs: f64) {}

    /// Fired when `Rooms::join` successfully opens a new room entry
    /// (first joiner of a brand-new room — not repeat joiners).
    fn on_room_opened(&self) {}

    /// Fired when the deferred cleanup task drops an empty room entry.
    fn on_room_closed(&self) {}
}

/// Zero-cost no-op implementation. Used in tests and when a caller
/// opts out of metrics.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMetrics;

impl SignalingMetrics for NoMetrics {}
