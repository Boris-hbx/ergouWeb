//! `/api/events/*` — user-side behavior analytics ingest (T-218 / SPEC analytics).
//!
//! Any active user reports *their own* events. `user_id` is taken from the
//! `ActiveUserId` guard and never trusted from the client. Ingest is best-effort:
//! a write failure is logged but still returns 200 so analytics can never break
//! the main UX.

use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use crate::auth::ActiveUserId;
use crate::state::AppState;

/// Allowed `event_type` values; anything else is dropped from the batch.
const ALLOWED_EVENT_TYPES: [&str; 5] = ["pageview", "click", "dwell", "input", "custom"];
/// Max events accepted in one batch.
const MAX_BATCH: usize = 100;
/// Max serialized `meta` size; larger payloads are stored as NULL.
const MAX_META_BYTES: usize = 2048;
/// Defensive clamp on free-text label length.
const MAX_LABEL_CHARS: usize = 200;

#[derive(Debug, Deserialize)]
pub struct EventBatchRequest {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub events: Vec<EventIn>,
}

#[derive(Debug, Deserialize)]
pub struct EventIn {
    pub event_type: String,
    pub target_id: Option<String>,
    pub target_label: Option<String>,
    pub route: Option<String>,
    pub dwell_ms: Option<i64>,
    pub meta: Option<JsonValue>,
    pub client_ts: String,
}

fn clamp_chars(s: String, max: usize) -> String {
    if s.chars().count() > max {
        s.chars().take(max).collect()
    } else {
        s
    }
}

/// Serialize `meta` to a JSON string, dropping it (NULL) if it exceeds the cap.
fn normalize_meta(meta: Option<JsonValue>) -> Option<String> {
    let value = meta?;
    let s = serde_json::to_string(&value).ok()?;
    if s.len() > MAX_META_BYTES || s == "null" {
        None
    } else {
        Some(s)
    }
}

/// POST /api/events/batch — ingest a batch of behavior events for the caller.
pub async fn events_batch(
    State(state): State<AppState>,
    user: ActiveUserId,
    Json(req): Json<EventBatchRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let session_id = req.session_id.trim().to_string();
    if session_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "session_id 不能为空" })),
        );
    }
    if req.events.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "events 不能为空" })),
        );
    }
    if req.events.len() > MAX_BATCH {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "单批 events 不能超过 100 条" })),
        );
    }

    let now = Utc::now().to_rfc3339();
    let mut db = state.db.lock();
    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => {
            // Best-effort: never fail the client because analytics couldn't write.
            eprintln!("[events] begin tx failed: {e}");
            return (
                StatusCode::OK,
                Json(json!({ "success": true, "accepted": 0 })),
            );
        }
    };

    let mut accepted = 0usize;
    for ev in req.events {
        let event_type = ev.event_type.trim();
        if !ALLOWED_EVENT_TYPES.contains(&event_type) {
            continue; // drop unknown event types, keep the rest
        }
        if ev.client_ts.trim().is_empty() {
            continue;
        }
        let target_label = ev.target_label.map(|s| clamp_chars(s, MAX_LABEL_CHARS));
        let meta = normalize_meta(ev.meta);
        let id = uuid::Uuid::new_v4().to_string();
        let res = tx.execute(
            "INSERT INTO behavior_events
                (id, user_id, session_id, event_type, target_id, target_label, route, dwell_ms, meta, client_ts, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                id,
                user.0,
                session_id,
                event_type,
                ev.target_id,
                target_label,
                ev.route,
                ev.dwell_ms,
                meta,
                ev.client_ts,
                now,
            ],
        );
        match res {
            Ok(_) => accepted += 1,
            Err(e) => eprintln!("[events] insert failed: {e}"),
        }
    }

    if let Err(e) = tx.commit() {
        eprintln!("[events] commit failed: {e}");
        return (
            StatusCode::OK,
            Json(json!({ "success": true, "accepted": 0 })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({ "success": true, "accepted": accepted })),
    )
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{auth_cookie, create_admin_user, create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde_json::Value as JsonValue;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let body = to_bytes(resp.into_body(), 2_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn post(uri: &str, cookie: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("Cookie", cookie)
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn batch_ingests_valid_events_and_takes_user_from_guard() {
        let state = test_state();
        let (user_id, token) = create_test_user(&state, "ev-user", "Pa55word1");
        let app = crate::build_app(state.clone());

        // 3 events: 1 click, 1 pageview, 1 bogus type (must be dropped).
        let resp = app
            .oneshot(post(
                "/api/events/batch",
                &auth_cookie(&token),
                r#"{"session_id":"s1","events":[
                    {"event_type":"click","target_id":"expense.add","target_label":"记一笔","route":"expenses","client_ts":"2026-06-17T14:00:00+08:00"},
                    {"event_type":"pageview","route":"expenses","client_ts":"2026-06-17T14:01:00+08:00"},
                    {"event_type":"bogus","client_ts":"2026-06-17T14:02:00+08:00"}
                ]}"#,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let j = body_json(resp).await;
        assert_eq!(j["accepted"], 2, "bogus event_type must be dropped");

        let db = state.db.lock();
        let (count, owner): (i64, String) = db
            .query_row(
                "SELECT COUNT(*), MAX(user_id) FROM behavior_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(owner, user_id, "user_id must come from the guard");
    }

    #[tokio::test]
    async fn batch_rejects_empty_session() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "ev-empty", "Pa55word1");
        let app = crate::build_app(state);
        let resp = app
            .oneshot(post(
                "/api/events/batch",
                &auth_cookie(&token),
                r#"{"session_id":"","events":[{"event_type":"click","client_ts":"2026-06-17T14:00:00+08:00"}]}"#,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn admin_analytics_aggregates_and_requires_admin() {
        let state = test_state();
        let (_u, user_token) = create_test_user(&state, "ev-actor", "Pa55word1");
        let (_a, admin_token) = create_admin_user(&state, "ev-admin", "Pa55word1");
        let app = crate::build_app(state.clone());

        // Seed events via the ingest endpoint as the normal user.
        let resp = app
            .clone()
            .oneshot(post(
                "/api/events/batch",
                &auth_cookie(&user_token),
                r#"{"session_id":"s9","events":[
                    {"event_type":"click","target_id":"expense.add","target_label":"记一笔","route":"expenses","client_ts":"2026-06-17T09:00:00+08:00"},
                    {"event_type":"pageview","route":"expenses","client_ts":"2026-06-17T09:00:05+08:00"},
                    {"event_type":"dwell","route":"expenses","dwell_ms":4200,"client_ts":"2026-06-17T09:02:00+08:00"}
                ]}"#,
            ))
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["accepted"], 3);

        // Non-admin is forbidden.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/analytics/overview")
                    .header("Cookie", auth_cookie(&user_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Admin overview aggregates the seeded events.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/analytics/overview")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["total_events"], 3);
        assert_eq!(j["sessions"], 1);
        assert_eq!(j["active_users"], 1);

        // top-targets sees the click; feature-usage sees pageview + dwell.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/analytics/top-targets")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["items"][0]["target_id"], "expense.add");
        assert_eq!(j["items"][0]["clicks"], 1);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/analytics/feature-usage")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["items"][0]["route"], "expenses");
        assert_eq!(j["items"][0]["pageviews"], 1);
        assert_eq!(j["items"][0]["total_dwell_ms"], 4200);
    }
}
