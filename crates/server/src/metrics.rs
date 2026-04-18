//! Prometheus metrics registry for oxpulse-chat.
//!
//! One [`Metrics`] instance per process, wrapped in [`Arc`], held by
//! [`crate::router::AppState`]. Handlers and background tasks reach
//! into it by cloning the Arc and calling `inc()`/`observe()`/`set()`.
//!
//! The registry is exposed via `/metrics` (token-gated) using the
//! Prometheus text format 0.0.4, per Task 3.1 of the phase-2 plan.

use prometheus::{Histogram, HistogramOpts, IntCounter, IntCounterVec, IntGauge, Opts, Registry};

pub struct Metrics {
    pub registry: Registry,
    pub rooms_active: IntGauge,
    pub ws_connects_total: IntCounter,
    pub ws_disconnects_total: IntCounter,
    /// label: result=ok|err
    pub ws_join_total: IntCounterVec,
    pub ws_handshake_failed_total: IntCounter,
    pub call_duration_seconds: Histogram,
    pub turn_servers_healthy: IntGauge,
    pub turn_creds_issued_total: IntCounter,
    /// SLO target p99 < 150 ms.
    pub turn_cred_latency_seconds: Histogram,
    /// label: result=ok|err
    pub analytics_events_total: IntCounterVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        macro_rules! reg {
            ($m:expr) => {{
                let m = $m;
                registry
                    .register(Box::new(m.clone()))
                    .expect("metric registration");
                m
            }};
        }

        let rooms_active = reg!(IntGauge::with_opts(Opts::new(
            "rooms_active",
            "Currently active signaling rooms",
        ))
        .unwrap());
        let ws_connects_total = reg!(IntCounter::with_opts(Opts::new(
            "ws_connects_total",
            "Total WebSocket connections accepted",
        ))
        .unwrap());
        let ws_disconnects_total = reg!(IntCounter::with_opts(Opts::new(
            "ws_disconnects_total",
            "Total WebSocket connections closed",
        ))
        .unwrap());
        let ws_join_total = reg!(IntCounterVec::new(
            Opts::new("ws_join_total", "Room join attempts by result"),
            &["result"],
        )
        .unwrap());
        let ws_handshake_failed_total = reg!(IntCounter::with_opts(Opts::new(
            "ws_handshake_failed_total",
            "WS transport/handshake failures before join",
        ))
        .unwrap());
        let call_duration_seconds = reg!(Histogram::with_opts(
            HistogramOpts::new("call_duration_seconds", "Duration of completed calls")
                .buckets(vec![5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0])
        )
        .unwrap());
        let turn_servers_healthy = reg!(IntGauge::with_opts(Opts::new(
            "turn_servers_healthy",
            "Number of TURN servers currently healthy",
        ))
        .unwrap());
        let turn_creds_issued_total = reg!(IntCounter::with_opts(Opts::new(
            "turn_creds_issued_total",
            "Total TURN credential responses issued",
        ))
        .unwrap());
        let turn_cred_latency_seconds = reg!(Histogram::with_opts(
            HistogramOpts::new(
                "turn_cred_latency_seconds",
                "Latency of /api/turn-credentials handler (p99 SLO target: 150ms)",
            )
            .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.15, 0.25, 0.5, 1.0, 2.0])
        )
        .unwrap());
        let analytics_events_total = reg!(IntCounterVec::new(
            Opts::new("analytics_events_total", "Analytics insert results by outcome"),
            &["result"],
        )
        .unwrap());

        Self {
            registry,
            rooms_active,
            ws_connects_total,
            ws_disconnects_total,
            ws_join_total,
            ws_handshake_failed_total,
            call_duration_seconds,
            turn_servers_healthy,
            turn_creds_issued_total,
            turn_cred_latency_seconds,
            analytics_events_total,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}



impl oxpulse_signaling::SignalingMetrics for Metrics {
    fn on_ws_connect(&self) {
        self.ws_connects_total.inc();
    }
    fn on_ws_disconnect(&self) {
        self.ws_disconnects_total.inc();
    }
    fn on_ws_join_ok(&self) {
        self.ws_join_total.with_label_values(&["ok"]).inc();
    }
    fn on_ws_join_err(&self) {
        self.ws_join_total.with_label_values(&["err"]).inc();
    }
    fn on_ws_handshake_failed(&self) {
        self.ws_handshake_failed_total.inc();
    }
    fn on_call_ended(&self, secs: f64) {
        self.call_duration_seconds.observe(secs);
    }
    fn on_room_opened(&self) {
        self.rooms_active.inc();
    }
    fn on_room_closed(&self) {
        self.rooms_active.dec();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Encoder;

    #[test]
    fn registry_contains_all_metrics() {
        let m = Metrics::new();
        m.rooms_active.inc();
        m.ws_join_total.with_label_values(&["ok"]).inc();
        m.analytics_events_total.with_label_values(&["err"]).inc();

        let mut buf = Vec::new();
        prometheus::TextEncoder::new()
            .encode(&m.registry.gather(), &mut buf)
            .unwrap();
        let out = String::from_utf8(buf).unwrap();
        for name in [
            "rooms_active",
            "ws_connects_total",
            "ws_disconnects_total",
            "ws_join_total",
            "ws_handshake_failed_total",
            "call_duration_seconds",
            "turn_servers_healthy",
            "turn_creds_issued_total",
            "turn_cred_latency_seconds",
            "analytics_events_total",
        ] {
            assert!(
                out.contains(name),
                "exposition missing metric {name}: {out}"
            );
        }
    }
}

#[cfg(test)]
mod signaling_impl_tests {
    use super::*;
    use oxpulse_signaling::SignalingMetrics;

    #[test]
    fn on_ws_connect_increments_counter() {
        let m = Metrics::new();
        let before = m.ws_connects_total.get();
        m.on_ws_connect();
        assert_eq!(m.ws_connects_total.get(), before + 1);
    }

    #[test]
    fn on_ws_join_ok_and_err_have_separate_series() {
        let m = Metrics::new();
        m.on_ws_join_ok();
        m.on_ws_join_ok();
        m.on_ws_join_err();
        assert_eq!(m.ws_join_total.with_label_values(&["ok"]).get(), 2);
        assert_eq!(m.ws_join_total.with_label_values(&["err"]).get(), 1);
    }

    #[test]
    fn on_room_opened_and_closed_balance() {
        let m = Metrics::new();
        m.on_room_opened();
        m.on_room_opened();
        m.on_room_closed();
        assert_eq!(m.rooms_active.get(), 1);
    }

    #[test]
    fn on_call_ended_records_duration() {
        let m = Metrics::new();
        m.on_call_ended(42.5);
        assert_eq!(m.call_duration_seconds.get_sample_count(), 1);
        assert!((m.call_duration_seconds.get_sample_sum() - 42.5).abs() < 1e-9);
    }
}
