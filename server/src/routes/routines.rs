use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::Connection;
use serde::Serialize;

use crate::auth::{ActiveUserId, UserId};
use crate::models::routine::*;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct RoutinesResponse {
    pub success: bool,
    pub items: Vec<Routine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RoutineResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Routine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Ensure collaboration tables exist (idempotent)
fn ensure_collab_tables(db: &Connection) {
    db.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS routine_collaborators (
            id TEXT PRIMARY KEY,
            routine_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            UNIQUE(routine_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_routine_collab_user ON routine_collaborators(user_id, status);
        CREATE INDEX IF NOT EXISTS idx_routine_collab_routine ON routine_collaborators(routine_id);
        CREATE TABLE IF NOT EXISTS routine_completions (
            id TEXT PRIMARY KEY,
            routine_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            completed_date TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(routine_id, user_id, completed_date)
        );
        CREATE INDEX IF NOT EXISTS idx_routine_comp_lookup ON routine_completions(routine_id, user_id, completed_date);
        ",
    )
    .ok();

    let has_collab_col: bool = db
        .prepare("SELECT is_collaborative FROM routines LIMIT 0")
        .is_ok();
    if !has_collab_col {
        db.execute_batch("ALTER TABLE routines ADD COLUMN is_collaborative INTEGER DEFAULT 0;")
            .ok();
    }
}

fn get_user_display_name(db: &Connection, uid: &str) -> Option<String> {
    db.query_row(
        "SELECT COALESCE(display_name, username) FROM users WHERE id = ?1",
        [uid],
        |r| r.get(0),
    )
    .ok()
}

pub async fn list_routines(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<RoutinesResponse>) {
    let db = state.db.lock();
    ensure_collab_tables(&db);
    let tz = chrono::FixedOffset::east_opt(8 * 3600).unwrap(); // UTC+8
    let today = chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string();

    let mut items: Vec<Routine> = Vec::new();

    if let Ok(mut stmt) = db.prepare(
        "SELECT id, text, completed_today, last_completed_date, created_at, COALESCE(is_collaborative, 0) FROM routines WHERE user_id = ?1",
    ) {
        if let Ok(rows) = stmt.query_map([&user_id.0], |row| {
            let completed_int: i32 = row.get(2)?;
            let last_date: Option<String> = row.get(3)?;
            let is_collab_int: i32 = row.get(5)?;

            let completed_today = if completed_int != 0 {
                match &last_date {
                    Some(d) => d == &today,
                    None => false,
                }
            } else {
                false
            };

            let is_collaborative = if is_collab_int != 0 {
                Some(true)
            } else {
                None
            };

            Ok(Routine {
                id: row.get(0)?,
                text: row.get(1)?,
                completed_today,
                last_completed_date: last_date,
                created_at: row.get(4)?,
                is_collaborative,
                owner_name: None,
                owner_id: None,
            })
        }) {
            for r in rows.flatten() {
                items.push(r);
            }
        }
    }

    // Collaborative routines (where user is collaborator, not owner)
    if let Ok(mut stmt) = db.prepare(
        "SELECT r.id, r.text, r.created_at, r.user_id as owner_id
         FROM routines r
         JOIN routine_collaborators rc ON r.id = rc.routine_id
         WHERE rc.user_id = ?1 AND rc.status = 'active'",
    ) {
        if let Ok(rows) = stmt.query_map([&user_id.0], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        }) {
            for r in rows.flatten() {
                let (rid, text, created_at, owner_id) = r;

                let completed_today: bool = db
                    .query_row(
                        "SELECT COUNT(*) > 0 FROM routine_completions WHERE routine_id = ?1 AND user_id = ?2 AND completed_date = ?3",
                        rusqlite::params![rid, user_id.0, today],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);

                let owner_name = get_user_display_name(&db, &owner_id);

                items.push(Routine {
                    id: rid,
                    text,
                    completed_today,
                    last_completed_date: if completed_today {
                        Some(today.clone())
                    } else {
                        None
                    },
                    created_at,
                    is_collaborative: Some(true),
                    owner_name,
                    owner_id: Some(owner_id),
                });
            }
        }
    }

    (
        StatusCode::OK,
        Json(RoutinesResponse {
            success: true,
            items,
            message: None,
        }),
    )
}

pub async fn create_routine(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<CreateRoutineRequest>,
) -> (StatusCode, Json<RoutineResponse>) {
    let db = state.db.lock();
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();

    if let Err(e) = db.execute(
        "INSERT INTO routines (id, user_id, text, completed_today, last_completed_date, created_at) VALUES (?1,?2,?3,0,NULL,?4)",
        rusqlite::params![id, user_id.0, req.text, now],
    ) {
        eprintln!("[routines] create insert failed: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RoutineResponse {
                success: false,
                item: None,
                message: Some("创建失败".into()),
            }),
        );
    }

    let routine = Routine {
        id,
        text: req.text,
        completed_today: false,
        last_completed_date: None,
        created_at: now,
        is_collaborative: None,
        owner_name: None,
        owner_id: None,
    };

    (
        StatusCode::OK,
        Json(RoutineResponse {
            success: true,
            item: Some(routine),
            message: Some("日常任务创建成功".into()),
        }),
    )
}

pub async fn toggle_routine(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<RoutineResponse>) {
    let db = state.db.lock();
    ensure_collab_tables(&db);
    let tz = chrono::FixedOffset::east_opt(8 * 3600).unwrap(); // UTC+8
    let today = chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let is_owner: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM routines WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);

    let is_collaborator: bool = if !is_owner {
        db.query_row(
            "SELECT COUNT(*) > 0 FROM routine_collaborators WHERE routine_id = ?1 AND user_id = ?2 AND status = 'active'",
            rusqlite::params![id, user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false)
    } else {
        false
    };

    if !is_owner && !is_collaborator {
        return (
            StatusCode::NOT_FOUND,
            Json(RoutineResponse {
                success: false,
                item: None,
                message: Some(format!("日常任务不存在: {}", id)),
            }),
        );
    }

    if is_collaborator {
        let already_done: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM routine_completions WHERE routine_id=?1 AND user_id=?2 AND completed_date=?3",
                rusqlite::params![id, user_id.0, today],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if already_done {
            db.execute(
                "DELETE FROM routine_completions WHERE routine_id=?1 AND user_id=?2 AND completed_date=?3",
                rusqlite::params![id, user_id.0, today],
            ).ok();
        } else {
            let comp_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            db.execute(
                "INSERT INTO routine_completions (id, routine_id, user_id, completed_date, created_at) VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![comp_id, id, user_id.0, today, now],
            ).ok();
        }

        let completed_today = !already_done;
        let text: String = db
            .query_row(
                "SELECT text FROM routines WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let oid: String = db
            .query_row(
                "SELECT user_id FROM routines WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let oname = get_user_display_name(&db, &oid);

        let routine = Routine {
            id,
            text,
            completed_today,
            last_completed_date: if completed_today { Some(today) } else { None },
            created_at: String::new(),
            is_collaborative: Some(true),
            owner_name: oname,
            owner_id: Some(oid),
        };

        let message = if completed_today {
            "已完成"
        } else {
            "已取消完成"
        };
        return (
            StatusCode::OK,
            Json(RoutineResponse {
                success: true,
                item: Some(routine),
                message: Some(message.into()),
            }),
        );
    }

    // Owner path
    let result = db.query_row(
        "SELECT id, text, completed_today, last_completed_date, created_at, COALESCE(is_collaborative, 0) FROM routines WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id.0],
        |row| {
            let completed_int: i32 = row.get(2)?;
            let last_date: Option<String> = row.get(3)?;
            let is_collab_int: i32 = row.get(5)?;
            let completed_today = if completed_int != 0 {
                match &last_date {
                    Some(d) => d == &today,
                    None => false,
                }
            } else {
                false
            };
            Ok(Routine {
                id: row.get(0)?,
                text: row.get(1)?,
                completed_today,
                last_completed_date: last_date,
                created_at: row.get(4)?,
                is_collaborative: if is_collab_int != 0 { Some(true) } else { None },
                owner_name: None,
                owner_id: None,
            })
        },
    );

    let mut routine = match result {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(RoutineResponse {
                    success: false,
                    item: None,
                    message: Some(format!("日常任务不存在: {}", id)),
                }),
            )
        }
    };

    routine.completed_today = !routine.completed_today;
    if routine.completed_today {
        routine.last_completed_date = Some(today);
    } else {
        routine.last_completed_date = None;
    }

    if let Err(e) = db.execute(
        "UPDATE routines SET completed_today = ?1, last_completed_date = ?2 WHERE id = ?3",
        rusqlite::params![
            routine.completed_today as i32,
            routine.last_completed_date,
            id
        ],
    ) {
        eprintln!("[routines] toggle update failed: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RoutineResponse {
                success: false,
                item: None,
                message: Some("更新失败".into()),
            }),
        );
    }

    let message = if routine.completed_today {
        "已完成"
    } else {
        "已取消完成"
    };

    (
        StatusCode::OK,
        Json(RoutineResponse {
            success: true,
            item: Some(routine),
            message: Some(message.into()),
        }),
    )
}

pub async fn delete_routine(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    let rows = db
        .execute(
            "DELETE FROM routines WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some(format!("日常任务不存在: {}", id)),
            }),
        );
    }

    // Clean up collaborators and completions
    db.execute(
        "DELETE FROM routine_collaborators WHERE routine_id = ?1",
        rusqlite::params![id],
    )
    .ok();
    db.execute(
        "DELETE FROM routine_completions WHERE routine_id = ?1",
        rusqlite::params![id],
    )
    .ok();

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("日常任务已删除".into()),
        }),
    )
}
