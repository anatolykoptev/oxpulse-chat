use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use crate::router::AppState;

/// Cookie name set by the visitor middleware — keep in lockstep with
/// crates/server/src/visitor.rs::COOKIE_NAME.
const VISITOR_COOKIE: &str = "ox_vid";

#[derive(Deserialize)]
pub struct EventBatch {
    #[serde(rename = "did")]
    pub device_id: String,
    #[serde(rename = "src", default)]
    pub source: String,
    pub events: Vec<Event>,
}

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "r")]
    pub room_id: Option<String>,
    #[serde(rename = "d", default)]
    pub data: serde_json::Value,
}

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(batch): Json<EventBatch>,
) -> StatusCode {
    let pool = match &state.pool {
        Some(p) => p,
        None => return StatusCode::NO_CONTENT,
    };

    if batch.device_id.is_empty() || batch.device_id.len() > 64 {
        return StatusCode::BAD_REQUEST;
    }
    if batch.events.is_empty() || batch.events.len() > 20 {
        return StatusCode::BAD_REQUEST;
    }

    // Pull the server-set visitor identity cookie. Always optional —
    // first-time visitors on their very first request may not have it
    // yet (cookie was set in the same response that their SPA shell
    // loaded), and privacy-paranoid clients may block it outright.
    let visitor_id = extract_visitor_cookie(&headers);

    for event in &batch.events {
        let id = uuid::Uuid::new_v4();
        let res = sqlx::query(
            "INSERT INTO call_events (id, device_id, event_type, room_id, source, data, visitor_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, now())",
        )
        .bind(id)
        .bind(&batch.device_id)
        .bind(&event.event_type)
        .bind(&event.room_id)
        .bind(&batch.source)
        .bind(&event.data)
        .bind(&visitor_id)
        .execute(pool)
        .await;
        match res {
            Ok(_) => {
                state.metrics.analytics_events_total.with_label_values(&["ok"]).inc();
            }
            Err(e) => {
                tracing::warn!(error = %e, event_type = %event.event_type, "analytics_insert_failed");
                state.metrics.analytics_events_total.with_label_values(&["err"]).inc();
            }
        }
    }

    StatusCode::NO_CONTENT
}

/// Parse the Cookie header and return the ox_vid value if present.
/// Defensive: silently drops malformed UTF-8 and empty values.
fn extract_visitor_cookie(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        let eq = part.find('=')?;
        if &part[..eq] == VISITOR_COOKIE {
            let value = &part[eq + 1..];
            if !value.is_empty() && value.len() <= 64 {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_visitor_cookie_single() {
        let mut h = HeaderMap::new();
        h.insert(axum::http::header::COOKIE, "ox_vid=abc-123".parse().unwrap());
        assert_eq!(extract_visitor_cookie(&h).as_deref(), Some("abc-123"));
    }

    #[test]
    fn extract_visitor_cookie_multi() {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::COOKIE,
            "sess=x; ox_vid=uuid-v4-here; other=z".parse().unwrap(),
        );
        assert_eq!(extract_visitor_cookie(&h).as_deref(), Some("uuid-v4-here"));
    }

    #[test]
    fn extract_visitor_cookie_missing() {
        let mut h = HeaderMap::new();
        h.insert(axum::http::header::COOKIE, "sess=x; other=z".parse().unwrap());
        assert_eq!(extract_visitor_cookie(&h), None);
    }

    #[test]
    fn extract_visitor_cookie_empty_value_ignored() {
        let mut h = HeaderMap::new();
        h.insert(axum::http::header::COOKIE, "ox_vid=".parse().unwrap());
        assert_eq!(extract_visitor_cookie(&h), None);
    }

    #[test]
    fn extract_visitor_cookie_too_long_ignored() {
        let mut h = HeaderMap::new();
        let long = "a".repeat(128);
        h.insert(
            axum::http::header::COOKIE,
            format!("ox_vid={long}").parse().unwrap(),
        );
        assert_eq!(extract_visitor_cookie(&h), None);
    }
}
