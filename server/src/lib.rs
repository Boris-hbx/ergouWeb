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
        .route("/avatar", put(auth::update_avatar));

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
        .route("/usage", get(routes::conversations::get_usage));

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
        .route("/summary", get(routes::expenses::get_summary))
        .route("/analytics", get(routes::expenses::get_analytics))
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
                .route("/conversations/{id}/messages", get(routes::admin::get_conversation_messages)),
        )
        .route("/moment", get(routes::moment::get_moment))
        .route(
            "/uploads/{user_id}/{filename}",
            get(routes::expenses::serve_photo),
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
