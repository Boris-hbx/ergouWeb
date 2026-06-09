use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use crate::auth::{check_guest_ai_quota, extract_client_ip, ActiveUserId, UserId};
use crate::models::trip::*;
use crate::services::llm::LlmClient;
use crate::state::AppState;

// ===== Permission helpers =====

/// Returns (has_access, is_owner, role)
fn check_trip_access(db: &Connection, trip_id: &str, user_id: &str) -> (bool, bool, String) {
    // Check if owner
    let is_owner: bool = db
        .query_row(
            "SELECT COUNT(*) FROM trips WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![trip_id, user_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if is_owner {
        return (true, true, "owner".to_string());
    }

    // Check if collaborator
    let role: Option<String> = db
        .query_row(
            "SELECT role FROM trip_collaborators WHERE trip_id = ?1 AND user_id = ?2",
            rusqlite::params![trip_id, user_id],
            |row| row.get(0),
        )
        .ok();

    match role {
        Some(r) => (true, false, r),
        None => (false, false, String::new()),
    }
}

/// Check item access: returns (has_access, is_owner, role, trip_id)
fn check_item_access(
    db: &Connection,
    item_id: &str,
    user_id: &str,
) -> (bool, bool, String, String) {
    let trip_id: Option<String> = db
        .query_row(
            "SELECT trip_id FROM trip_items WHERE id = ?1",
            rusqlite::params![item_id],
            |row| row.get(0),
        )
        .ok();

    match trip_id {
        Some(tid) => {
            let (access, owner, role) = check_trip_access(db, &tid, user_id);
            (access, owner, role, tid)
        }
        None => (false, false, String::new(), String::new()),
    }
}

fn build_reimburse_summary(db: &Connection, trip_id: &str) -> ReimburseSummary {
    let mut summary = ReimburseSummary::default();
    if let Ok(mut stmt) = db.prepare(
        "SELECT reimburse_status, COUNT(*) FROM trip_items WHERE trip_id = ?1 GROUP BY reimburse_status",
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![trip_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) {
            for r in rows.flatten() {
                match r.0.as_str() {
                    "pending" => summary.pending = r.1,
                    "submitted" => summary.submitted = r.1,
                    "approved" => summary.approved = r.1,
                    "rejected" => summary.rejected = r.1,
                    "na" => summary.na = r.1,
                    _ => {}
                }
                summary.total += r.1;
            }
        }
    }
    summary
}

// ===== List trips =====
pub async fn list_trips(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    // Own trips + collaborated trips
    let sql = "
        SELECT t.id, t.user_id, t.title, t.destination, t.date_from, t.date_to,
               t.purpose, t.notes, t.currency, t.created_at, t.updated_at,
               (SELECT COUNT(*) FROM trip_items WHERE trip_id = t.id) as item_count,
               (SELECT COALESCE(SUM(amount), 0) FROM trip_items WHERE trip_id = t.id) as total_amount,
               CASE WHEN t.user_id = ?1 THEN 1 ELSE 0 END as is_owner
        FROM trips t
        WHERE t.user_id = ?1
        UNION
        SELECT t.id, t.user_id, t.title, t.destination, t.date_from, t.date_to,
               t.purpose, t.notes, t.currency, t.created_at, t.updated_at,
               (SELECT COUNT(*) FROM trip_items WHERE trip_id = t.id) as item_count,
               (SELECT COALESCE(SUM(amount), 0) FROM trip_items WHERE trip_id = t.id) as total_amount,
               0 as is_owner
        FROM trips t
        JOIN trip_collaborators tc ON tc.trip_id = t.id
        WHERE tc.user_id = ?1
        ORDER BY date_from DESC
    ";

    let mut trips: Vec<Trip> = Vec::new();
    if let Ok(mut stmt) = db.prepare(sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![user_id.0], |row| {
            Ok(Trip {
                id: row.get(0)?,
                user_id: row.get(1)?,
                title: row.get(2)?,
                destination: row.get(3)?,
                date_from: row.get(4)?,
                date_to: row.get(5)?,
                purpose: row.get(6)?,
                notes: row.get(7)?,
                currency: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                item_count: row.get(11)?,
                total_amount: row.get(12)?,
                reimburse_summary: ReimburseSummary::default(),
                is_owner: row.get::<_, i64>(13).unwrap_or(0) != 0,
            })
        }) {
            trips = rows.filter_map(|r| r.ok()).collect();
        }
    }

    // Fill reimburse summaries
    for trip in &mut trips {
        trip.reimburse_summary = build_reimburse_summary(&db, &trip.id);
    }

    (
        StatusCode::OK,
        Json(json!({ "success": true, "trips": trips })),
    )
}

// ===== Create trip =====
pub async fn create_trip(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<CreateTripRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "标题不能为空" })),
        );
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let destination = req.destination.unwrap_or_default();
    let purpose = req.purpose.unwrap_or_default();
    let notes = req.notes.unwrap_or_default();
    let currency = req.currency.unwrap_or_else(|| "CAD".to_string());

    let db = state.db.lock();
    match db.execute(
        "INSERT INTO trips (id, user_id, title, destination, date_from, date_to, purpose, notes, currency, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        rusqlite::params![id, user_id.0, req.title.trim(), destination, req.date_from, req.date_to, purpose, notes, currency, now],
    ) {
        Ok(_) => {
            let trip = Trip {
                id,
                user_id: user_id.0,
                title: req.title,
                destination,
                date_from: req.date_from,
                date_to: req.date_to,
                purpose,
                notes,
                currency,
                created_at: now.clone(),
                updated_at: now,
                item_count: 0,
                total_amount: 0.0,
                reimburse_summary: ReimburseSummary::default(),
                is_owner: true,
            };
            (StatusCode::CREATED, Json(json!({ "success": true, "trip": trip })))
        }
        Err(e) => {
            eprintln!("[Trip] create error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "message": "创建失败" })))
        }
    }
}

// ===== Get trip detail =====
pub async fn get_trip(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, _role) = check_trip_access(&db, &id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        );
    }

    let trip = db.query_row(
        "SELECT id, user_id, title, destination, date_from, date_to, purpose, notes, currency, created_at, updated_at,
                (SELECT COUNT(*) FROM trip_items WHERE trip_id = t.id),
                (SELECT COALESCE(SUM(amount), 0) FROM trip_items WHERE trip_id = t.id)
         FROM trips t WHERE t.id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(Trip {
                id: row.get(0)?,
                user_id: row.get(1)?,
                title: row.get(2)?,
                destination: row.get(3)?,
                date_from: row.get(4)?,
                date_to: row.get(5)?,
                purpose: row.get(6)?,
                notes: row.get(7)?,
                currency: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                item_count: row.get(11)?,
                total_amount: row.get(12)?,
                reimburse_summary: ReimburseSummary::default(),
                is_owner,
            })
        },
    );

    let mut trip = match trip {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "行程不存在" })),
            );
        }
    };
    trip.reimburse_summary = build_reimburse_summary(&db, &id);

    // Get items with photo counts
    let items: Vec<TripItem> = db
        .prepare(
            "SELECT id, trip_id, type, date, description, amount, currency, reimburse_status, notes, sort_order, created_at, updated_at,
                    (SELECT COUNT(*) FROM trip_photos WHERE item_id = ti.id) as photo_count
             FROM trip_items ti WHERE trip_id = ?1 ORDER BY date, sort_order",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok(TripItem {
                    id: row.get(0)?,
                    trip_id: row.get(1)?,
                    item_type: row.get(2)?,
                    date: row.get(3)?,
                    description: row.get(4)?,
                    amount: row.get(5)?,
                    currency: row.get(6)?,
                    reimburse_status: row.get(7)?,
                    notes: row.get(8)?,
                    sort_order: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                    photo_count: row.get(12)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Get all photos for this trip's items in one query (avoids N+1)
    let all_photos: Vec<TripPhoto> = db
        .prepare(
            "SELECT p.id, p.item_id, p.filename, p.file_size, p.mime_type, p.created_at, p.storage_path
             FROM trip_photos p
             JOIN trip_items ti ON ti.id = p.item_id
             WHERE ti.trip_id = ?1
             ORDER BY p.item_id, p.created_at",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok(TripPhoto {
                    id: row.get(0)?,
                    item_id: row.get(1)?,
                    filename: row.get(2)?,
                    file_size: row.get(3)?,
                    mime_type: row.get(4)?,
                    created_at: row.get(5)?,
                    storage_path: row.get(6)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Group photos by item_id
    let mut photos_map: std::collections::HashMap<String, Vec<TripPhoto>> =
        std::collections::HashMap::new();
    for photo in all_photos {
        photos_map
            .entry(photo.item_id.clone())
            .or_default()
            .push(photo);
    }

    let items_with_photos: Vec<TripItemWithPhotos> = items
        .into_iter()
        .map(|item| {
            let photos = photos_map.remove(&item.id).unwrap_or_default();
            TripItemWithPhotos { item, photos }
        })
        .collect();

    // Get collaborators
    let collaborators: Vec<TripCollaborator> = db
        .prepare(
            "SELECT tc.user_id, COALESCE(u.display_name, u.username) as display_name, tc.role, tc.created_at
             FROM trip_collaborators tc
             JOIN users u ON u.id = tc.user_id
             WHERE tc.trip_id = ?1",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok(TripCollaborator {
                    user_id: row.get(0)?,
                    display_name: row.get(1)?,
                    role: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let detail = TripDetail {
        trip,
        items: items_with_photos,
        collaborators,
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "trip": detail })),
    )
}

// ===== Update trip =====
pub async fn update_trip(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
    Json(req): Json<UpdateTripRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, _) = check_trip_access(&db, &id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        );
    }
    if !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "只有创建者可以编辑行程" })),
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut sets = vec!["updated_at = ?1".to_string()];
    let mut idx = 2u32;

    macro_rules! maybe_set {
        ($field:expr, $name:expr) => {
            if $field.is_some() {
                sets.push(format!("{} = ?{}", $name, idx));
                idx += 1;
            }
        };
    }

    maybe_set!(req.title, "title");
    maybe_set!(req.destination, "destination");
    maybe_set!(req.date_from, "date_from");
    maybe_set!(req.date_to, "date_to");
    maybe_set!(req.purpose, "purpose");
    maybe_set!(req.notes, "notes");
    maybe_set!(req.currency, "currency");

    let sql = format!("UPDATE trips SET {} WHERE id = ?{}", sets.join(", "), idx);

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
    if let Some(v) = &req.title {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.destination {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.date_from {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.date_to {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.purpose {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.notes {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.currency {
        params.push(Box::new(v.clone()));
    }
    params.push(Box::new(id));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    match db.execute(&sql, param_refs.as_slice()) {
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => {
            eprintln!("[Trip] update error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "更新失败" })),
            )
        }
    }
}

// ===== Delete trip =====
pub async fn delete_trip(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, _) = check_trip_access(&db, &id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        );
    }
    if !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "只有创建者可以删除行程" })),
        );
    }

    // Collect photo paths for cleanup
    let photos: Vec<String> = db
        .prepare(
            "SELECT tp.storage_path FROM trip_photos tp
             JOIN trip_items ti ON ti.id = tp.item_id
             WHERE ti.trip_id = ?1",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    match db.execute("DELETE FROM trips WHERE id = ?1", rusqlite::params![id]) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        ),
        Ok(_) => {
            for path in photos {
                std::fs::remove_file(&path).ok();
            }
            (StatusCode::OK, Json(json!({ "success": true })))
        }
        Err(e) => {
            eprintln!("[Trip] delete error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "删除失败" })),
            )
        }
    }
}

// ===== Create item =====
pub async fn create_item(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(trip_id): Path<String>,
    Json(req): Json<CreateTripItemRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, role) = check_trip_access(&db, &trip_id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        );
    }
    if !is_owner && role != "editor" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "没有权限添加条目" })),
        );
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let description = req.description.unwrap_or_default();
    let amount = req.amount.unwrap_or(0.0);
    let currency = req.currency.unwrap_or_else(|| "CAD".to_string());
    let reimburse_status = req
        .reimburse_status
        .unwrap_or_else(|| "pending".to_string());
    let notes = req.notes.unwrap_or_default();

    // Get next sort_order
    let max_sort: i32 = db
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM trip_items WHERE trip_id = ?1 AND date = ?2",
            rusqlite::params![trip_id, req.date],
            |row| row.get(0),
        )
        .unwrap_or(0);

    match db.execute(
        "INSERT INTO trip_items (id, trip_id, type, date, description, amount, currency, reimburse_status, notes, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        rusqlite::params![id, trip_id, req.item_type, req.date, description, amount, currency, reimburse_status, notes, max_sort + 1, now],
    ) {
        Ok(_) => {
            let item = TripItem {
                id,
                trip_id,
                item_type: req.item_type,
                date: req.date,
                description,
                amount,
                currency,
                reimburse_status,
                notes,
                sort_order: max_sort + 1,
                created_at: now.clone(),
                updated_at: now,
                photo_count: 0,
            };
            (StatusCode::CREATED, Json(json!({ "success": true, "item": item })))
        }
        Err(e) => {
            eprintln!("[Trip] create_item error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "message": "创建条目失败" })))
        }
    }
}

// ===== Update item =====
pub async fn update_item(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(item_id): Path<String>,
    Json(req): Json<UpdateTripItemRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, role, _trip_id) = check_item_access(&db, &item_id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "条目不存在" })),
        );
    }

    // Editor can only update reimburse_status
    if !is_owner && role == "editor" {
        if req.item_type.is_some()
            || req.date.is_some()
            || req.description.is_some()
            || req.amount.is_some()
            || req.currency.is_some()
            || req.notes.is_some()
        {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "success": false, "message": "协作者只能更新报销状态" })),
            );
        }
    } else if !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "没有权限修改条目" })),
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut sets = vec!["updated_at = ?1".to_string()];
    let mut idx = 2u32;

    macro_rules! maybe_set {
        ($field:expr, $name:expr) => {
            if $field.is_some() {
                sets.push(format!("{} = ?{}", $name, idx));
                idx += 1;
            }
        };
    }

    maybe_set!(req.item_type, "type");
    maybe_set!(req.date, "date");
    maybe_set!(req.description, "description");
    maybe_set!(req.amount, "amount");
    maybe_set!(req.currency, "currency");
    maybe_set!(req.reimburse_status, "reimburse_status");
    maybe_set!(req.notes, "notes");

    if sets.len() == 1 {
        return (StatusCode::OK, Json(json!({ "success": true })));
    }

    let sql = format!(
        "UPDATE trip_items SET {} WHERE id = ?{}",
        sets.join(", "),
        idx
    );

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
    if let Some(v) = &req.item_type {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.date {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.description {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = req.amount {
        params.push(Box::new(v));
    }
    if let Some(v) = &req.currency {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.reimburse_status {
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &req.notes {
        params.push(Box::new(v.clone()));
    }
    params.push(Box::new(item_id));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    match db.execute(&sql, param_refs.as_slice()) {
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => {
            eprintln!("[Trip] update_item error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "更新失败" })),
            )
        }
    }
}

// ===== Delete item =====
pub async fn delete_item(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(item_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, _, _) = check_item_access(&db, &item_id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "条目不存在" })),
        );
    }
    if !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "只有创建者可以删除条目" })),
        );
    }

    // Collect photo paths
    let photos: Vec<String> = db
        .prepare("SELECT storage_path FROM trip_photos WHERE item_id = ?1")
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![item_id], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    match db.execute(
        "DELETE FROM trip_items WHERE id = ?1",
        rusqlite::params![item_id],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "条目不存在" })),
        ),
        Ok(_) => {
            for path in photos {
                std::fs::remove_file(&path).ok();
            }
            (StatusCode::OK, Json(json!({ "success": true })))
        }
        Err(e) => {
            eprintln!("[Trip] delete_item error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "删除失败" })),
            )
        }
    }
}

// ===== Upload item photos =====
pub async fn upload_item_photos(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(item_id): Path<String>,
    mut multipart: Multipart,
) -> (StatusCode, Json<serde_json::Value>) {
    // Check access
    let owner_user_id: String;
    {
        let db = state.db.lock();
        let (has_access, is_owner, role, _) = check_item_access(&db, &item_id, &user_id.0);
        if !has_access {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "条目不存在" })),
            );
        }
        if !is_owner && role != "editor" {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "success": false, "message": "没有权限上传照片" })),
            );
        }
        // Get trip owner for upload directory
        owner_user_id = db
            .query_row(
                "SELECT t.user_id FROM trips t JOIN trip_items ti ON ti.trip_id = t.id WHERE ti.id = ?1",
                rusqlite::params![item_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| user_id.0.clone());
    }

    let upload_dir = format!(
        "{}/uploads/{}",
        std::env::var("DATABASE_PATH")
            .unwrap_or_else(|_| "data/next.db".to_string())
            .replace("/next.db", "")
            .replace("\\next.db", ""),
        owner_user_id
    );
    std::fs::create_dir_all(&upload_dir).ok();

    let mut uploaded: Vec<TripPhoto> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field.file_name().unwrap_or("photo.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpeg").to_string();

        if !content_type.starts_with("image/") {
            continue;
        }

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(_) => continue,
        };

        if data.is_empty() || data.len() > 10_000_000 {
            continue;
        }

        let photo_id = uuid::Uuid::new_v4().to_string();
        let ext = filename.rsplit('.').next().unwrap_or("jpg").to_lowercase();
        let storage_name = format!("{}.{}", photo_id, ext);
        let storage_path = format!("{}/{}", upload_dir, storage_name);

        if std::fs::write(&storage_path, &data).is_err() {
            continue;
        }

        let now = chrono::Utc::now().to_rfc3339();
        let file_size = data.len() as i64;

        let db = state.db.lock();
        let result = db.execute(
            "INSERT INTO trip_photos (id, item_id, filename, storage_path, file_size, mime_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![photo_id, item_id, filename, storage_path, file_size, content_type, now],
        );

        if result.is_ok() {
            uploaded.push(TripPhoto {
                id: photo_id,
                item_id: item_id.clone(),
                filename,
                file_size,
                mime_type: content_type,
                created_at: now,
                storage_path,
            });
        }
    }

    (
        StatusCode::OK,
        Json(json!({ "success": true, "photos": uploaded, "count": uploaded.len() })),
    )
}

// ===== Delete photo =====
pub async fn delete_photo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(photo_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    // Get photo info + verify access
    let photo_info: Option<(String, String)> = db
        .query_row(
            "SELECT tp.storage_path, tp.item_id FROM trip_photos tp WHERE tp.id = ?1",
            rusqlite::params![photo_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let (path, item_id) = match photo_info {
        Some(info) => info,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "照片不存在" })),
            )
        }
    };

    let (has_access, is_owner, _, _) = check_item_access(&db, &item_id, &user_id.0);
    if !has_access || !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "只有创建者可以删除照片" })),
        );
    }

    db.execute(
        "DELETE FROM trip_photos WHERE id = ?1",
        rusqlite::params![photo_id],
    )
    .ok();
    std::fs::remove_file(&path).ok();

    (StatusCode::OK, Json(json!({ "success": true })))
}

// ===== Add collaborator =====
pub async fn add_collaborator(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(trip_id): Path<String>,
    Json(req): Json<AddCollaboratorRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, _) = check_trip_access(&db, &trip_id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        );
    }
    if !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "只有创建者可以添加协作者" })),
        );
    }

    // Verify friendship
    let is_friend: bool = db
        .query_row(
            "SELECT COUNT(*) FROM friendships
             WHERE status = 'accepted'
             AND ((requester_id = ?1 AND addressee_id = ?2) OR (requester_id = ?2 AND addressee_id = ?1))",
            rusqlite::params![user_id.0, req.friend_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !is_friend {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "只能添加好友为协作者" })),
        );
    }

    let role = if req.role == "editor" {
        "editor"
    } else {
        "viewer"
    };
    let now = chrono::Utc::now().to_rfc3339();

    match db.execute(
        "INSERT OR REPLACE INTO trip_collaborators (trip_id, user_id, role, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![trip_id, req.friend_id, role, now],
    ) {
        Ok(_) => {
            let display_name: String = db
                .query_row(
                    "SELECT COALESCE(display_name, username) FROM users WHERE id = ?1",
                    rusqlite::params![req.friend_id],
                    |row| row.get(0),
                )
                .unwrap_or_default();

            let collab = TripCollaborator {
                user_id: req.friend_id,
                display_name,
                role: role.to_string(),
                created_at: now,
            };
            (StatusCode::OK, Json(json!({ "success": true, "collaborator": collab })))
        }
        Err(e) => {
            eprintln!("[Trip] add_collaborator error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "message": "添加失败" })))
        }
    }
}

// ===== Remove collaborator =====
pub async fn remove_collaborator(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path((trip_id, uid)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    let (has_access, is_owner, _) = check_trip_access(&db, &trip_id, &user_id.0);
    if !has_access {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "行程不存在" })),
        );
    }
    if !is_owner {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "message": "只有创建者可以移除协作者" })),
        );
    }

    match db.execute(
        "DELETE FROM trip_collaborators WHERE trip_id = ?1 AND user_id = ?2",
        rusqlite::params![trip_id, uid],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "协作者不存在" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => {
            eprintln!("[Trip] remove_collaborator error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "移除失败" })),
            )
        }
    }
}

// ===== Shared xlsx builder =====
fn build_xlsx_buffer(db: &rusqlite::Connection, trip_id: &str) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, Workbook};

    let rows: Vec<(String, String, String, f64, String, String, String)> = db
        .prepare(
            "SELECT date, type, description, amount, currency, reimburse_status, notes
             FROM trip_items WHERE trip_id = ?1 AND amount > 0 ORDER BY date, sort_order",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![trip_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    let bold = Format::new().set_bold();
    let headers = ["日期", "类型", "描述", "金额", "币种", "报销状态", "备注"];
    for (col, h) in headers.iter().enumerate() {
        sheet.write_with_format(0, col as u16, *h, &bold).ok();
    }

    sheet.set_column_width(0, 12).ok();
    sheet.set_column_width(2, 32).ok();
    sheet.set_column_width(6, 20).ok();

    for (row_idx, r) in rows.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        let type_label = match r.1.as_str() {
            "flight" => "机票",
            "train" => "火车",
            "hotel" => "酒店",
            "taxi" => "出租/打车",
            "meal" => "餐饮",
            "meeting" => "会议",
            "telecom" => "通讯",
            _ => "其他",
        };
        let status_label = match r.5.as_str() {
            "pending" => "待提交",
            "submitted" => "已提交",
            "approved" => "已批准",
            "rejected" => "已拒绝",
            "na" => "无需报销",
            _ => r.5.as_str(),
        };
        sheet.write(row, 0, r.0.as_str()).ok();
        sheet.write(row, 1, type_label).ok();
        sheet.write(row, 2, r.2.as_str()).ok();
        sheet.write(row, 3, r.3).ok();
        sheet.write(row, 4, r.4.as_str()).ok();
        sheet.write(row, 5, status_label).ok();
        sheet.write(row, 6, r.6.as_str()).ok();
    }

    workbook
        .save_to_buffer()
        .map_err(|e| format!("xlsx error: {}", e))
}

// ===== Export XLSX =====
pub async fn export_xlsx(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let db = state.db.lock();

    let (has_access, _, _) = check_trip_access(&db, &id, &user_id.0);
    if !has_access {
        return axum::response::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(axum::body::Body::from("行程不存在"))
            .unwrap();
    }

    let title: String = db
        .query_row(
            "SELECT title FROM trips WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "trip".to_string());

    let xlsx_data = match build_xlsx_buffer(&db, &id) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[Trip] {}", e);
            return axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::from("生成失败"))
                .unwrap();
        }
    };

    let filename = format!("{}-报销清单.xlsx", title);
    let encoded = urlencoding::encode(&filename).into_owned();
    let disposition = format!("attachment; filename*=UTF-8''{}", encoded);

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(http::header::CONTENT_DISPOSITION, disposition)
        .body(axum::body::Body::from(xlsx_data))
        .unwrap()
}

// ===== Export photos zip =====
pub async fn export_photos(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let db = state.db.lock();

    let (has_access, _, _) = check_trip_access(&db, &id, &user_id.0);
    if !has_access {
        return (StatusCode::NOT_FOUND, "行程不存在").into_response();
    }

    // Collect all photos with item info (storage_path, filename, date, description)
    let photos: Vec<(String, String, String, String)> = db
        .prepare(
            "SELECT tp.storage_path, tp.filename, ti.date, COALESCE(ti.description, '')
             FROM trip_photos tp
             JOIN trip_items ti ON ti.id = tp.item_id
             WHERE ti.trip_id = ?1 ORDER BY ti.date, ti.sort_order, tp.created_at",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    if photos.is_empty() {
        return (StatusCode::OK, "没有票据照片").into_response();
    }

    // Create zip in memory, organized by date folder
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Track per-date counters for duplicate handling
        let mut date_counters: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (path, orig_filename, date, desc) in &photos {
            if let Ok(data) = std::fs::read(path) {
                let ext = orig_filename.rsplit('.').next().unwrap_or("jpg");
                // Sanitize description for filename (keep CJK + alphanumeric + basic punctuation)
                let clean_desc: String = desc
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>()
                    .trim_matches('_')
                    .chars()
                    .take(40)
                    .collect();

                let counter = date_counters.entry(date.clone()).or_insert(0);
                *counter += 1;

                let name_in_zip = if clean_desc.is_empty() {
                    format!("{}/{:02}.{}", date, counter, ext)
                } else {
                    format!("{}/{:02}_{}.{}", date, counter, clean_desc, ext)
                };

                zip.start_file(&name_in_zip, options).ok();
                use std::io::Write;
                zip.write_all(&data).ok();
            }
        }
        zip.finish().ok();
    }

    let title: String = db
        .query_row(
            "SELECT title FROM trips WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "trip".to_string());

    let filename = format!("{}-票据.zip", title);
    let encoded = urlencoding::encode(&filename);

    (
        StatusCode::OK,
        [
            (http::header::CONTENT_TYPE, "application/zip"),
            (
                http::header::CONTENT_DISPOSITION,
                &format!("attachment; filename*=UTF-8''{}", encoded),
            ),
        ],
        axum::body::Body::from(buf.into_inner()),
    )
        .into_response()
}

// ===== Export bundle (xlsx + photos zip) =====

/// Sanitize a folder name for cross-platform zip compatibility
fn sanitize_folder_name(date: &str, description: &str) -> String {
    let desc = if description.trim().is_empty() {
        "未命名".to_string()
    } else {
        description
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                _ => c,
            })
            .collect::<String>()
            .trim()
            .to_string()
    };
    let raw = format!("{} - {}", date, desc);
    raw.chars().take(80).collect()
}

pub async fn export_bundle(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let db = state.db.lock();

    let (has_access, _, _) = check_trip_access(&db, &id, &user_id.0);
    if !has_access {
        return (StatusCode::NOT_FOUND, "行程不存在").into_response();
    }

    let title: String = db
        .query_row(
            "SELECT title FROM trips WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "trip".to_string());

    // Build xlsx
    let xlsx_data = match build_xlsx_buffer(&db, &id) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[Trip] bundle: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "生成失败").into_response();
        }
    };

    // Collect photos grouped by item: (item_id, date, description, storage_path, filename)
    let photos: Vec<(String, String, String, String, String)> = db
        .prepare(
            "SELECT ti.id, ti.date, COALESCE(ti.description, ''), tp.storage_path, tp.filename
             FROM trip_photos tp
             JOIN trip_items ti ON ti.id = tp.item_id
             WHERE ti.trip_id = ?1 ORDER BY ti.date, ti.sort_order, tp.created_at",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Build zip
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 1. Add xlsx at root
        let xlsx_name = format!("{}-报销清单.xlsx", title);
        zip.start_file(&xlsx_name, options).ok();
        use std::io::Write;
        zip.write_all(&xlsx_data).ok();

        // 2. Add photos grouped by item
        if !photos.is_empty() {
            // Build folder names, handling duplicates
            let mut folder_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new(); // item_id -> folder_name
            let mut used_names: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();

            // Collect unique items in order
            let mut item_order: Vec<String> = Vec::new();
            let mut item_info: std::collections::HashMap<String, (String, String)> =
                std::collections::HashMap::new(); // item_id -> (date, desc)

            for (item_id, date, desc, _, _) in &photos {
                if !item_info.contains_key(item_id) {
                    item_order.push(item_id.clone());
                    item_info.insert(item_id.clone(), (date.clone(), desc.clone()));
                }
            }

            for item_id in &item_order {
                let (date, desc) = &item_info[item_id];
                let base = sanitize_folder_name(date, desc);
                let count = used_names.entry(base.clone()).or_insert(0);
                *count += 1;
                let folder = if *count == 1 {
                    base
                } else {
                    format!("{} ({})", base, count)
                };
                folder_map.insert(item_id.clone(), folder);
            }

            // Track per-item photo counters
            let mut item_counters: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();

            for (item_id, _, _, path, orig_filename) in &photos {
                if let Ok(data) = std::fs::read(path) {
                    let ext = orig_filename.rsplit('.').next().unwrap_or("jpg");
                    let counter = item_counters.entry(item_id.clone()).or_insert(0);
                    *counter += 1;
                    let folder = &folder_map[item_id];
                    let name_in_zip = format!("{}/{:02}.{}", folder, counter, ext);
                    zip.start_file(&name_in_zip, options).ok();
                    zip.write_all(&data).ok();
                }
            }
        }

        zip.finish().ok();
    }

    let filename = format!("{}-报销材料.zip", title);
    let encoded = urlencoding::encode(&filename);

    (
        StatusCode::OK,
        [
            (http::header::CONTENT_TYPE, "application/zip"),
            (
                http::header::CONTENT_DISPOSITION,
                &format!("attachment; filename*=UTF-8''{}", encoded),
            ),
        ],
        axum::body::Body::from(buf.into_inner()),
    )
        .into_response()
}

// ===== AI Item Analysis =====

#[derive(Deserialize)]
pub struct AnalyzeImageData {
    pub data: String,
    pub mime_type: String,
}

#[derive(Deserialize)]
pub struct AnalyzeItemRequest {
    #[serde(default)]
    pub images: Vec<AnalyzeImageData>,
    pub text: Option<String>,
}

/// POST /api/trips/analyze
/// 图片(base64) + 可选文字 → AI提取差旅条目数组
/// 一图多事件→拆分多条；多图同一事件→合并一条
pub async fn analyze_item(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    headers: http::HeaderMap,
    Json(req): Json<AnalyzeItemRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Guest AI quota check (T-089: IP aggregate)
    let client_ip = extract_client_ip(&headers);
    let guest_ai_remaining = match check_guest_ai_quota(&state, &user_id.0, &client_ip) {
        Ok(r) => r,
        Err(e) => return e,
    };

    let has_images = !req.images.is_empty();
    let text = req.text.as_deref().unwrap_or("").trim().to_string();
    let has_text = !text.is_empty();

    if !has_images && !has_text {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "请提供票据照片或行程文字" })),
        );
    }

    let client = match LlmClient::for_user(&state.db.lock(), &user_id.0) {
        Some(c) => c,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "AI服务未配置" })),
            )
        }
    };

    let system = r#"你是差旅费用提取助手。从票据照片或文字中提取差旅费用信息，以JSON数组格式返回。

规则：
- 一张图/一段文字可能含多个差旅事件，请拆分成多条（例如机票+行李费→两条）
- 多张图片描述同一件事时，合并成一条
- type 只能是：flight, train, hotel, taxi, meal, meeting, telecom, misc
- date 格式 YYYY-MM-DD，不确定则留 null
- amount 为数字，不确定则留 null
- currency 默认 CAD，明显是人民币用 CNY
- description 简短精确，如"北京→上海 CA1234"、"Marriott 2晚"
- notes 放补充信息（订单号、发票号等）

只返回 JSON 数组，不要其他文字。"#;

    let user_message = match (has_images, has_text) {
        (true, true) => format!(
            "请分析这些票据照片，同时结合以下文字信息，提取所有差旅条目：\n\n{}",
            text
        ),
        (true, false) => "请分析这些票据照片，提取所有差旅条目。".to_string(),
        (false, true) => format!("请分析以下行程信息，提取所有差旅条目：\n\n{}", text),
        (false, false) => unreachable!(), // already guarded above
    };

    let images: Vec<(String, String)> = req
        .images
        .iter()
        .map(|img| (img.data.clone(), img.mime_type.clone()))
        .collect();

    let result = if has_images {
        client
            .vision_generate(system, images, &user_message, 4096)
            .await
    } else {
        client.simple_generate(system, &user_message, 4096).await
    };
    match result {
        Ok(raw) => {
            // Try array first, then single object wrapped in array
            let json_str = if let (Some(s), Some(e)) = (raw.find('['), raw.rfind(']')) {
                raw[s..=e].to_string()
            } else if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
                format!("[{}]", &raw[s..=e])
            } else {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "success": false,
                        "message": "AI未返回结构化数据，请手动填写",
                        "raw": raw
                    })),
                );
            };

            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(items) => {
                    let mut resp = json!({ "success": true, "items": items });
                    if guest_ai_remaining < 999 {
                        resp["ai_remaining"] = json!(guest_ai_remaining);
                    }
                    (StatusCode::OK, Json(resp))
                }
                Err(_) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": false,
                        "message": "解析失败，请手动填写",
                        "raw": raw
                    })),
                ),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e })),
        ),
    }
}
