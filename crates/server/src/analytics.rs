use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::router::AppState;

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

pub async fn ingest(State(state): State<AppState>, Json(batch): Json<EventBatch>) -> StatusCode {
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

    for event in &batch.events {
        let id = uuid::Uuid::new_v4();
        let res = sqlx::query(
            "INSERT INTO call_events (id, device_id, event_type, room_id, source, data, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, now())",
        )
        .bind(id)
        .bind(&batch.device_id)
        .bind(&event.event_type)
        .bind(&event.room_id)
        .bind(&batch.source)
        .bind(&event.data)
        .execute(pool)
        .await;
        if let Err(e) = res {
            tracing::warn!(error = %e, event_type = %event.event_type, "analytics_insert_failed");
        }
    }

    StatusCode::NO_CONTENT
}
