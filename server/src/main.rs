mod auth;
mod db;
mod models;
mod routes;
mod services;
mod state;

#[cfg(test)]
mod test_helpers;

use axum::extract::DefaultBodyLimit;
use axum::response::IntoResponse;
use axum::{
    http::StatusCode,
    routing::{delete, get, post, put},
    Router,
};
use http::HeaderValue;
use parking_lot::Mutex;
use state::AppState;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Build the full application router. Extracted so integration tests can call
/// `build_app(test_state())` and use `tower::ServiceExt::oneshot()`.
pub fn build_app(state: AppState) -> Router {
    // Auth routes (no session required)
    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/guest", post(auth::guest_login))
        .route("/me", get(auth::me))
        .route("/change-password", post(auth::change_password))
        .route("/avatar", put(auth::update_avatar))
        // T-096 / ADR-006:owner 紧急密码重置(需要 OWNER_RECOVERY_KEY 环境变量)
        .route("/owner-recovery", post(auth::owner_recovery))
        // T-116 / spec auth § 12:个人访问令牌 PAT(CLI/CC 场景替代 cookie)
        // 管 PAT 自身仍要 cookie 登录(UserId 守卫),不允许"用 PAT 创建 PAT"
        .route(
            "/tokens",
            get(routes::auth_tokens::list_tokens).post(routes::auth_tokens::create_token),
        )
        .route("/tokens/{id}", axum::routing::delete(routes::auth_tokens::revoke_token));

    // Todo routes (session required via UserId extractor)
    let todo_routes = Router::new()
        .route(
            "/",
            get(routes::todos::list_todos).post(routes::todos::create_todo),
        )
        .route("/counts", get(routes::todos::get_todo_counts))
        .route("/batch", put(routes::todos::batch_update_todos))
        .route(
            "/{id}",
            get(routes::todos::get_todo)
                .put(routes::todos::update_todo)
                .delete(routes::todos::delete_todo),
        )
        .route("/{id}/restore", post(routes::todos::restore_todo))
        .route(
            "/{id}/permanent",
            delete(routes::todos::permanent_delete_todo),
        );

    // Routine routes
    let routine_routes = Router::new()
        .route(
            "/",
            get(routes::routines::list_routines).post(routes::routines::create_routine),
        )
        .route("/{id}", delete(routes::routines::delete_routine))
        .route("/{id}/toggle", post(routes::routines::toggle_routine));

    // Review routes
    let review_routes = Router::new()
        .route(
            "/",
            get(routes::reviews::list_reviews).post(routes::reviews::create_review),
        )
        .route(
            "/{id}",
            put(routes::reviews::update_review).delete(routes::reviews::delete_review),
        )
        .route("/{id}/complete", post(routes::reviews::complete_review))
        .route("/{id}/uncomplete", post(routes::reviews::uncomplete_review));

    // Quote routes
    let quote_routes = Router::new().route("/random", get(routes::quotes::get_random_quote));

    // Chat routes (阿宝)
    let chat_routes = Router::new()
        .route("/", post(routes::chat::chat_handler))
        .route("/stream", post(routes::chat::chat_stream_handler))
        .route("/usage", get(routes::conversations::get_usage))
        .route(
            "/messages/{id}/feedback",
            put(routes::chat::message_feedback_handler),
        );

    // Conversation routes
    let conversation_routes = Router::new()
        .route("/", get(routes::conversations::list_conversations))
        .route("/{id}/messages", get(routes::conversations::get_messages))
        .route("/{id}", delete(routes::conversations::delete_conversation))
        .route(
            "/{id}/rename",
            post(routes::conversations::rename_conversation),
        );

    // Expense routes
    let expense_routes = Router::new()
        .route(
            "/",
            get(routes::expenses::list_entries).post(routes::expenses::create_entry),
        )
        .route("/stats", get(routes::expenses::get_stats))
        .route("/tags", get(routes::expenses::list_tags))
        .route("/rates", get(routes::expenses::get_rates))
        .route("/parse-preview", post(routes::expenses::parse_preview))
        .route(
            "/{id}",
            get(routes::expenses::get_entry)
                .put(routes::expenses::update_entry)
                .delete(routes::expenses::delete_entry),
        )
        .route("/{id}/photos", post(routes::expenses::upload_photos))
        .route("/{id}/parse", post(routes::expenses::parse_receipts))
        .route("/photos/{photo_id}", delete(routes::expenses::delete_photo))
        .layer(DefaultBodyLimit::max(50_000_000)); // 50MB for base64 encoded photos

    // Trip routes (差旅)
    let trip_routes = Router::new()
        .route(
            "/",
            get(routes::trips::list_trips).post(routes::trips::create_trip),
        )
        .route(
            "/{id}",
            get(routes::trips::get_trip)
                .put(routes::trips::update_trip)
                .delete(routes::trips::delete_trip),
        )
        .route("/{id}/items", post(routes::trips::create_item))
        .route(
            "/items/{item_id}",
            put(routes::trips::update_item).delete(routes::trips::delete_item),
        )
        .route(
            "/items/{item_id}/photos",
            post(routes::trips::upload_item_photos),
        )
        .route("/photos/{photo_id}", delete(routes::trips::delete_photo))
        .route("/{id}/collaborators", post(routes::trips::add_collaborator))
        .route(
            "/{id}/collaborators/{uid}",
            delete(routes::trips::remove_collaborator),
        )
        .route("/{id}/export/xlsx", get(routes::trips::export_xlsx))
        .route("/{id}/export/photos", get(routes::trips::export_photos))
        .route("/{id}/export/bundle", get(routes::trips::export_bundle))
        .route("/analyze", post(routes::trips::analyze_item))
        .layer(DefaultBodyLimit::max(50_000_000));

    // English scenario routes
    let english_routes = Router::new()
        .route(
            "/scenarios",
            get(routes::english::list_scenarios).post(routes::english::create_scenario),
        )
        .route(
            "/scenarios/{id}",
            get(routes::english::get_scenario)
                .put(routes::english::update_scenario)
                .delete(routes::english::delete_scenario),
        )
        .route(
            "/scenarios/{id}/generate",
            post(routes::english::generate_scenario),
        )
        .route(
            "/scenarios/{id}/archive",
            post(routes::english::archive_scenario),
        );

    // Friends routes
    let friends_routes = Router::new()
        .route("/", get(routes::friends::list_friends))
        .route("/requests", get(routes::friends::list_friend_requests))
        .route("/request", post(routes::friends::send_friend_request))
        .route("/search", get(routes::friends::search_users))
        .route("/{id}/accept", post(routes::friends::accept_friend))
        .route("/{id}/decline", post(routes::friends::decline_friend))
        .route("/{id}", delete(routes::friends::delete_friend));

    // Reminder routes
    let reminder_routes = Router::new()
        .route(
            "/",
            get(routes::reminders::list_reminders).post(routes::reminders::create_reminder),
        )
        .route("/pending-count", get(routes::reminders::pending_count))
        .route(
            "/{id}",
            put(routes::reminders::update_reminder).delete(routes::reminders::cancel_reminder),
        )
        .route(
            "/{id}/acknowledge",
            post(routes::reminders::acknowledge_reminder),
        )
        .route("/{id}/snooze", post(routes::reminders::snooze_reminder));

    // Push subscription routes
    let push_routes = Router::new()
        .route("/vapid-public-key", get(routes::push::get_vapid_public_key))
        .route(
            "/subscribe",
            post(routes::push::subscribe).delete(routes::push::unsubscribe),
        );

    // Notification routes
    let notification_routes = Router::new()
        .route("/unread", get(routes::notifications::unread_notifications))
        .route("/read-all", post(routes::notifications::mark_all_read))
        .route("/{id}/read", post(routes::notifications::mark_read));

    // Share routes
    let share_routes = Router::new()
        .route("/", post(routes::friends::share_item))
        .route("/sent", get(routes::friends::shared_sent))
        .route("/inbox", get(routes::friends::shared_inbox))
        .route("/inbox/count", get(routes::friends::shared_inbox_count))
        .route("/{id}/accept", post(routes::friends::accept_shared))
        .route("/{id}/dismiss", post(routes::friends::dismiss_shared));

    // Contacts routes
    let contacts_routes = Router::new()
        .route(
            "/",
            get(routes::contacts::list_contacts).post(routes::contacts::create_contact),
        )
        .route(
            "/{id}",
            put(routes::contacts::update_contact).delete(routes::contacts::delete_contact),
        );

    // Collaborate routes (todo + routine collaboration + confirmations)
    let collaborate_routes = Router::new()
        .route(
            "/todos/{id}",
            post(routes::collaborate::set_collaborator)
                .delete(routes::collaborate::remove_collaborator),
        )
        .route(
            "/todos/{id}/collaborators",
            get(routes::collaborate::list_collaborators),
        )
        .route(
            "/routines/{id}",
            post(routes::routine_collab::set_routine_collaborator)
                .delete(routes::routine_collab::remove_routine_collaborator),
        )
        .route(
            "/confirmations/pending",
            get(routes::collaborate::list_pending_confirmations),
        )
        .route(
            "/confirmations/{id}/respond",
            post(routes::collaborate::respond_confirmation),
        )
        .route(
            "/confirmations/{id}/withdraw",
            post(routes::collaborate::withdraw_confirmation),
        );

    // Settings routes (user preferences)
    let settings_routes = Router::new()
        .route(
            "/timezone",
            get(auth::get_timezone).put(auth::set_timezone),
        )
        .route(
            "/memories",
            get(auth::list_memories).delete(auth::delete_all_memories),
        )
        .route(
            "/memories/{id}",
            delete(auth::delete_memory_by_id),
        );

    // Health check
    let start_time = std::time::Instant::now();

    // API router
    let api_routes = Router::new()
        .nest("/auth", auth_routes)
        .nest("/settings", settings_routes)
        .nest("/todos", todo_routes)
        .nest("/routines", routine_routes)
        .nest("/reviews", review_routes)
        .nest("/quotes", quote_routes)
        .nest("/chat", chat_routes)
        .nest("/conversations", conversation_routes)
        .nest("/english", english_routes)
        .nest("/expenses", expense_routes)
        .nest("/trips", trip_routes)
        .nest("/friends", friends_routes)
        .nest("/reminders", reminder_routes)
        .nest("/notifications", notification_routes)
        .nest("/push", push_routes)
        .nest("/share", share_routes)
        .nest("/contacts", contacts_routes)
        .nest("/collaborate", collaborate_routes)
        // T-137 报告集:公开列表(无 session,handler 不取 UserId)
        .route(
            "/public/insight-reports",
            get(routes::insight_share::public_reports_list),
        )
        // Work module (T-094 / SPEC work-task-table) — registered flat to avoid
        // an Axum 0.8 nest issue we hit when nesting another Router inside api_routes.
        .route(
            "/work/tasks",
            get(routes::work_tasks::list_tasks).post(routes::work_tasks::create_task),
        )
        .route(
            "/work/tasks/{id}",
            axum::routing::patch(routes::work_tasks::update_task)
                .delete(routes::work_tasks::delete_task),
        )
        // T-131:周期任务完成历史
        .route(
            "/work/tasks/{id}/completions",
            get(routes::work_tasks::list_completions),
        )
        .route(
            "/work/task-completions",
            get(routes::work_tasks::list_all_completions),
        )
        .route(
            "/work/columns",
            get(routes::work_columns::list_columns)
                .put(routes::work_columns::batch_save_columns)
                .post(routes::work_columns::create_column),
        )
        .route(
            "/work/columns/{key}",
            delete(routes::work_columns::delete_column),
        )
        // Insight module (T-105 / SPEC insight) — Hybrid 架构:Web 后端只做收件箱/抓取/存储,
        // 报告生成在 Claude Code(Boris 本机)写回,公开 /r/{token} 不在 /api 下,见 build_app 末尾。
        .route(
            "/insights",
            get(routes::insights::list_insights).post(routes::insights::create_insight),
        )
        .route(
            "/insights/{id}",
            get(routes::insights::get_insight)
                .patch(routes::insights::update_insight)
                .delete(routes::insights::delete_insight),
        )
        .route(
            "/insights/{id}/claim",
            axum::routing::post(routes::insights::claim_insight),
        )
        .route(
            "/insights/{id}/release",
            axum::routing::post(routes::insights::release_insight),
        )
        .route(
            "/insights/{id}/reports",
            get(routes::reports::list_reports).post(routes::reports::create_report),
        )
        // T-107 v0.2:`/latest` 必须在 `/{version}` 之前注册(静态优先于动态)
        .route(
            "/insights/{id}/reports/latest",
            get(routes::reports::get_latest_report),
        )
        .route(
            "/insights/{id}/reports/{version}",
            get(routes::reports::get_report_by_version)
                .patch(routes::reports::update_report),
        )
        .route(
            "/insights/{id}/regenerate",
            axum::routing::post(routes::insights::regenerate_insight),
        )
        // T-107 v0.2:share 端点保留 GET(查 active);POST/DELETE 物理删除(前端 T-106 没开工,无调用方)
        // 业务统一走 publish/retract(事务化,见 spec § 8.4)
        .route(
            "/insights/{id}/share",
            get(routes::share_links::list_shares_for_insight),
        )
        .route(
            "/insights/{id}/publish",
            axum::routing::post(routes::share_links::publish_insight),
        )
        .route(
            "/insights/{id}/retract",
            axum::routing::post(routes::share_links::retract_insight),
        )
        .route(
            "/sources",
            get(routes::sources::list_sources).post(routes::sources::create_source),
        )
        .route(
            "/sources/{id}",
            axum::routing::patch(routes::sources::update_source)
                .delete(routes::sources::delete_source),
        )
        .route(
            "/sources/{id}/refetch",
            axum::routing::post(routes::sources::refetch_source),
        )
        // T-107 v0.2:Annotations
        .route(
            "/insights/{id}/annotations",
            get(routes::annotations::list_annotations)
                .post(routes::annotations::create_annotation),
        )
        .route(
            "/annotations/{id}",
            axum::routing::patch(routes::annotations::update_annotation)
                .delete(routes::annotations::delete_annotation),
        )
        // Insight v0.3(T-122 / SPEC insight v0.3)— 单层 insight_task,
        // 必须与 lib.rs build_app 同步(memory:duplicate-build-app-lib-rs)。
        .route(
            "/insight-tasks",
            get(routes::insight_tasks::list_tasks).post(routes::insight_tasks::create_task),
        )
        .route(
            "/insight-tasks/{id}",
            get(routes::insight_tasks::get_task)
                .patch(routes::insight_tasks::update_task)
                .delete(routes::insight_tasks::delete_task),
        )
        .route(
            "/insight-tasks/{id}/claim",
            post(routes::insight_tasks::claim_task),
        )
        .route(
            "/insight-tasks/{id}/release",
            post(routes::insight_tasks::release_task),
        )
        // T-126:报告分享(v0.4)
        .route(
            "/insight-tasks/{id}/publish",
            post(routes::insight_share::publish),
        )
        .route(
            "/insight-tasks/{id}/retract",
            post(routes::insight_share::retract),
        )
        .route(
            "/insight-tasks/{id}/share",
            get(routes::insight_share::list_shares),
        )
        // `/latest` 必须在动态 reports 之前(本组无 /{version},但保持静态优先习惯)
        .route(
            "/insight-tasks/{id}/reports/latest",
            get(routes::insight_tasks::get_latest_report),
        )
        .route(
            "/insight-tasks/{id}/reports",
            get(routes::insight_tasks::list_reports).post(routes::insight_tasks::create_report),
        )
        .nest(
            "/soul-state",
            Router::new()
                .route(
                    "/",
                    get(routes::soul_state::get_soul_state)
                        .put(routes::soul_state::update_soul_state),
                )
                .route("/logs", get(routes::soul_state::get_evolution_logs)),
        )
        .nest(
            "/memories",
            Router::new()
                .route(
                    "/",
                    get(routes::memories::list_memories)
                        .post(routes::memories::create_memory)
                        .delete(routes::memories::clear_memories),
                )
                .route("/batch", post(routes::memories::batch_import))
                .route("/search", get(routes::memories::search_memories))
                .route(
                    "/{id}",
                    put(routes::memories::update_memory)
                        .delete(routes::memories::delete_memory),
                ),
        )
        .nest(
            "/admin",
            Router::new()
                // Existing endpoints
                .route("/dashboard", get(routes::admin::dashboard))
                .route("/pending-users", get(routes::admin::pending_users))
                .route("/users/{id}/approve", post(routes::admin::approve_user))
                .route("/users/{id}/reject", post(routes::admin::reject_user))
                .route("/security-events", get(routes::admin::security_events))
                .route("/users/{id}/restore", post(routes::admin::restore_user))
                // New endpoints: User Management
                .route("/users", get(routes::admin::list_users))
                .route("/users/{id}/role", put(routes::admin::change_role))
                .route("/users/{id}/force-logout", post(routes::admin::force_logout))
                .route("/users/{id}/suspend", post(routes::admin::suspend_user))
                // New endpoints: Conversation Monitor
                .route("/conversations/users", get(routes::admin::conversation_user_summary))
                .route("/conversations", get(routes::admin::list_conversations))
                .route("/conversations/{id}/messages", get(routes::admin::get_conversation_messages))
                // New endpoints: AI Dashboard
                .route("/ai-usage", get(routes::admin::ai_usage))
                .route("/ai-usage/providers", get(routes::admin::ai_providers))
                // New endpoints: Risk & System
                .route("/security-events-v2", get(routes::admin::security_events_v2))
                .route("/security-events/{id}/review", post(routes::admin::review_security_event))
                .route("/system-status", get(routes::admin::system_status))
                .route("/audit-log", get(routes::admin::audit_log))
                // People (人物档案)
                .route("/people", get(routes::admin::list_people).post(routes::admin::create_person))
                .route("/people/{id}", put(routes::admin::update_person).delete(routes::admin::delete_person))
                // T-115:一次性回补 todo.content → work_task.desc
                .route("/migrate-todo-content", post(routes::admin::migrate_todo_content))
                // T-122:洞察 v0.3 数据迁移(insights → insight_tasks)
                .route("/migrate-insight-v0.3", post(routes::admin::migrate_insight_v0_3))
                // T-089 块2:30 req/min/user 限流 —— 防暴力枚举 / DoS。
                // route_layer 比 layer 更紧凑;仅作用于 admin 路由子树。
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    auth::admin_rate_limit_middleware,
                )),
        )
        .route("/moment", get(routes::moment::get_moment))
        .route(
            "/uploads/{user_id}/{filename}",
            get(routes::expenses::serve_photo),
        )
        .route("/client-errors", post(routes::observability::report_client_error));

    Router::new()
        .route("/health", get(move || async move {
            let uptime = start_time.elapsed().as_secs();
            axum::Json(serde_json::json!({
                "status": "ok",
                "uptime": uptime
            }))
        }))
        .nest("/api", api_routes)
        // T-105/T-126 SPEC insight:公开分享页 /r/{token}(无需 session)
        // v0.4 起指向 insight_share(单层 insight_tasks/insight_reports);v0.2 share_links 已弃
        .route("/r", get(routes::insight_share::collection_page))
        .route("/r/{token}", get(routes::insight_share::public_page))
        .route("/r/{token}/data", get(routes::insight_share::public_data))
        .layer(SetResponseHeaderLayer::overriding(
            http::header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            http::header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            http::header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            http::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(self), microphone=(), geolocation=()"),
        ))
        .layer(DefaultBodyLimit::max(1_048_576)) // 1MB global body size limit
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin, _| {
                    if let Ok(s) = origin.to_str() {
                        s == "https://next-boris.fly.dev"
                            || s == "https://next-boris-staging.fly.dev"
                            || s.starts_with("http://localhost:")
                            || s.starts_with("http://127.0.0.1:")
                    } else {
                        false
                    }
                }))
                .allow_methods([
                    http::Method::GET,
                    http::Method::POST,
                    http::Method::PUT,
                    http::Method::DELETE,
                    http::Method::OPTIONS,
                ])
                .allow_headers([
                    http::header::CONTENT_TYPE,
                    http::header::COOKIE,
                    http::header::AUTHORIZATION,
                ])
                .allow_credentials(true)
                .max_age(std::time::Duration::from_secs(3600)),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,tower_http=info")),
        )
        .compact()
        .init();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "data/next.db".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    println!("Initializing database at {}", db_path);
    let conn = db::init_db(&db_path);

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        db_path: db_path.clone(),
        start_time: std::time::Instant::now(),
        moment_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        login_ip_attempts: Arc::new(Mutex::new(std::collections::HashMap::new())),
        login_user_lockouts: Arc::new(Mutex::new(std::collections::HashMap::new())),
        ai_rate_limits: Arc::new(Mutex::new(std::collections::HashMap::new())),
        guest_ip_rate_limits: Arc::new(Mutex::new(std::collections::HashMap::new())),
        error_report_limits: Arc::new(Mutex::new(std::collections::HashMap::new())),
        recovery_ip_attempts: Arc::new(Mutex::new(std::collections::HashMap::new())),
        recovery_ip_lockouts: Arc::new(Mutex::new(std::collections::HashMap::new())),
        admin_user_rate_limits: Arc::new(Mutex::new(std::collections::HashMap::new())),
        guest_ai_ip_aggregate: Arc::new(Mutex::new(std::collections::HashMap::new())),
    };

    // Spawn reminder poller (checks every 30s for due reminders)
    services::reminder_poller::spawn_poller(state.db.clone());

    // Schedule daily backup
    let backup_state = state.clone();
    let backup_db_path = db_path.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            let db = backup_state.db.lock();
            let backup_dir = std::path::Path::new(&backup_db_path)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("backups");
            db::daily_backup(&db, backup_dir.to_str().unwrap_or("data/backups"));
        }
    });

    // Spawn cleanup task: purge expired rate-limit entries + expired sessions every 10 min
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(600)).await;
            // Clean expired IP attempts
            {
                let mut attempts = cleanup_state.login_ip_attempts.lock();
                attempts.retain(|_, (_, t)| t.elapsed().as_secs() < 120);
            }
            // Clean expired user lockouts
            {
                let mut lockouts = cleanup_state.login_user_lockouts.lock();
                lockouts.retain(|_, (_, t)| t.elapsed().as_secs() < 900);
            }
            // Clean expired AI rate limits
            {
                let mut limits = cleanup_state.ai_rate_limits.lock();
                limits.retain(|_, t| t.elapsed().as_secs() < 60);
            }
            // Clean expired error report rate limits
            {
                let mut limits = cleanup_state.error_report_limits.lock();
                limits.retain(|_, (_, t)| t.elapsed().as_secs() < 60);
            }
            // Clean expired sessions from DB
            {
                let db = cleanup_state.db.lock();
                if let Err(e) = db.execute(
                    "DELETE FROM sessions WHERE expires_at < datetime('now')",
                    [],
                ) {
                    eprintln!("[cleanup] session cleanup failed: {}", e);
                }
                // Clean client_errors older than 14 days
                if let Err(e) = db.execute(
                    "DELETE FROM client_errors WHERE created_at < datetime('now', '-14 days')",
                    [],
                ) {
                    eprintln!("[cleanup] client_errors cleanup failed: {}", e);
                }
            }
        }
    });

    // Spawn guest cleanup task: purge expired guests every hour
    let guest_cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            services::guest_seed::cleanup_expired_guests(&guest_cleanup_state);
        }
    });

    // Frontend static files
    let frontend_dir = std::env::var("FRONTEND_DIR").unwrap_or_else(|_| "../frontend".to_string());
    let sw_dir = frontend_dir.clone();
    let index_dir = frontend_dir.clone();
    let index_dir2 = frontend_dir.clone();
    let login_dir = frontend_dir.clone();

    let app = build_app(state)
        .route(
            "/sw.js",
            get(move || async move {
                match tokio::fs::read_to_string(format!("{}/sw.js", sw_dir)).await {
                    Ok(body) => (
                        [
                            (http::header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
                            (
                                http::header::CACHE_CONTROL,
                                "no-cache, no-store, must-revalidate",
                            ),
                        ],
                        body,
                    )
                        .into_response(),
                    Err(_) => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        // Serve HTML files explicitly with charset=utf-8 to prevent mojibake on mobile.
        // axum::response::Html sets Content-Type: text/html; charset=utf-8 automatically.
        .route(
            "/",
            get(move || async move {
                match tokio::fs::read_to_string(format!("{}/index.html", index_dir)).await {
                    Ok(body) => axum::response::Html(body).into_response(),
                    Err(_) => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        .route(
            "/index.html",
            get(move || async move {
                match tokio::fs::read_to_string(format!("{}/index.html", index_dir2)).await {
                    Ok(body) => axum::response::Html(body).into_response(),
                    Err(_) => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        .route(
            "/login.html",
            get(move || async move {
                match tokio::fs::read_to_string(format!("{}/login.html", login_dir)).await {
                    Ok(body) => axum::response::Html(body).into_response(),
                    Err(_) => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        .fallback_service(
            tower_http::services::ServeDir::new(&frontend_dir)
                .append_index_html_on_directories(true),
        )
        .layer(axum::middleware::map_response(|mut response: axum::response::Response| async move {
            if let Some(ct) = response.headers().get(http::header::CONTENT_TYPE).cloned() {
                if let Ok(s) = ct.to_str() {
                    if s.starts_with("text/") && !s.contains("charset") {
                        if let Ok(v) = HeaderValue::from_str(&format!("{}; charset=utf-8", s)) {
                            response.headers_mut().insert(http::header::CONTENT_TYPE, v);
                        }
                    }
                }
            }
            response
        }));

    let addr = format!("0.0.0.0:{}", port);
    println!("Next server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
