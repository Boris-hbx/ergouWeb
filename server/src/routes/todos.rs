use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use serde_json::json;

use crate::auth::{ActiveUserId, UserId};
use crate::models::todo::*;
use crate::services::collaboration;
use crate::state::AppState;

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<serde_json::Value>) {
    eprintln!("[{}] db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"success": false, "error": "内部错误"})),
    )
}

#[derive(Debug, Serialize)]
pub struct TodosResponse {
    pub success: bool,
    pub items: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TodoResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Todo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub tab: Option<String>,
}

/// Load the next pending/triggered reminder for a todo
fn load_next_reminder(
    db: &rusqlite::Connection,
    todo_id: &str,
    user_id: &str,
) -> Option<TodoReminder> {
    db.query_row(
        "SELECT id, remind_at, status FROM reminders \
         WHERE related_todo_id = ?1 AND user_id = ?2 AND status IN ('pending', 'triggered') \
         ORDER BY CASE status WHEN 'triggered' THEN 0 ELSE 1 END, remind_at ASC LIMIT 1",
        rusqlite::params![todo_id, user_id],
        |row| {
            Ok(TodoReminder {
                id: row.get(0)?,
                remind_at: row.get(1)?,
                status: row.get(2)?,
            })
        },
    )
    .ok()
}

/// Load changelog for a todo from the database
fn load_changelog(db: &rusqlite::Connection, todo_id: &str) -> Vec<ChangeEntry> {
    let mut stmt = match db.prepare(
        "SELECT field, label, from_val, to_val, time FROM todo_changelog WHERE todo_id = ?1 ORDER BY id DESC LIMIT 50",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[todos] load_changelog prepare error: {}", e);
            return Vec::new();
        }
    };
    let result = stmt.query_map([todo_id], |row| {
        Ok(ChangeEntry {
            field: row.get(0)?,
            label: row.get(1)?,
            old_value: row.get(2).unwrap_or_default(),
            new_value: row.get(3).unwrap_or_default(),
            timestamp: row.get(4)?,
        })
    });
    match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[todos] load_changelog query error: {}", e);
            Vec::new()
        }
    }
}

/// Build a Todo from a database row
fn row_to_todo(row: &rusqlite::Row) -> rusqlite::Result<Todo> {
    let tags_json: String = row.get(11)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let completed_int: i32 = row.get(7)?;
    let deleted_int: i32 = row.get(9)?;

    Ok(Todo {
        id: row.get(0)?,
        text: row.get(1)?,
        content: row.get(2).unwrap_or_default(),
        tab: Tab::parse(&row.get::<_, String>(3)?),
        quadrant: Quadrant::parse(&row.get::<_, String>(4)?),
        progress: row.get::<_, i32>(5)? as u8,
        completed: completed_int != 0,
        completed_at: row.get(6)?,
        due_date: row.get(8)?,
        deleted: deleted_int != 0,
        assignee: row.get(10).unwrap_or_default(),
        tags,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        deleted_at: row.get(14)?,
        changelog: Vec::new(), // loaded separately
        next_reminder: None,   // loaded separately
    })
}

pub async fn list_todos(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let db = state.db.lock();

    // Helper macro for prepare+query with error handling
    macro_rules! query_todos {
        ($db:expr, $sql:expr, $params:expr, $ctx:expr) => {{
            let mut stmt = match $db.prepare($sql) {
                Ok(s) => s,
                Err(e) => return db_error($ctx, e).into_response(),
            };
            let result = stmt.query_map($params, row_to_todo);
            match result {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect::<Vec<Todo>>(),
                Err(e) => return db_error($ctx, e).into_response(),
            }
        }};
    }

    // Own todos
    let mut items: Vec<Todo> = if let Some(tab) = &query.tab {
        query_todos!(
            db,
            "SELECT id, text, content, tab, quadrant, progress, completed_at, completed, due_date, deleted, assignee, tags, created_at, updated_at, deleted_at FROM todos WHERE user_id = ?1 AND tab = ?2 AND deleted = 0 ORDER BY completed ASC, created_at ASC",
            rusqlite::params![user_id.0, tab],
            "todos/list"
        )
    } else {
        query_todos!(
            db,
            "SELECT id, text, content, tab, quadrant, progress, completed_at, completed, due_date, deleted, assignee, tags, created_at, updated_at, deleted_at FROM todos WHERE user_id = ?1 AND deleted = 0 ORDER BY completed ASC, created_at ASC",
            [&user_id.0],
            "todos/list"
        )
    };

    // Collaborative todos (from todo_collaborators) - use collaborator view settings
    let collab_sql_tab = "SELECT t.id, t.text, t.content, tc.tab, tc.quadrant, t.progress, t.completed_at, t.completed, t.due_date, t.deleted, t.assignee, t.tags, t.created_at, t.updated_at, t.deleted_at FROM todos t JOIN todo_collaborators tc ON t.id = tc.todo_id WHERE tc.user_id = ?1 AND tc.status = 'active' AND t.deleted = 0 AND tc.tab = ?2 ORDER BY t.completed ASC, t.created_at ASC";
    let collab_sql_all = "SELECT t.id, t.text, t.content, tc.tab, tc.quadrant, t.progress, t.completed_at, t.completed, t.due_date, t.deleted, t.assignee, t.tags, t.created_at, t.updated_at, t.deleted_at FROM todos t JOIN todo_collaborators tc ON t.id = tc.todo_id WHERE tc.user_id = ?1 AND tc.status = 'active' AND t.deleted = 0 ORDER BY t.completed ASC, t.created_at ASC";

    let collab_items: Vec<Todo> = if let Some(tab) = &query.tab {
        query_todos!(
            db,
            collab_sql_tab,
            rusqlite::params![user_id.0, tab],
            "todos/list-collab"
        )
    } else {
        query_todos!(db, collab_sql_all, [&user_id.0], "todos/list-collab")
    };

    // Merge and deduplicate (own todos take priority)
    let own_ids: std::collections::HashSet<String> = items.iter().map(|t| t.id.clone()).collect();
    for ct in collab_items {
        if !own_ids.contains(&ct.id) {
            items.push(ct);
        }
    }

    // Load changelogs and next reminders
    for todo in &mut items {
        todo.changelog = load_changelog(&db, &todo.id);
        todo.next_reminder = load_next_reminder(&db, &todo.id, &user_id.0);
    }

    // Enrich with collaboration info
    let mut enriched_items: Vec<serde_json::Value> = Vec::new();
    for todo in &items {
        let mut val = serde_json::to_value(todo).unwrap();
        if let Some(info) = collaboration::get_collab_info(&db, &todo.id, &user_id.0) {
            val["is_collaborative"] = json!(true);
            val["collaborator_name"] = json!(info.collaborator_name);
            val["my_role"] = json!(info.my_role);
        }
        enriched_items.push(val);
    }

    (
        StatusCode::OK,
        Json(TodosResponse {
            success: true,
            items: enriched_items,
            message: None,
        }),
    )
        .into_response()
}

pub async fn get_todo(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<TodoResponse>) {
    let db = state.db.lock();

    // Try owner first, then collaborator
    let result = db.query_row(
        "SELECT id, text, content, tab, quadrant, progress, completed_at, completed, due_date, deleted, assignee, tags, created_at, updated_at, deleted_at FROM todos WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id.0],
        row_to_todo,
    );

    let result = match result {
        Ok(todo) => Ok(todo),
        Err(_) => {
            db.query_row(
                "SELECT t.id, t.text, t.content, tc.tab, tc.quadrant, t.progress, t.completed_at, t.completed, t.due_date, t.deleted, t.assignee, t.tags, t.created_at, t.updated_at, t.deleted_at FROM todos t JOIN todo_collaborators tc ON t.id = tc.todo_id WHERE t.id = ?1 AND tc.user_id = ?2 AND tc.status = 'active' AND t.deleted = 0",
                rusqlite::params![id, user_id.0],
                row_to_todo,
            )
        }
    };

    match result {
        Ok(mut todo) => {
            todo.changelog = load_changelog(&db, &todo.id);
            (
                StatusCode::OK,
                Json(TodoResponse {
                    success: true,
                    item: Some(todo),
                    message: None,
                }),
            )
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(TodoResponse {
                success: false,
                item: None,
                message: Some(format!("任务不存在: {}", id)),
            }),
        ),
    }
}

pub async fn create_todo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<CreateTodoRequest>,
) -> (StatusCode, Json<TodoResponse>) {
    // Input length validation
    if req.text.len() > 500 {
        return (
            StatusCode::BAD_REQUEST,
            Json(TodoResponse {
                success: false,
                item: None,
                message: Some("任务标题不能超过 500 字符".into()),
            }),
        );
    }
    if req.content.as_ref().map(|c| c.len()).unwrap_or(0) > 10000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(TodoResponse {
                success: false,
                item: None,
                message: Some("任务内容不能超过 10000 字符".into()),
            }),
        );
    }

    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();
    let id = Todo::generate_id();
    let progress = req.progress.unwrap_or(0).min(100);
    let completed = progress == 100;
    let completed_at = if completed { Some(now.clone()) } else { None };
    let tags = req.tags.unwrap_or_default();
    let tags_json = serde_json::to_string(&tags).unwrap();

    if let Err(e) = db.execute(
        "INSERT INTO todos (id, user_id, text, content, tab, quadrant, progress, completed, completed_at, due_date, assignee, tags, created_at, updated_at, deleted, deleted_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,0,NULL)",
        rusqlite::params![
            id,
            user_id.0,
            req.text,
            req.content.as_deref().unwrap_or(""),
            req.tab,
            req.quadrant,
            progress as i32,
            completed as i32,
            completed_at,
            req.due_date,
            req.assignee.as_deref().unwrap_or(""),
            tags_json,
            now,
            now,
        ],
    ) {
        eprintln!("[todos] create_todo DB error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(TodoResponse {
            success: false, item: None,
            message: Some("数据库写入失败，请稍后重试".into()),
        }));
    }

    let todo = Todo {
        id,
        text: req.text,
        content: req.content.unwrap_or_default(),
        tab: Tab::parse(&req.tab),
        quadrant: Quadrant::parse(&req.quadrant),
        progress,
        completed,
        completed_at,
        due_date: req.due_date,
        assignee: req.assignee.unwrap_or_default(),
        tags,
        created_at: now.clone(),
        updated_at: now,
        changelog: Vec::new(),
        deleted: false,
        deleted_at: None,
        next_reminder: None,
    };

    (
        StatusCode::OK,
        Json(TodoResponse {
            success: true,
            item: Some(todo),
            message: Some("任务创建成功".into()),
        }),
    )
}

pub async fn update_todo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
    Json(update): Json<TodoUpdate>,
) -> (StatusCode, Json<TodoResponse>) {
    let db = state.db.lock();

    // Check role: owner or collaborator
    let is_collaborator = !collaboration::check_todo_owner(&db, &id, &user_id.0)
        && collaboration::check_todo_collaborator(&db, &id, &user_id.0);

    let current = if is_collaborator {
        db.query_row(
            "SELECT t.id, t.text, t.content, tc.tab, tc.quadrant, t.progress, t.completed_at, t.completed, t.due_date, t.deleted, t.assignee, t.tags, t.created_at, t.updated_at, t.deleted_at FROM todos t JOIN todo_collaborators tc ON t.id = tc.todo_id WHERE t.id = ?1 AND tc.user_id = ?2 AND tc.status = 'active'",
            rusqlite::params![id, user_id.0],
            row_to_todo,
        )
    } else {
        db.query_row(
            "SELECT id, text, content, tab, quadrant, progress, completed_at, completed, due_date, deleted, assignee, tags, created_at, updated_at, deleted_at FROM todos WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
            row_to_todo,
        )
    };

    let mut todo = match current {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(TodoResponse {
                    success: false,
                    item: None,
                    message: Some(format!("任务不存在: {}", id)),
                }),
            )
        }
    };

    let now = chrono::Utc::now().to_rfc3339();

    // Track changes and apply updates
    if let Some(text) = &update.text {
        if *text != todo.text {
            insert_changelog(
                &db,
                &id,
                "text",
                Todo::field_label("text"),
                &todo.text,
                text,
                &now,
            );
            todo.text = text.clone();
        }
    }
    if let Some(content) = &update.content {
        if *content != todo.content {
            insert_changelog(
                &db,
                &id,
                "content",
                Todo::field_label("content"),
                "(已更新)",
                "(已更新)",
                &now,
            );
            todo.content = content.clone();
        }
    }
    if let Some(tab_str) = &update.tab {
        let old_tab = todo.tab.as_str().to_string();
        if *tab_str != old_tab {
            insert_changelog(
                &db,
                &id,
                "tab",
                Todo::field_label("tab"),
                &old_tab,
                tab_str,
                &now,
            );
            todo.tab = Tab::parse(tab_str);
        }
    }
    if let Some(q_str) = &update.quadrant {
        let old_q = todo.quadrant.as_str().to_string();
        if *q_str != old_q {
            let old_label = todo.quadrant.label();
            let new_q = Quadrant::parse(q_str);
            let new_label = new_q.label();
            insert_changelog(
                &db,
                &id,
                "quadrant",
                Todo::field_label("quadrant"),
                old_label,
                new_label,
                &now,
            );
            todo.quadrant = new_q;
        }
    }
    if let Some(progress) = update.progress {
        let new_progress = progress.min(100);
        if new_progress != todo.progress {
            insert_changelog(
                &db,
                &id,
                "progress",
                Todo::field_label("progress"),
                &todo.progress.to_string(),
                &new_progress.to_string(),
                &now,
            );
            todo.progress = new_progress;
            if new_progress == 100 && !todo.completed {
                insert_changelog(
                    &db,
                    &id,
                    "completed",
                    Todo::field_label("completed"),
                    "未完成",
                    "已完成",
                    &now,
                );
                todo.completed = true;
                todo.completed_at = Some(now.clone());
            }
        }
    }
    if let Some(completed) = update.completed {
        if completed != todo.completed {
            insert_changelog(
                &db,
                &id,
                "completed",
                Todo::field_label("completed"),
                if todo.completed {
                    "已完成"
                } else {
                    "未完成"
                },
                if completed { "已完成" } else { "未完成" },
                &now,
            );
            todo.completed = completed;
            todo.completed_at = if completed { Some(now.clone()) } else { None };
        }
    }
    if let Some(due_date) = &update.due_date {
        let old = todo.due_date.clone().unwrap_or_default();
        if *due_date != old {
            insert_changelog(
                &db,
                &id,
                "due_date",
                Todo::field_label("due_date"),
                &old,
                due_date,
                &now,
            );
            todo.due_date = Some(due_date.clone());
        }
    }
    if let Some(assignee) = &update.assignee {
        if *assignee != todo.assignee {
            insert_changelog(
                &db,
                &id,
                "assignee",
                Todo::field_label("assignee"),
                &todo.assignee,
                assignee,
                &now,
            );
            todo.assignee = assignee.clone();
        }
    }
    if let Some(tags) = &update.tags {
        let old_tags = todo.tags.join(", ");
        let new_tags = tags.join(", ");
        if old_tags != new_tags {
            insert_changelog(
                &db,
                &id,
                "tags",
                Todo::field_label("tags"),
                &old_tags,
                &new_tags,
                &now,
            );
            todo.tags = tags.clone();
        }
    }

    todo.updated_at = now;
    let tags_json = serde_json::to_string(&todo.tags).unwrap();

    if is_collaborator {
        // Collaborator: tab/quadrant updates go to todo_collaborators
        db.execute(
            "UPDATE todo_collaborators SET tab=?1, quadrant=?2 WHERE todo_id=?3 AND user_id=?4 AND status='active'",
            rusqlite::params![todo.tab.as_str(), todo.quadrant.as_str(), id, user_id.0],
        ).ok();
        // Shared fields update the main todos table (verify collaborator access via subquery)
        db.execute(
            "UPDATE todos SET text=?1, content=?2, progress=?3, completed=?4, completed_at=?5, due_date=?6, assignee=?7, tags=?8, updated_at=?9 WHERE id=?10 AND id IN (SELECT todo_id FROM todo_collaborators WHERE user_id=?11 AND status='active')",
            rusqlite::params![
                todo.text,
                todo.content,
                todo.progress as i32,
                todo.completed as i32,
                todo.completed_at,
                todo.due_date,
                todo.assignee,
                tags_json,
                todo.updated_at,
                id,
                user_id.0,
            ],
        ).ok();
    } else if let Err(e) = db.execute(
        "UPDATE todos SET text=?1, content=?2, tab=?3, quadrant=?4, progress=?5, completed=?6, completed_at=?7, due_date=?8, assignee=?9, tags=?10, updated_at=?11 WHERE id=?12 AND user_id=?13",
        rusqlite::params![
            todo.text,
            todo.content,
            todo.tab.as_str(),
            todo.quadrant.as_str(),
            todo.progress as i32,
            todo.completed as i32,
            todo.completed_at,
            todo.due_date,
            todo.assignee,
            tags_json,
            todo.updated_at,
            id,
            user_id.0,
        ],
    ) {
        eprintln!("[todos] update_todo DB error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(TodoResponse {
            success: false, item: None,
            message: Some("数据库写入失败，请稍后重试".into()),
        }));
    }

    todo.changelog = load_changelog(&db, &id);

    (
        StatusCode::OK,
        Json(TodoResponse {
            success: true,
            item: Some(todo),
            message: Some("任务更新成功".into()),
        }),
    )
}

fn insert_changelog(
    db: &rusqlite::Connection,
    todo_id: &str,
    field: &str,
    label: &str,
    from: &str,
    to: &str,
    time: &str,
) {
    db.execute(
        "INSERT INTO todo_changelog (todo_id, field, label, from_val, to_val, time) VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![todo_id, field, label, from, to, time],
    )
    .ok();

    // Keep only 50 most recent entries per todo
    db.execute(
        "DELETE FROM todo_changelog WHERE todo_id = ?1 AND id NOT IN (SELECT id FROM todo_changelog WHERE todo_id = ?1 ORDER BY id DESC LIMIT 50)",
        [todo_id],
    )
    .ok();
}

pub async fn delete_todo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    // For collaborative todos, create a pending confirmation instead of immediate delete
    if collaboration::is_todo_collaborative(&db, &id)
        && collaboration::check_todo_participant(&db, &id, &user_id.0)
    {
        let conf_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        db.execute(
            "INSERT INTO pending_confirmations (id, item_type, item_id, action, initiated_by, initiated_at, status) VALUES (?1, 'todo', ?2, 'delete', ?3, ?4, 'pending')",
            rusqlite::params![conf_id, id, user_id.0, now],
        ).ok();

        return (
            StatusCode::OK,
            Json(SimpleResponse {
                success: true,
                message: Some("已发起删除确认，等待协作者同意".into()),
            }),
        );
    }

    let rows = db
        .execute(
            "UPDATE todos SET deleted = 1, deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
            rusqlite::params![now, id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some(format!("任务不存在: {}", id)),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("任务已删除".into()),
        }),
    )
}

pub async fn restore_todo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<TodoResponse>) {
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    let rows = db
        .execute(
            "UPDATE todos SET deleted = 0, deleted_at = NULL, updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
            rusqlite::params![now, id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(TodoResponse {
                success: false,
                item: None,
                message: Some(format!("任务不存在: {}", id)),
            }),
        );
    }

    let todo = db
        .query_row(
            "SELECT id, text, content, tab, quadrant, progress, completed_at, completed, due_date, deleted, assignee, tags, created_at, updated_at, deleted_at FROM todos WHERE id = ?1",
            [&id],
            row_to_todo,
        );
    match todo {
        Ok(mut t) => {
            t.changelog = load_changelog(&db, &id);
            (
                StatusCode::OK,
                Json(TodoResponse {
                    success: true,
                    item: Some(t),
                    message: Some("任务已恢复".into()),
                }),
            )
        }
        Err(e) => {
            eprintln!("[todos] restore_todo fetch error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TodoResponse {
                    success: false,
                    item: None,
                    message: Some("读取任务数据失败".into()),
                }),
            )
        }
    }
}

pub async fn permanent_delete_todo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    let rows = db
        .execute(
            "DELETE FROM todos WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some(format!("任务不存在: {}", id)),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("任务已永久删除".into()),
        }),
    )
}

pub async fn batch_update_todos(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(updates): Json<Vec<BatchUpdateItem>>,
) -> (StatusCode, Json<SimpleResponse>) {
    if updates.len() > 200 {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("批量更新上限 200 条".into()),
            }),
        );
    }
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();
    let mut updated_count = 0;

    for item in &updates {
        // Verify ownership
        let owned: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM todos WHERE id = ?1 AND user_id = ?2",
                rusqlite::params![item.id, user_id.0],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !owned {
            continue;
        }

        if let Some(tab) = &item.tab {
            db.execute(
                "UPDATE todos SET tab = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
                rusqlite::params![tab, now, item.id, user_id.0],
            )
            .ok();
        }
        if let Some(quadrant) = &item.quadrant {
            db.execute(
                "UPDATE todos SET quadrant = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
                rusqlite::params![quadrant, now, item.id, user_id.0],
            )
            .ok();
        }
        if let Some(progress) = item.progress {
            let p = progress.min(100) as i32;
            let completed = if p == 100 { 1 } else { 0 };
            db.execute(
                "UPDATE todos SET progress = ?1, completed = ?2, updated_at = ?3 WHERE id = ?4 AND user_id = ?5",
                rusqlite::params![p, completed, now, item.id, user_id.0],
            )
            .ok();
        }
        if let Some(completed) = item.completed {
            db.execute(
                "UPDATE todos SET completed = ?1, completed_at = ?2, updated_at = ?3 WHERE id = ?4 AND user_id = ?5",
                rusqlite::params![
                    completed as i32,
                    if completed { Some(now.clone()) } else { None },
                    now,
                    item.id,
                    user_id.0,
                ],
            )
            .ok();
        }
        updated_count += 1;
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some(format!("已更新 {} 个任务", updated_count)),
        }),
    )
}

#[derive(Debug, Deserialize)]
pub struct CountsQuery {
    pub tab: String,
}

pub async fn get_todo_counts(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<CountsQuery>,
) -> Json<serde_json::Value> {
    let db = state.db.lock();

    let quadrants = [
        "important-urgent",
        "important-not-urgent",
        "not-important-urgent",
        "not-important-not-urgent",
    ];

    let mut counts = serde_json::Map::new();
    for q in &quadrants {
        let count: i32 = db
            .query_row(
                "SELECT COUNT(*) FROM todos WHERE user_id = ?1 AND tab = ?2 AND quadrant = ?3 AND deleted = 0 AND completed = 0",
                rusqlite::params![user_id.0, query.tab, q],
                |row| row.get(0),
            )
            .unwrap_or(0);
        counts.insert(q.to_string(), serde_json::Value::from(count));
    }

    Json(serde_json::json!({
        "success": true,
        "counts": counts
    }))
}
