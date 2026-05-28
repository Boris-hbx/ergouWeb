pub mod auth;
pub mod db;
pub mod models;
pub mod routes;
pub mod services;
pub mod state;

pub mod test_helpers;

// Re-export build_app from the binary crate is not possible,
// so we duplicate the builder here for integration tests.
use axum::extract::DefaultBodyLimit;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use http::HeaderValue;
use tower_http::set_header::SetResponseHeaderLayer;

/// Build the API router for testing. Mirrors main.rs `build_app`.
pub fn build_app(state: state::AppState) -> Router {
    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/guest", post(auth::guest_login))
        .route("/me", get(auth::me))
        .route("/change-password", post(auth::change_password))
        .route("/avatar", put(auth::update_avatar))
        // T-096 / ADR-006:owner 紧急密码重置(必须双写 main.rs + lib.rs,见 memory duplicate-build-app-lib-rs.md)
        .route("/owner-recovery", post(auth::owner_recovery))
        // T-116 / spec auth § 12:个人访问令牌 PAT(CLI/CC 场景替代 cookie)
        .route(
            "/tokens",
            get(routes::auth_tokens::list_tokens).post(routes::auth_tokens::create_token),
        )
        .route("/tokens/{id}", axum::routing::delete(routes::auth_tokens::revoke_token));

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

    let routine_routes = Router::new()
        .route(
            "/",
            get(routes::routines::list_routines).post(routes::routines::create_routine),
        )
        .route("/{id}", delete(routes::routines::delete_routine))
        .route("/{id}/toggle", post(routes::routines::toggle_routine));

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

    let quote_routes = Router::new().route("/random", get(routes::quotes::get_random_quote));

    let chat_routes = Router::new()
        .route("/", post(routes::chat::chat_handler))
        .route("/usage", get(routes::conversations::get_usage))
        .route(
            "/messages/{id}/feedback",
            put(routes::chat::message_feedback_handler),
        );

    let conversation_routes = Router::new()
        .route("/", get(routes::conversations::list_conversations))
        .route("/{id}/messages", get(routes::conversations::get_messages))
        .route("/{id}", delete(routes::conversations::delete_conversation))
        .route(
            "/{id}/rename",
            post(routes::conversations::rename_conversation),
        );

    let expense_routes = Router::new()
        .route(
            "/",
            get(routes::expenses::list_entries).post(routes::expenses::create_entry),
        )
        .route("/stats", get(routes::expenses::get_stats))
        .route("/tags", get(routes::expenses::list_tags))
        .route(
            "/{id}",
            get(routes::expenses::get_entry)
                .put(routes::expenses::update_entry)
                .delete(routes::expenses::delete_entry),
        )
        .route("/{id}/photos", post(routes::expenses::upload_photos))
        .route("/{id}/parse", post(routes::expenses::parse_receipts))
        .route("/photos/{photo_id}", delete(routes::expenses::delete_photo))
        .layer(DefaultBodyLimit::max(20_000_000));

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
        .layer(DefaultBodyLimit::max(50_000_000));

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

    let friends_routes = Router::new()
        .route("/", get(routes::friends::list_friends))
        .route("/requests", get(routes::friends::list_friend_requests))
        .route("/request", post(routes::friends::send_friend_request))
        .route("/search", get(routes::friends::search_users))
        .route("/{id}/accept", post(routes::friends::accept_friend))
        .route("/{id}/decline", post(routes::friends::decline_friend))
        .route("/{id}", delete(routes::friends::delete_friend));

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

    let push_routes = Router::new()
        .route("/vapid-public-key", get(routes::push::get_vapid_public_key))
        .route(
            "/subscribe",
            post(routes::push::subscribe).delete(routes::push::unsubscribe),
        );

    let notification_routes = Router::new()
        .route("/unread", get(routes::notifications::unread_notifications))
        .route("/read-all", post(routes::notifications::mark_all_read))
        .route("/{id}/read", post(routes::notifications::mark_read));

    let share_routes = Router::new()
        .route("/", post(routes::friends::share_item))
        .route("/inbox", get(routes::friends::shared_inbox))
        .route("/inbox/count", get(routes::friends::shared_inbox_count))
        .route("/{id}/accept", post(routes::friends::accept_shared))
        .route("/{id}/dismiss", post(routes::friends::dismiss_shared));

    let contacts_routes = Router::new()
        .route(
            "/",
            get(routes::contacts::list_contacts).post(routes::contacts::create_contact),
        )
        .route(
            "/{id}",
            put(routes::contacts::update_contact).delete(routes::contacts::delete_contact),
        );

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

    let start_time = std::time::Instant::now();

    let api_routes = Router::new()
        .nest("/auth", auth_routes)
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
        .nest(
            "/soul-state",
            Router::new().route(
                "/",
                get(routes::soul_state::get_soul_state)
                    .put(routes::soul_state::update_soul_state),
            ),
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
                .route("/{id}", delete(routes::memories::delete_memory)),
        )
        .nest(
            "/admin",
            Router::new()
                .route("/dashboard", get(routes::admin::dashboard))
                .route("/pending-users", get(routes::admin::pending_users))
                .route("/users/{id}/approve", post(routes::admin::approve_user))
                .route("/users/{id}/reject", post(routes::admin::reject_user))
                .route("/conversations/users", get(routes::admin::conversation_user_summary))
                .route("/conversations", get(routes::admin::list_conversations))
                .route("/conversations/{id}/messages", get(routes::admin::get_conversation_messages))
                // T-115:一次性回补 todo.content → work_task.desc
                .route("/migrate-todo-content", post(routes::admin::migrate_todo_content))
                // T-089 块2:30 req/min/user 限流(必须双写 main.rs + lib.rs)
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
        // Work module (T-094 / SPEC work-task-table)
        .route(
            "/work/tasks",
            get(routes::work_tasks::list_tasks).post(routes::work_tasks::create_task),
        )
        .route(
            "/work/tasks/{id}",
            axum::routing::patch(routes::work_tasks::update_task)
                .delete(routes::work_tasks::delete_task),
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
        // T-105 SPEC insight — 注意此处必须与 main.rs 同步(memory:duplicate-build-app)
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
        // T-107 v0.2 镜像 main.rs
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
        // T-107 v0.2:share 只剩 GET;POST/DELETE 物理删除;业务走 publish/retract
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
        // T-107 v0.2:Annotations(镜像 main.rs)
        .route(
            "/insights/{id}/annotations",
            get(routes::annotations::list_annotations)
                .post(routes::annotations::create_annotation),
        )
        .route(
            "/annotations/{id}",
            axum::routing::patch(routes::annotations::update_annotation)
                .delete(routes::annotations::delete_annotation),
        );

    Router::new()
        .route("/health", get(move || async move {
            let uptime = start_time.elapsed().as_secs();
            axum::Json(serde_json::json!({
                "status": "ok",
                "uptime": uptime
            }))
        }))
        .nest("/api", api_routes)
        // T-105 公开 /r/{token}(无 session,镜像 main.rs)
        .route("/r/{token}", get(routes::share_links::public_share_page))
        .route("/r/{token}/data", get(routes::share_links::public_share_data))
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
        .layer(DefaultBodyLimit::max(1_048_576))
        .with_state(state)
}
