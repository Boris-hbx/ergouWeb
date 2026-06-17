//! `/api/auth/tokens` — Personal Access Token (PAT) 管理端点。
//!
//! 详见 spec: `C:\Project\ergouPM\specs\auth\spec.md` § 12。
//!
//! 用途:替代 session cookie 用于 CLI/CC 场景(典型:Claude Code 调 `/api/insights/*`)。
//! Boris 在 web 设置页生成一次,header `Authorization: Bearer tok_xxx` 鉴权。
//!
//! 实施关键决定:
//! - **SHA-256 哈希存储**(非 Argon2):token 是 32 字节高熵随机串,无暴力风险;
//!   SHA-256 确定性 → `idx_pat_hash` 索引可用精确查表(spec 写 Argon2 是误导,
//!   Argon2 随机 salt 让 hash 索引查表失效,实施必须用确定性 hash)
//! - `token_prefix` 前 8 字符明文供用户在列表中识别哪条
//! - 仅创建时返回明文 token(GET 端点不返)
//! - 软撤销(`revoked_at`)保留历史

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::auth::UserId;
use crate::state::AppState;

// ============ Token 工具 ============

/// 生成新 PAT 字符串:`tok_` 前缀 + base64url(32 bytes 随机) 总长约 47 字符。
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    format!("tok_{}", URL_SAFE_NO_PAD.encode(bytes))
}

/// SHA-256 hex 哈希。用于存储 + 查表索引。
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 提取 prefix(token 头 8 字符,如 "tok_3MvA")供列表识别。
pub fn token_prefix(token: &str) -> String {
    token.chars().take(8).collect()
}

// ============ 请求 / 响应类型 ============

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub label: String,
    #[serde(rename = "expiresAt", default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenListItem {
    pub id: i64,
    pub label: String,
    #[serde(rename = "tokenPrefix")]
    pub token_prefix: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "lastUsedAt", skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(rename = "revokedAt", skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

fn err_internal() -> (StatusCode, Json<JsonValue>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "INTERNAL" })),
    )
}

// ============ POST /api/auth/tokens — 创建 PAT ============

pub async fn create_token(
    State(state): State<AppState>,
    user: UserId,
    Json(req): Json<CreateTokenRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let label = req.label.trim();
    if label.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "EMPTY_LABEL", "message": "label 不能为空" })),
        );
    }
    if label.chars().count() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "success": false, "error": "LABEL_TOO_LONG", "message": "label 最多 64 字符" }),
            ),
        );
    }

    // 简单合法性:expires_at(如有)应是 RFC3339 / ISO-8601
    if let Some(exp) = req.expires_at.as_deref() {
        if !exp.is_empty() && chrono::DateTime::parse_from_rfc3339(exp).is_err() {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({ "success": false, "error": "INVALID_EXPIRES_AT", "message": "expiresAt 必须 RFC3339" }),
                ),
            );
        }
    }

    let token = generate_token();
    let hash = hash_token(&token);
    let prefix = token_prefix(&token);
    let now = chrono::Utc::now().to_rfc3339();
    let exp_norm = req
        .expires_at
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let new_id = {
        let db = state.db.lock();
        let res = db.execute(
            "INSERT INTO personal_tokens(user_id, label, token_hash, token_prefix, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![user.0, label, &hash, &prefix, &now, exp_norm],
        );
        match res {
            Ok(_) => db.last_insert_rowid(),
            Err(e) => {
                warn!(target: "pat", "insert failed: {}", e);
                return err_internal();
            }
        }
    };

    info!(
        target: "pat",
        "[pat] created id={} user={} prefix={} label={:?}",
        new_id, user.0, prefix, label
    );

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "item": {
                "id": new_id,
                "label": label,
                // 明文 token 仅在创建响应中返回这一次
                "token": token,
                "tokenPrefix": prefix,
                "createdAt": now,
                "expiresAt": exp_norm,
            }
        })),
    )
}

// ============ GET /api/auth/tokens — 列表(不返明文) ============

pub async fn list_tokens(
    State(state): State<AppState>,
    user: UserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let mut stmt = match db.prepare(
        "SELECT id, label, token_prefix, created_at, last_used_at, expires_at, revoked_at
         FROM personal_tokens
         WHERE user_id = ?1
         ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "pat", "list prepare: {}", e);
            return err_internal();
        }
    };
    let rows = stmt.query_map(params![user.0], |r| {
        Ok(TokenListItem {
            id: r.get(0)?,
            label: r.get(1)?,
            token_prefix: r.get(2)?,
            created_at: r.get(3)?,
            last_used_at: r.get(4)?,
            expires_at: r.get(5)?,
            revoked_at: r.get(6)?,
        })
    });
    let items: Vec<TokenListItem> = match rows {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            warn!(target: "pat", "list query: {}", e);
            return err_internal();
        }
    };
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

// ============ DELETE /api/auth/tokens/:id — 撤销 ============

pub async fn revoke_token(
    State(state): State<AppState>,
    user: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let now = chrono::Utc::now().to_rfc3339();
    let db = state.db.lock();
    // 仅可撤销自己的 token;已 revoke 的不再覆盖时间戳
    let updated = match db.execute(
        "UPDATE personal_tokens
         SET revoked_at = ?1
         WHERE id = ?2 AND user_id = ?3 AND revoked_at IS NULL",
        params![&now, id, user.0],
    ) {
        Ok(n) => n,
        Err(e) => {
            warn!(target: "pat", "revoke update: {}", e);
            return err_internal();
        }
    };
    if updated == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "TOKEN_NOT_FOUND_OR_ALREADY_REVOKED" })),
        );
    }
    info!(target: "pat", "[pat] revoked id={} user={}", id, user.0);
    (
        StatusCode::OK,
        Json(json!({ "success": true, "revokedAt": now })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn create_pat(
        app: &axum::Router,
        cookie: &str,
        label: &str,
        expires_at: Option<&str>,
    ) -> serde_json::Value {
        let body = if let Some(exp) = expires_at {
            format!(r#"{{"label":"{}","expiresAt":"{}"}}"#, label, exp)
        } else {
            format!(r#"{{"label":"{}"}}"#, label)
        };
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/tokens")
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        body_json(resp).await
    }

    /// 用 /api/auth/me 验证鉴权是否通过(Bearer 或 cookie)
    async fn call_me(
        app: &axum::Router,
        cookie: Option<&str>,
        bearer: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut req = Request::builder().method("GET").uri("/api/auth/me");
        if let Some(c) = cookie {
            req = req.header("Cookie", c);
        }
        if let Some(b) = bearer {
            req = req.header("Authorization", format!("Bearer {}", b));
        }
        let resp = app
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        (status, body_json(resp).await)
    }

    #[tokio::test]
    async fn create_returns_plaintext_token_once() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "alice", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let j = create_pat(&app, &cookie, "claude-code-insight", None).await;
        assert_eq!(j["success"], true);
        let t = j["item"]["token"].as_str().expect("token present once");
        assert!(t.starts_with("tok_"));
        assert!(t.len() > 40);
        let prefix = j["item"]["tokenPrefix"].as_str().unwrap();
        assert_eq!(prefix.chars().count(), 8);
    }

    #[tokio::test]
    async fn list_never_returns_plaintext_token() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "bob", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        let _ = create_pat(&app, &cookie, "label1", None).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/auth/tokens")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        let item = &j["items"][0];
        assert!(
            item.get("token").is_none(),
            "list 不应返回明文 token,got {:?}",
            item.get("token")
        );
        assert!(item["tokenPrefix"].as_str().is_some());
    }

    #[tokio::test]
    async fn bearer_token_authenticates_like_cookie() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "carol", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        let created = create_pat(&app, &cookie, "test", None).await;
        let pat = created["item"]["token"].as_str().unwrap().to_string();

        // Bearer 单独鉴权应通过(不带 cookie)
        let (status, j) = call_me(&app, None, Some(&pat)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(j["user"]["username"], "carol");
    }

    #[tokio::test]
    async fn revoked_bearer_token_unauthorized() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "dave", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        let created = create_pat(&app, &cookie, "x", None).await;
        let pat = created["item"]["token"].as_str().unwrap().to_string();
        let id = created["item"]["id"].as_i64().unwrap();

        // 撤销
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/auth/tokens/{}", id))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 撤销后用 Bearer 应失败(无 cookie 时 401)
        let (status, _) = call_me(&app, None, Some(&pat)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn expired_bearer_token_unauthorized() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "eve", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        // 创建一个已过期的 token(expiresAt 在过去)
        let past = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        let created = create_pat(&app, &cookie, "expired", Some(&past)).await;
        let pat = created["item"]["token"].as_str().unwrap().to_string();

        let (status, _) = call_me(&app, None, Some(&pat)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn cookie_and_bearer_both_present_works() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "fran", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        let created = create_pat(&app, &cookie, "x", None).await;
        let pat = created["item"]["token"].as_str().unwrap().to_string();

        // 同时带 cookie + Bearer:Bearer 优先;cookie 也有效;两者都 ok 时通过
        let (status, j) = call_me(&app, Some(&cookie), Some(&pat)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(j["user"]["username"], "fran");
    }

    #[tokio::test]
    async fn revoke_immediate_effect() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "ginny", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        let created = create_pat(&app, &cookie, "test", None).await;
        let pat = created["item"]["token"].as_str().unwrap().to_string();
        let id = created["item"]["id"].as_i64().unwrap();

        // 撤销前:能用
        let (status, _) = call_me(&app, None, Some(&pat)).await;
        assert_eq!(status, StatusCode::OK);

        // 撤销
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/auth/tokens/{}", id))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // 撤销后:立刻失效
        let (status, _) = call_me(&app, None, Some(&pat)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn suspended_user_pat_blocked_by_active_user_id() {
        let state = test_state();
        let (uid, sess) = create_test_user(&state, "hank", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state.clone());

        let created = create_pat(&app, &cookie, "test", None).await;
        let pat = created["item"]["token"].as_str().unwrap().to_string();

        // 把账户挂起(直接 SQL)
        {
            let db = state.db.lock();
            db.execute(
                "UPDATE users SET status = 'suspended' WHERE id = ?1",
                rusqlite::params![uid],
            )
            .unwrap();
        }

        // POST /api/routines 用 ActiveUserId;suspended 应在 extractor 就被 403,先于 body 校验
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/routines")
                    .header("Authorization", format!("Bearer {}", pat))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"text":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let j = body_json(resp).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(j["error"], "ACCOUNT_SUSPENDED");
    }

    #[tokio::test]
    async fn invalid_bearer_falls_back_to_cookie() {
        // spec § 12.5:"未命中/撤销/过期 → 不要立即 401,回落到 cookie 流程"
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "ivy", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        // 假 token + 有效 cookie → 应通过(走 cookie)
        let (status, j) = call_me(&app, Some(&cookie), Some("tok_invalid_garbage")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(j["user"]["username"], "ivy");
    }

    #[tokio::test]
    async fn empty_label_rejected() {
        let state = test_state();
        let (_uid, sess) = create_test_user(&state, "jack", "Pa55word1");
        let cookie = auth_cookie(&sess);
        let app = crate::build_app(state);

        let j = create_pat(&app, &cookie, "  ", None).await;
        assert_eq!(j["success"], false);
        assert_eq!(j["error"], "EMPTY_LABEL");
    }
}
