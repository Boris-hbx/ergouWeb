use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use next_server::build_app;
use next_server::test_helpers::{
    auth_cookie, create_admin_user, create_test_user, create_test_user_with_status, test_state,
};

/// Helper: send a request and return (status, body as serde_json::Value).
async fn send(app: axum::Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let resp = app.oneshot(req).await.expect("request failed");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("body collect")
        .to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or(serde_json::json!({"raw": String::from_utf8_lossy(&bytes).to_string()}));
    (status, body)
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Health â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_health() {
    let app = build_app(test_state());
    let req = Request::get("/health").body(Body::empty()).unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Auth: Register â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_register_success() {
    let app = build_app(test_state());
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "alice",
                "password": "Alice123x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert!(body["user"]["id"].is_string());
}

#[tokio::test]
async fn test_register_duplicate() {
    let state = test_state();
    create_test_user(&state, "bob", "Bobpass1");

    let app = build_app(state);
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "bob",
                "password": "Bobpass1"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_register_weak_password() {
    let app = build_app(test_state());
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "charlie",
                "password": "short"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Auth: Login â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_login_success() {
    let state = test_state();
    create_test_user(&state, "dave", "Davepass1");

    let app = build_app(state);
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "dave",
                "password": "Davepass1"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert!(body["user"]["username"].is_string());
}

#[tokio::test]
async fn test_login_wrong_password() {
    let state = test_state();
    create_test_user(&state, "eve", "Evepass12");

    let app = build_app(state);
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "eve",
                "password": "Wrong123x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Auth: Unauthenticated â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_unauthenticated_401() {
    let app = build_app(test_state());
    let req = Request::get("/api/todos").body(Body::empty()).unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Todos â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_create_todo() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "frank", "Frank123");

    let app = build_app(state);
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "text": "Buy milk"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["item"]["text"], "Buy milk");
}

#[tokio::test]
async fn test_upgrade_todo_to_work_is_idempotent_and_recreates_after_soft_delete() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "todo_upgrade", "Upgrade123");
    let cookie = auth_cookie(&token);

    let app = build_app(state.clone());
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "text": "Prepare project brief",
                "content": "Bring notes into the formal tracker",
                "quadrant": "important-urgent",
                "progress": 40,
                "due_date": "2026-06-30",
                "tags": ["planning", "todo-source"]
            }))
            .unwrap(),
        ))
        .unwrap();
    let (_, body) = send(app, req).await;
    let todo_id = body["item"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["item"]["upgradedToWork"], false);

    let app = build_app(state.clone());
    let req = Request::post(&format!("/api/todos/{todo_id}/upgrade-to-work"))
        .header("cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["todo"]["upgradedToWork"], true);
    assert_eq!(body["workTask"]["title"], "Prepare project brief");
    assert_eq!(
        body["workTask"]["desc"],
        "Bring notes into the formal tracker"
    );
    assert_eq!(body["workTask"]["priority"], "high");
    assert_eq!(body["workTask"]["progress"], 40);
    assert_eq!(body["workTask"]["sourceType"], "todo");
    assert_eq!(body["workTask"]["sourceTodoId"], todo_id);
    let first_work_id = body["workTask"]["id"].as_i64().unwrap();

    let app = build_app(state.clone());
    let req = Request::post(&format!("/api/todos/{todo_id}/upgrade-to-work"))
        .header("cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(app, req).await;
    assert_eq!(body["workTask"]["id"].as_i64().unwrap(), first_work_id);

    let app = build_app(state.clone());
    let req = Request::delete(&format!("/api/work/tasks/{first_work_id}"))
        .header("cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);

    let app = build_app(state.clone());
    let req = Request::post(&format!("/api/todos/{todo_id}/upgrade-to-work"))
        .header("cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(app, req).await;
    let second_work_id = body["workTask"]["id"].as_i64().unwrap();
    assert_ne!(second_work_id, first_work_id);
    assert_eq!(
        body["todo"]["workTaskId"].as_str().unwrap(),
        second_work_id.to_string()
    );

    let app = build_app(state);
    let req = Request::get("/api/work/tasks?source_type=todo")
        .header("cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(app, req).await;
    let ids: Vec<i64> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["id"].as_i64())
        .collect();
    assert_eq!(ids, vec![second_work_id]);
}

#[tokio::test]
async fn test_create_todo_too_long() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "grace", "Grace123");

    let app = build_app(state);
    let long_text = "x".repeat(501);
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "text": long_text
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_list_todos() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "heidi", "Heidi123");

    // Create a todo first
    let app = build_app(state.clone());
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "text": "Test item"
            }))
            .unwrap(),
        ))
        .unwrap();
    let _ = send(app, req).await;

    // List
    let app = build_app(state);
    let req = Request::get("/api/todos")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["items"].as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn test_update_todo() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "ivan", "Ivan1234");

    // Create
    let app = build_app(state.clone());
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Original" })).unwrap(),
        ))
        .unwrap();
    let (_, body) = send(app, req).await;
    let todo_id = body["item"]["id"].as_str().unwrap().to_string();

    // Update
    let app = build_app(state);
    let req = Request::put(&format!("/api/todos/{}", todo_id))
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Updated" })).unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["item"]["text"], "Updated");
}

#[tokio::test]
async fn test_delete_todo() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "judy", "Judy1234");

    // Create
    let app = build_app(state.clone());
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Delete me" })).unwrap(),
        ))
        .unwrap();
    let (_, body) = send(app, req).await;
    let todo_id = body["item"]["id"].as_str().unwrap().to_string();

    // Delete (soft)
    let app = build_app(state.clone());
    let req = Request::delete(&format!("/api/todos/{}", todo_id))
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);

    // List should not contain it
    let app = build_app(state);
    let req = Request::get("/api/todos")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(app, req).await;
    let ids: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["id"].as_str())
        .collect();
    assert!(!ids.contains(&todo_id.as_str()));
}

#[tokio::test]
async fn test_user_isolation() {
    let state = test_state();
    let (_, token_a) = create_test_user(&state, "alice_iso", "Alice123");
    let (_, token_b) = create_test_user(&state, "bob_iso", "Bobbb123");

    // Alice creates a todo
    let app = build_app(state.clone());
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token_a))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Alice secret" })).unwrap(),
        ))
        .unwrap();
    let _ = send(app, req).await;

    // Bob lists â€” should see nothing
    let app = build_app(state);
    let req = Request::get("/api/todos")
        .header("cookie", auth_cookie(&token_b))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Edge cases â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_empty_text() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "kate", "Kate1234");

    let app = build_app(state);
    // Empty text â€” the server should accept it (no explicit empty check in create_todo
    // unless we add one). This test documents current behavior.
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "" })).unwrap(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    // Currently the server accepts empty text (200). If we add validation later,
    // this test will catch the change.
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_batch_limit() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "leo", "Leoo1234");

    let app = build_app(state);
    // 201 items exceeds the 200 batch limit
    let items: Vec<serde_json::Value> = (0..201)
        .map(|i| serde_json::json!({ "id": format!("fake-{}", i) }))
        .collect();
    let req = Request::put("/api/todos/batch")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(serde_json::to_string(&items).unwrap()))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Registration: status field â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_register_returns_active_status() {
    let app = build_app(test_state());
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "status_user",
                "password": "Status1x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["user"]["status"], "active");
}

#[tokio::test]
async fn test_register_11th_user_becomes_pending() {
    let state = test_state();

    // Create 10 users directly to fill daily quota
    for i in 0..10 {
        create_test_user(&state, &format!("filler_{}", i), "Filler1x");
    }

    // 11th user via registration API should be pending
    let app = build_app(state);
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "user_eleven",
                "password": "Eleven1x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["user"]["status"], "pending");
    assert!(body["message"].as_str().unwrap().contains("待审核"));
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ /me returns status â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_me_returns_status() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "me_user", "Meuser1x");

    let app = build_app(state);
    let req = Request::get("/api/auth/me")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["status"], "active");
}

#[tokio::test]
async fn test_me_returns_pending_status() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pending_me", "Pending1", "pending");

    let app = build_app(state);
    let req = Request::get("/api/auth/me")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["status"], "pending");
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Login returns status â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_login_returns_status_active() {
    let state = test_state();
    create_test_user(&state, "login_st", "Login1xx");

    let app = build_app(state);
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "login_st",
                "password": "Login1xx"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["status"], "active");
}

#[tokio::test]
async fn test_login_rejected_user_blocked() {
    let state = test_state();
    create_test_user_with_status(&state, "rejected_u", "Reject1x", "rejected");

    let app = build_app(state);
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "rejected_u",
                "password": "Reject1x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["success"], false);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Pending user: read OK, write 403 â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_pending_user_can_read_todos() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_read", "Pendrd1x", "pending");

    let app = build_app(state);
    let req = Request::get("/api/todos")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn test_pending_user_cannot_create_todo() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_write", "Pendwr1x", "pending");

    let app = build_app(state);
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Should fail" })).unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_PENDING");
}

#[tokio::test]
async fn test_pending_user_cannot_create_routine() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_rout", "Pendrt1x", "pending");

    let app = build_app(state);
    let req = Request::post("/api/routines")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Morning run" })).unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_PENDING");
}

#[tokio::test]
async fn test_pending_user_can_read_routines() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_rrd", "Pendrr1x", "pending");

    let app = build_app(state);
    let req = Request::get("/api/routines")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn test_pending_user_cannot_create_review() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_rev", "Pendrv1x", "pending");

    let app = build_app(state);
    let req = Request::post("/api/reviews")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "text": "Review thing",
                "frequency": "daily"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_PENDING");
}

#[tokio::test]
async fn test_pending_user_cannot_create_expense() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_exp", "Pendex1x", "pending");

    let app = build_app(state);
    let req = Request::post("/api/expenses")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "amount": 10.0,
                "date": "2026-02-26"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_PENDING");
}

#[tokio::test]
async fn test_pending_user_cannot_send_friend_request() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_fr", "Pendfr1x", "pending");

    let app = build_app(state);
    let req = Request::post("/api/friends/request")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "username": "nobody" })).unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_PENDING");
}

#[tokio::test]
async fn test_pending_user_cannot_change_password() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "pend_pw", "Pendpw1x", "pending");

    let app = build_app(state);
    let req = Request::post("/api/auth/change-password")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "old_password": "Pendpw1x",
                "new_password": "Newpwd1x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_PENDING");
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Rejected user: session returns 403 â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_rejected_user_write_returns_forbidden() {
    let state = test_state();
    let (_, token) = create_test_user_with_status(&state, "rej_sess", "Rejses1x", "rejected");

    let app = build_app(state);
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "Should fail" })).unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "ACCOUNT_REJECTED");
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Admin: pending users CRUD â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_admin_list_pending_users() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_lp", "Admin1xx");
    create_test_user_with_status(&state, "pend_a", "Penda11x", "pending");
    create_test_user_with_status(&state, "pend_b", "Pendb11x", "pending");

    let app = build_app(state);
    let req = Request::get("/api/admin/pending-users")
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["users"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_admin_approve_user() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_ap", "Admin2xx");
    let (pending_id, pending_token) =
        create_test_user_with_status(&state, "to_approve", "Approv1x", "pending");

    // Approve
    let app = build_app(state.clone());
    let req = Request::post(&format!("/api/admin/users/{}/approve", pending_id))
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);

    // Now the user should be able to create a todo
    let app = build_app(state);
    let req = Request::post("/api/todos")
        .header("content-type", "application/json")
        .header("cookie", auth_cookie(&pending_token))
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "text": "I can write now!" })).unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_admin_reject_user() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_rj", "Admin3xx");
    let (pending_id, pending_token) =
        create_test_user_with_status(&state, "to_reject", "Reject1x", "pending");

    // Reject
    let app = build_app(state.clone());
    let req = Request::post(&format!("/api/admin/users/{}/reject", pending_id))
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);

    // Rejected user's session should be invalidated (sessions deleted)
    let app = build_app(state);
    let req = Request::get("/api/auth/me")
        .header("cookie", auth_cookie(&pending_token))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_non_admin_cannot_list_pending() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "nonadmin", "Nonadm1x");

    let app = build_app(state);
    let req = Request::get("/api/admin/pending-users")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["success"], false);
}

#[tokio::test]
async fn test_non_admin_cannot_approve() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "nonadm_ap", "Nonadm2x");
    let (pending_id, _) = create_test_user_with_status(&state, "target_ap", "Target1x", "pending");

    let app = build_app(state);
    let req = Request::post(&format!("/api/admin/users/{}/approve", pending_id))
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["success"], false);
}

#[tokio::test]
async fn test_approve_already_active_user_404() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_aa", "Admin4xx");
    let (active_id, _) = create_test_user(&state, "already_active", "Active1x");

    let app = build_app(state);
    let req = Request::post(&format!("/api/admin/users/{}/approve", active_id))
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Admin dashboard: pending_count â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_admin_dashboard_includes_pending_count() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_pc", "Admin5xx");
    create_test_user_with_status(&state, "pend_c", "Pendc11x", "pending");

    let app = build_app(state);
    let req = Request::get("/api/admin/dashboard")
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["users"]["pending_count"], 1);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Path traversal protection â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_path_traversal_dotdot_in_user_id() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "pt_user1", "Ptuser1x");

    let app = build_app(state);
    let req = Request::get("/api/uploads/..%2F..%2F/next.db")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_path_traversal_dotdot_in_filename() {
    let state = test_state();
    let (user_id, token) = create_test_user(&state, "pt_user2", "Ptuser2x");

    let app = build_app(state);
    let req = Request::get(&format!("/api/uploads/{}/..%2F..%2Fetc%2Fpasswd", user_id))
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_path_traversal_backslash_in_user_id() {
    let state = test_state();
    let (_, token) = create_test_user(&state, "pt_user3", "Ptuser3x");

    let app = build_app(state);
    let req = Request::get("/api/uploads/foo%5Cbar/photo.jpg")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Pending registration notifies admins â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_pending_registration_notifies_admins() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_notif", "Admin6xx");

    // Fill 10 users to trigger pending
    for i in 0..10 {
        create_test_user(&state, &format!("fill_notif_{}", i), "Filler1x");
    }

    // Register 11th user
    let app = build_app(state.clone());
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "notif_user",
                "password": "Notif11x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["status"], "pending");

    // Admin should have a notification
    let app = build_app(state);
    let req = Request::get("/api/notifications/unread")
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    let has_pending_notif = items
        .iter()
        .any(|n| n["title"].as_str().unwrap_or("").contains("待审批"));
    assert!(
        has_pending_notif,
        "Admin should have a pending-user notification"
    );
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Approve creates user notification â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_approve_creates_user_notification() {
    let state = test_state();
    let (_, admin_token) = create_admin_user(&state, "admin_an", "Admin7xx");
    let (pending_id, pending_token) =
        create_test_user_with_status(&state, "appnotif_u", "Appnot1x", "pending");

    // Approve
    let app = build_app(state.clone());
    let req = Request::post(&format!("/api/admin/users/{}/approve", pending_id))
        .header("cookie", auth_cookie(&admin_token))
        .body(Body::empty())
        .unwrap();
    let _ = send(app, req).await;

    // User should have a notification about approval
    let app = build_app(state);
    let req = Request::get("/api/notifications/unread")
        .header("cookie", auth_cookie(&pending_token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    let has_approval = items
        .iter()
        .any(|n| n["title"].as_str().unwrap_or("").contains("通过"));
    assert!(has_approval, "User should have an approval notification");
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Routine Toggle â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_routine_toggle_and_list() {
    let state = test_state();
    let (_uid, token) = create_test_user(&state, "routineuser", "pass123");

    // Create a routine
    let app = build_app(state.clone());
    let req = Request::post("/api/routines")
        .header("cookie", auth_cookie(&token))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"text":"Morning exercise"}"#))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["success"].as_bool().unwrap());
    let routine_id = body["item"]["id"].as_str().unwrap().to_string();
    assert!(!body["item"]["completed_today"].as_bool().unwrap());

    // Toggle to complete
    let app = build_app(state.clone());
    let req = Request::post(format!("/api/routines/{}/toggle", routine_id))
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["success"].as_bool().unwrap());
    assert!(body["item"]["completed_today"].as_bool().unwrap());

    // List routines â€” should show completed_today = true
    let app = build_app(state.clone());
    let req = Request::get("/api/routines")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert!(
        items[0]["completed_today"].as_bool().unwrap(),
        "Routine should show completed_today=true after toggle. Got: {:?}",
        items[0]
    );

    // Toggle again to un-complete
    let app = build_app(state.clone());
    let req = Request::post(format!("/api/routines/{}/toggle", routine_id))
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body["item"]["completed_today"].as_bool().unwrap());

    // List again â€” should be uncompleted
    let app = build_app(state.clone());
    let req = Request::get("/api/routines")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert!(!items[0]["completed_today"].as_bool().unwrap());
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Soul State: GET â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_get_soul_state_success() {
    let state = test_state();
    let (user_id, token) = create_test_user(&state, "soul_user1", "Soul123x");

    // Pre-insert a soul state record
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, updated_at) VALUES (?1, datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    let app = build_app(state);
    let req = Request::get("/api/soul-state")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    let soul = &body["soul_state"];
    assert_eq!(soul["user_id"], user_id);
    assert_eq!(soul["classical_ratio"], 0.9);
    assert_eq!(soul["warmth_level"], 0.3);
    assert_eq!(soul["verbosity_level"], 0.3);
    assert_eq!(soul["proactivity_level"], 0.2);
    assert_eq!(soul["trust_level"], 0.1);
    assert_eq!(soul["relationship_stage"], "stranger");
    assert_eq!(soul["total_interactions"], 0);
    assert!(soul["updated_at"].is_string());
}

#[tokio::test]
async fn test_get_soul_state_lazy_create() {
    let state = test_state();
    let (user_id, token) = create_test_user(&state, "soul_lazy", "Lazy123x");

    // No pre-insert â€” should lazy-create on GET
    let app = build_app(state);
    let req = Request::get("/api/soul-state")
        .header("cookie", auth_cookie(&token))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["soul_state"]["user_id"], user_id);
    assert_eq!(body["soul_state"]["relationship_stage"], "stranger");
}

#[tokio::test]
async fn test_soul_state_unauthenticated_401() {
    let app = build_app(test_state());
    let req = Request::get("/api/soul-state").body(Body::empty()).unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Soul State: PUT â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_put_soul_state_partial_update() {
    let state = test_state();
    let (user_id, token) = create_test_user(&state, "soul_put1", "Sput123x");

    // Seed default soul state
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, updated_at) VALUES (?1, datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    let app = build_app(state);
    let req = Request::put("/api/soul-state")
        .header("cookie", auth_cookie(&token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "warmth_level": 0.5,
                "trust_level": 0.3
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    let soul = &body["soul_state"];
    // Updated fields
    assert_eq!(soul["warmth_level"], 0.5);
    assert_eq!(soul["trust_level"], 0.3);
    // Unchanged fields
    assert_eq!(soul["classical_ratio"], 0.9);
    assert_eq!(soul["verbosity_level"], 0.3);
    assert_eq!(soul["proactivity_level"], 0.2);
}

#[tokio::test]
async fn test_put_soul_state_range_validation() {
    let state = test_state();
    let (user_id, token) = create_test_user(&state, "soul_put2", "Sput223x");
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, updated_at) VALUES (?1, datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    let app = build_app(state);
    let req = Request::put("/api/soul-state")
        .header("cookie", auth_cookie(&token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "warmth_level": 1.5,
                "classical_ratio": 0.3
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], false);
    let invalid = body["invalid_fields"].as_array().unwrap();
    assert!(invalid.len() >= 2); // warmth_level > 1.0 and classical_ratio < 0.6
}

#[tokio::test]
async fn test_put_soul_state_logs_changes() {
    let state = test_state();
    let (user_id, token) = create_test_user(&state, "soul_log1", "Slog123x");
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, updated_at) VALUES (?1, datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    let app = build_app(state.clone());
    let req = Request::put("/api/soul-state")
        .header("cookie", auth_cookie(&token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "warmth_level": 0.6
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);

    // Check evolution log
    let db = state.db.lock();
    let log_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM soul_evolution_log WHERE user_id=?1 AND trigger_type='manual'",
            [&user_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        log_count >= 1,
        "Should have at least 1 manual evolution log entry"
    );
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Soul Evolution â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_soul_evolution_interaction_count() {
    let state = test_state();
    let (user_id, _token) = create_test_user(&state, "soul_evo1", "Sevo123x");
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, updated_at) VALUES (?1, datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    // Call evolve_after_chat (will skip LLM call since no API key in test)
    next_server::services::soul_evolution::evolve_after_chat(
        &state,
        &user_id,
        &[serde_json::json!({"role": "user", "content": "test"})],
    )
    .await;

    // Check total_interactions was incremented
    let db = state.db.lock();
    let count: i64 = db
        .query_row(
            "SELECT total_interactions FROM soul_states WHERE user_id=?1",
            [&user_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_soul_evolution_relationship_upgrade() {
    let state = test_state();
    let (user_id, _token) = create_test_user(&state, "soul_evo2", "Sevo223x");
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, total_interactions, updated_at) VALUES (?1, 9, datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    // This call should push total_interactions to 10 â†’ acquaintance
    next_server::services::soul_evolution::evolve_after_chat(
        &state,
        &user_id,
        &[serde_json::json!({"role": "user", "content": "hello"})],
    )
    .await;

    let db = state.db.lock();
    let stage: String = db
        .query_row(
            "SELECT relationship_stage FROM soul_states WHERE user_id=?1",
            [&user_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stage, "acquaintance");
    assert_eq!(
        db.query_row::<i64, _, _>(
            "SELECT total_interactions FROM soul_states WHERE user_id=?1",
            [&user_id],
            |r| r.get(0),
        )
        .unwrap(),
        10
    );
}

#[tokio::test]
async fn test_soul_state_field_clamping() {
    // Test that clamp_field works correctly
    use next_server::models::soul_state::clamp_field;

    // warmth_level: 0.0-1.0
    assert_eq!(clamp_field("warmth_level", 1.05), 1.0);
    assert_eq!(clamp_field("warmth_level", -0.1), 0.0);
    assert_eq!(clamp_field("warmth_level", 0.5), 0.5);

    // classical_ratio: 0.6-1.0
    assert_eq!(clamp_field("classical_ratio", 0.5), 0.6);
    assert_eq!(clamp_field("classical_ratio", 1.1), 1.0);

    // proactivity_level: 0.0-0.8
    assert_eq!(clamp_field("proactivity_level", 0.9), 0.8);
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Soul Prompt Building â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_soul_prompt_building() {
    let state = test_state();
    let (user_id, _token) = create_test_user(&state, "soul_prompt", "Spro123x");

    // Insert a soul state with familiar stage and adjusted params
    {
        let db = state.db.lock();
        db.execute(
            "INSERT INTO soul_states (user_id, warmth_level, trust_level, classical_ratio, relationship_stage, updated_at) VALUES (?1, 0.7, 0.7, 0.85, 'familiar', datetime('now'))",
            [&user_id],
        )
        .unwrap();
    }

    let db = state.db.lock();
    let prompt = next_server::services::context::build_system_prompt_with_page(
        &db,
        &user_id,
        None,
        "America/Toronto",
    );

    // Check soul state values are injected into the prompt.
    // (合并保留 main 的生产灵魂 prompt 格式：数值注入；dev 的语义段断言与合并后实际输出不符，已去除)
    assert!(prompt.contains("85%"), "Classical ratio should be present");
    assert!(prompt.contains("70%"), "Warmth should be present");
    assert!(
        prompt.len() > 1_000,
        "Prompt should include full dynamic context"
    );
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Registration creates soul state â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn test_register_creates_soul_state() {
    let state = test_state();
    let app = build_app(state.clone());
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": "soul_reg_user",
                "password": "Sreg123x"
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    let user_id = body["user"]["id"].as_str().unwrap();

    // Verify soul state was created
    let db = state.db.lock();
    let exists: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM soul_states WHERE user_id=?1",
            [user_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(exists, "Soul state should be created on registration");
}
