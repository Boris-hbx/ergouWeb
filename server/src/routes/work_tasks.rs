//! `/api/work/tasks` — CRUD for work tasks (independent of personal todos).
//!
//! See SPEC: `C:\Project\ergouPM\specs\work-task-table\spec.md`
//! Task ticket: T-094 (CRUD), T-101 (扩展 query 参数 + 给 LLM 工具复用)
//!
//! ## 设计:impl 函数 vs HTTP handler
//!
//! `*_impl` 函数(`create_task_impl` / `update_task_impl` / `query_tasks_impl`)
//! 接 `&Connection`,做纯 DB 逻辑;HTTP handler 只做参数提取 + 调 impl + JSON 响应。
//! 这样 LLM 工具(`services/tool_executor.rs`)能直接复用 impl,不重复 SQL。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value as JsonValue};
use tracing::{error, warn};

use crate::auth::UserId;
use crate::models::work_task::{CreateWorkTaskRequest, UpdateWorkTaskRequest, WorkTask};
use crate::state::AppState;

// ============ 共用结构 ============

/// 查询过滤器(GET /api/work/tasks query string + query_work_tasks 工具)。
/// 所有字段 AND;不传则返回最近(按 sort_order)若干条。
#[derive(Debug, Default, Deserialize)]
pub struct QueryFilters {
    pub q: Option<String>,
    pub assignee: Option<String>,
    pub level: Option<String>,
    pub status: Option<String>,
    pub status_not: Option<String>,
    pub priority: Option<String>,
    pub area: Option<String>,
    pub engage: Option<String>,
    pub due_before: Option<String>,
    pub due_after: Option<String>,
    /// `true` = due_date < today AND status ≠ done
    pub has_overdue: Option<bool>,
    /// T-119:按协作者筛选(模糊 JSON LIKE);精确单值
    pub collaborator: Option<String>,
    /// 默认 None → 不限制(返回全部,与 GET /api/work/tasks 原始行为兼容);
    /// LLM 工具默认填 10;上限 50。
    pub limit: Option<i64>,
}

/// 查询返回里附带的汇总字段(让 LLM 一句话概述)。
#[derive(Debug, Serialize)]
pub struct QuerySummary {
    pub overdue: usize,
    pub p0: usize,
    pub by_status: Map<String, JsonValue>, // {todo: N, doing: N, blocked: N, done: N}
}

// ============ HTTP 包装 ============

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "work_tasks", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// 把 'P0' 别名规整成后端规范的 'high'(任务令容许 LLM 输入 P0)
fn normalize_priority(p: &str) -> String {
    match p {
        "P0" | "p0" => "high".to_string(),
        other => other.to_string(),
    }
}

/// Parse a `custom_fields` JSON column from DB into a serde Map. Tolerates malformed
/// data by returning empty (with a warning).
fn parse_custom_fields(raw: &str) -> Map<String, JsonValue> {
    match serde_json::from_str::<JsonValue>(raw) {
        Ok(JsonValue::Object(m)) => m,
        Ok(_) => {
            warn!(target: "work_tasks", "custom_fields not an object: {}", raw);
            Map::new()
        }
        Err(e) => {
            warn!(target: "work_tasks", "custom_fields parse error: {} for {}", e, raw);
            Map::new()
        }
    }
}

/// Map a SELECT row to a `WorkTask`. Column order MUST match `SELECT_COLS`.
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<WorkTask> {
    let custom_raw: String = row.get(13)?;
    let tags_raw: String = row.get(16)?;
    let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
    let collab_raw: String = row.get(17)?;
    let collaborators: Vec<String> = serde_json::from_str(&collab_raw).unwrap_or_default();
    Ok(WorkTask {
        id: row.get(0)?,
        title: row.get(1)?,
        desc: row.get(2)?,
        assignee: row.get(3)?,
        level: row.get(4)?,
        freq: row.get(5)?,
        status: row.get(6)?,
        priority: row.get(7)?,
        area: row.get(8)?,
        engage: row.get(9)?,
        due_date: row.get(10)?,
        progress: row.get(11)?,
        sort_order: row.get(12)?,
        custom_fields: parse_custom_fields(&custom_raw),
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        tags,
        collaborators,
    })
}

// T-110:SELECT_COLS 末尾追加 tags;T-119:再追加 collaborators(末尾保持向前兼容)
const SELECT_COLS: &str =
    "id, title, desc, assignee, level, freq, status, priority, area, engage, due_date, progress, sort_order, custom_fields, created_at, updated_at, tags, collaborators";

// ============ impl 函数(给 HTTP + 工具复用) ============

/// 内部:按过滤器查任务并附 summary。LLM 工具和 HTTP GET 都走这。
pub fn query_tasks_impl(
    db: &Connection,
    user_id: &str,
    f: &QueryFilters,
) -> Result<(Vec<WorkTask>, QuerySummary), String> {
    let mut conditions: Vec<String> = vec!["user_id = ?1".to_string(), "deleted = 0".to_string()];
    let mut params_v: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(user_id.to_string())];
    let mut idx: usize = 2;

    if let Some(q) = f.q.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("(title LIKE ?{idx} OR desc LIKE ?{idx})"));
        params_v.push(Box::new(format!("%{q}%")));
        idx += 1;
    }
    if let Some(v) = f.assignee.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("assignee = ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.level.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("level = ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.status.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("status = ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.status_not.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("status != ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.priority.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("priority = ?{idx}"));
        params_v.push(Box::new(normalize_priority(v)));
        idx += 1;
    }
    if let Some(v) = f.area.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("area = ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.engage.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("engage = ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.due_before.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("due_date IS NOT NULL AND due_date <= ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = f.due_after.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("due_date IS NOT NULL AND due_date >= ?{idx}"));
        params_v.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(true) = f.has_overdue {
        // 等价于 due_before(today) AND status != 'done'
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        conditions.push(format!(
            "due_date IS NOT NULL AND due_date < ?{idx} AND status != 'done'"
        ));
        params_v.push(Box::new(today));
        idx += 1;
    }
    // T-119:按协作者筛选(JSON LIKE 模糊匹配,collaborators 存为 '["a","b"]' 形式)
    if let Some(v) = f.collaborator.as_deref().filter(|s| !s.is_empty()) {
        // JSON 数组里搜 "name" 字面(带引号定界,避免 substr 误匹配)
        conditions.push(format!("collaborators LIKE ?{idx}"));
        params_v.push(Box::new(format!("%\"{}\"%", v.replace('"', "\\\""))));
        // idx not used after this
    }

    let limit = f.limit.unwrap_or(0).clamp(0, 50);
    let limit_clause = if limit > 0 {
        format!(" LIMIT {limit}")
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT {SELECT_COLS} FROM work_tasks WHERE {} ORDER BY sort_order ASC, created_at ASC{limit_clause}",
        conditions.join(" AND ")
    );

    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = params_v.iter().map(|b| &**b).collect();
    let rows = stmt
        .query_map(params_ref.as_slice(), row_to_task)
        .map_err(|e| format!("query: {e}"))?;
    let items: Vec<WorkTask> = rows.filter_map(|r| r.ok()).collect();

    // 算 summary:overdue / p0 / by_status
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut overdue = 0usize;
    let mut p0 = 0usize;
    let mut by_status: Map<String, JsonValue> = Map::new();
    for t in &items {
        if let Some(due) = t.due_date.as_deref() {
            if t.status != "done" && due < today.as_str() {
                overdue += 1;
            }
        }
        if t.priority == "high" {
            p0 += 1;
        }
        let cnt = by_status
            .get(&t.status)
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            + 1;
        by_status.insert(t.status.clone(), json!(cnt));
    }
    Ok((
        items,
        QuerySummary {
            overdue,
            p0,
            by_status,
        },
    ))
}

/// 内部:创建任务,返回完整对象。
pub fn create_task_impl(
    db: &Connection,
    user_id: &str,
    req: &CreateWorkTaskRequest,
) -> Result<WorkTask, String> {
    let now = now_rfc3339();
    let custom_str = serde_json::to_string(
        &req.custom_fields
            .clone()
            .map(JsonValue::Object)
            .unwrap_or_else(|| JsonValue::Object(Map::new())),
    )
    .unwrap_or_else(|_| "{}".to_string());

    // T-110:tags 也存为 JSON 字符串
    let tags_str = serde_json::to_string(&req.tags).unwrap_or_else(|_| "[]".to_string());
    // T-119:协作者 + 去重(若与主责任人同名则剔除)
    let collab_clean: Vec<String> = req
        .collaborators
        .iter()
        .filter(|c| !c.is_empty() && c.as_str() != req.assignee.as_str())
        .cloned()
        .collect();
    let collab_str = serde_json::to_string(&collab_clean).unwrap_or_else(|_| "[]".to_string());

    let next_sort: f64 = db
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM work_tasks WHERE user_id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let priority = normalize_priority(&req.priority);

    db.execute(
        "INSERT INTO work_tasks
           (user_id, title, desc, assignee, level, freq, status, priority, area, engage,
            due_date, progress, custom_fields, tags, collaborators, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)",
        params![
            user_id,
            &req.title,
            &req.desc,
            &req.assignee,
            &req.level,
            &req.freq,
            &req.status,
            &priority,
            &req.area,
            &req.engage,
            &req.due_date,
            req.progress,
            &custom_str,
            &tags_str,
            &collab_str,
            next_sort,
            &now,
        ],
    )
    .map_err(|e| format!("insert: {e}"))?;

    let new_id = db.last_insert_rowid();
    load_task(db, user_id, new_id).ok_or_else(|| "Failed to reload created task".to_string())
}

/// 内部:更新任务。若任务不存在或不属于该用户返回 `Ok(None)`。
pub fn update_task_impl(
    db: &Connection,
    user_id: &str,
    id: i64,
    mut patch: UpdateWorkTaskRequest,
) -> Result<Option<WorkTask>, String> {
    // 1) 先读 old_status + 旧 custom_fields(merge 需要)+ freq/due_date(T-131 周期复发判断)
    let existing: Option<(String, String, String, Option<String>)> = db
        .query_row(
            "SELECT status, custom_fields, freq, due_date FROM work_tasks \
             WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, user_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .ok();
    let Some((old_status, old_custom_raw, freq, cur_due)) = existing else {
        return Ok(None);
    };

    // T-131:周期任务(freq != 一次性)完成 → 不归档,写完成历史 + 重置进度 + 顺延 due_date。
    //   覆盖 patch(status→todo / progress→0 / due_date→下个周期),后续按普通更新落库。
    //   一次性任务(freq 空或'一次性')完成仍走 status=done 归档,行为不变。
    let wants_complete = patch.status.as_deref() == Some("done")
        || patch.progress.map(|p| p >= 100).unwrap_or(false);
    let is_periodic = !freq.is_empty() && freq != "一次性";
    if wants_complete && is_periodic && old_status != "done" {
        let now = now_rfc3339();
        db.execute(
            "INSERT INTO work_task_completions \
                (task_id, user_id, completed_at, cycle_due_date, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?3)",
            params![id, user_id, &now, &cur_due],
        )
        .map_err(|e| format!("insert completion: {e}"))?;
        let new_due = next_cycle_due(cur_due.as_deref(), &freq);
        patch.status = Some("todo".to_string());
        patch.progress = Some(0);
        patch.due_date = Some(new_due);
    }

    // 2) 'P0' alias → 'high'
    if let Some(ref p) = patch.priority {
        patch.priority = Some(normalize_priority(p));
    }

    // 3) 构 SET 子句
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    macro_rules! push_str {
        ($field:literal, $opt:expr) => {
            if let Some(v) = $opt {
                sets.push(concat!($field, " = ?"));
                vals.push(Box::new(v));
            }
        };
    }
    // T-119:在 push_str! 把 patch.assignee 移走前,先保留一份用于 collaborators 去重
    let new_assignee = patch.assignee.clone();
    push_str!("title", patch.title);
    push_str!("desc", patch.desc);
    push_str!("assignee", patch.assignee);
    push_str!("level", patch.level);
    push_str!("freq", patch.freq);
    push_str!("area", patch.area);
    push_str!("engage", patch.engage);

    let new_status = patch.status.clone();
    if let Some(s) = patch.status {
        sets.push("status = ?");
        vals.push(Box::new(s));
    }
    push_str!("priority", patch.priority);

    // due_date: 空字符串 → NULL;非空 → 存
    if let Some(due) = patch.due_date {
        sets.push("due_date = ?");
        if due.is_empty() {
            vals.push(Box::new(Option::<String>::None));
        } else {
            vals.push(Box::new(Some(due)));
        }
    }

    // progress: 显式优先;status=done 且未传 progress → 自动 100。
    let mut effective_progress = patch.progress;
    if let Some(s) = new_status.as_deref() {
        if s == "done" && effective_progress.is_none() {
            effective_progress = Some(100);
        }
    } else if old_status == "done" {
        // status 未变;不做自动联动
    }
    // progress=100 显式 → 自动把 status 也设成 done(spec 边界规则)
    if let Some(p) = effective_progress {
        sets.push("progress = ?");
        vals.push(Box::new(p.clamp(0, 100)));
        if p >= 100 && new_status.is_none() && old_status != "done" {
            sets.push("status = ?");
            vals.push(Box::new("done".to_string()));
        }
    }

    // custom_fields: merge over existing
    if let Some(patch_custom) = patch.custom_fields {
        let mut merged = parse_custom_fields(&old_custom_raw);
        for (k, v) in patch_custom {
            merged.insert(k, v);
        }
        let s = serde_json::to_string(&JsonValue::Object(merged)).unwrap_or_else(|_| "{}".into());
        sets.push("custom_fields = ?");
        vals.push(Box::new(s));
    }

    // T-110:tags(内置「标签」列)整体替换,不 merge。空数组 → 清空。
    if let Some(patch_tags) = patch.tags {
        let s = serde_json::to_string(&patch_tags).unwrap_or_else(|_| "[]".into());
        sets.push("tags = ?");
        vals.push(Box::new(s));
    }
    // T-119:collaborators 整体替换;若与新 assignee 同名,后端去重(防客户端漏过滤)
    if let Some(patch_collab) = patch.collaborators {
        let effective_assignee = new_assignee
            .clone()
            .or_else(|| {
                db.query_row(
                    "SELECT assignee FROM work_tasks WHERE id = ?1 AND user_id = ?2",
                    params![id, user_id],
                    |r| r.get::<_, String>(0),
                )
                .ok()
            })
            .unwrap_or_default();
        let clean: Vec<String> = patch_collab
            .into_iter()
            .filter(|c| !c.is_empty() && c != &effective_assignee)
            .collect();
        let s = serde_json::to_string(&clean).unwrap_or_else(|_| "[]".into());
        sets.push("collaborators = ?");
        vals.push(Box::new(s));
    }

    if let Some(so) = patch.sort_order {
        sets.push("sort_order = ?");
        vals.push(Box::new(so));
    }

    if sets.is_empty() {
        return Ok(load_task(db, user_id, id));
    }

    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));

    vals.push(Box::new(id));
    vals.push(Box::new(user_id.to_string()));

    let sql = format!(
        "UPDATE work_tasks SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    db.execute(&sql, params_ref.as_slice())
        .map_err(|e| format!("update: {e}"))?;
    Ok(load_task(db, user_id, id))
}

/// 内部:加载某条任务(给 create/update 后回查用)。
fn load_task(db: &Connection, user_id: &str, id: i64) -> Option<WorkTask> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM work_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_task).ok()
}

// ============ T-131:周期复发 顺延计算 ============

/// 按 freq 顺延 due_date(base 不存在则按 today)。月/季越界按目标月最后一天(避免 1/31→2/31)。
fn next_cycle_due(base: Option<&str>, freq: &str) -> String {
    let today = Utc::now().date_naive();
    let d = base.and_then(parse_due).unwrap_or(today);
    let nd = match freq {
        "每日" => d + Duration::days(1),
        "每周" => d + Duration::days(7),
        "每月" => add_months(d, 1),
        "每季" => add_months(d, 3),
        _ => d + Duration::days(7), // 兜底(is_periodic 已排除一次性)
    };
    nd.format("%Y-%m-%d").to_string()
}
/// 解析 'YYYY-MM-DD';'MM-DD' 补当前年;否则 None。
fn parse_due(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }
    if s.len() == 5 && s.as_bytes().get(2) == Some(&b'-') {
        let y = Utc::now().year();
        if let Ok(d) = NaiveDate::parse_from_str(&format!("{y}-{s}"), "%Y-%m-%d") {
            return Some(d);
        }
    }
    None
}
fn last_day_of_month(y: i32, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}
fn add_months(d: NaiveDate, n: u32) -> NaiveDate {
    let total = d.month0() + n;
    let y = d.year() + (total / 12) as i32;
    let month = total % 12 + 1;
    let day = d.day().min(last_day_of_month(y, month));
    NaiveDate::from_ymd_opt(y, month, day).unwrap_or(d)
}

/// GET /api/work/tasks/:id/completions — 某周期任务的完成历史(倒序)。
pub async fn list_completions(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let mut stmt = match db.prepare(
        "SELECT id, completed_at, cycle_due_date, note FROM work_task_completions \
         WHERE task_id = ?1 AND user_id = ?2 ORDER BY completed_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => return db_error("completions prepare", e),
    };
    let rows = stmt.query_map(params![id, &user_id.0], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "completedAt": r.get::<_, String>(1)?,
            "cycleDueDate": r.get::<_, Option<String>>(2)?,
            "note": r.get::<_, String>(3)?,
        }))
    });
    let rows = match rows {
        Ok(r) => r,
        Err(e) => return db_error("completions query", e),
    };
    let items: Vec<JsonValue> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

/// GET /api/work/task-completions — 本人所有周期完成记录(已完成档案用,JOIN 任务标题)。
pub async fn list_all_completions(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let mut stmt = match db.prepare(
        "SELECT c.id, c.task_id, c.completed_at, c.cycle_due_date, t.title, t.freq \
         FROM work_task_completions c JOIN work_tasks t ON c.task_id = t.id \
         WHERE c.user_id = ?1 ORDER BY c.completed_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => return db_error("all completions prepare", e),
    };
    let rows = stmt.query_map(params![&user_id.0], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "taskId": r.get::<_, i64>(1)?,
            "completedAt": r.get::<_, String>(2)?,
            "cycleDueDate": r.get::<_, Option<String>>(3)?,
            "title": r.get::<_, String>(4)?,
            "freq": r.get::<_, String>(5)?,
        }))
    });
    let rows = match rows {
        Ok(r) => r,
        Err(e) => return db_error("all completions query", e),
    };
    let items: Vec<JsonValue> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

// ============ HTTP handlers(薄壳,委托 impl) ============

/// GET /api/work/tasks(支持 query string 过滤,T-101 扩展)
pub async fn list_tasks(
    State(state): State<AppState>,
    user_id: UserId,
    Query(filters): Query<QueryFilters>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match query_tasks_impl(&db, &user_id.0, &filters) {
        Ok((items, summary)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "items": items,
                "count": items.len(),
                "summary": summary,
            })),
        ),
        Err(e) => {
            error!(target: "work_tasks", "list_tasks: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

/// POST /api/work/tasks
pub async fn create_task(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateWorkTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match create_task_impl(&db, &user_id.0, &req) {
        Ok(item) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Err(e) => {
            error!(target: "work_tasks", "create_task: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

/// PATCH /api/work/tasks/{id}
pub async fn update_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateWorkTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match update_task_impl(&db, &user_id.0, id, patch) {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到任务" })),
        ),
        Err(e) => {
            error!(target: "work_tasks", "update_task: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

/// DELETE /api/work/tasks/{id} — soft delete
pub async fn delete_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let res = db.execute(
        "UPDATE work_tasks SET deleted = 1, deleted_at = ?1, updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match res {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到任务" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete_task", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::to_bytes;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let status = resp.status();
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        serde_json::from_slice(&body).unwrap_or_else(|e| {
            panic!("parse fail status={} body={:?} err={}", status, body_str, e)
        })
    }

    #[tokio::test]
    async fn create_then_list_returns_task() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "alice", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"院年度规划","level":"院"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["title"], "院年度规划");
        assert_eq!(j["item"]["status"], "todo");

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["items"].as_array().unwrap().len(), 1);
        assert_eq!(j["items"][0]["title"], "院年度规划");
        // T-101:list 返回带 summary
        assert_eq!(j["count"], 1);
        assert_eq!(j["summary"]["overdue"], 0);
        assert_eq!(j["summary"]["p0"], 0);
        assert_eq!(j["summary"]["by_status"]["todo"], 1);
    }

    #[tokio::test]
    async fn patch_status_done_auto_sets_progress_100() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "bob", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"done"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "done");
        assert_eq!(j["item"]["progress"], 100);
    }

    #[tokio::test]
    async fn patch_progress_100_auto_sets_status_done() {
        // T-101 spec 边界:progress=100 → 自动 status=done(与 status=done 双向联动)
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "bob100", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"progress":100}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["progress"], 100);
        assert_eq!(j["item"]["status"], "done");
    }

    #[tokio::test]
    async fn delete_soft_removes_from_list() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "carol", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"y"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn custom_fields_patch_merges() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "dan", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"z","customFields":{"a":"1"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"customFields":{"b":"2"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let cf = j["item"]["customFields"].as_object().unwrap();
        assert_eq!(cf.get("a"), Some(&json!("1")));
        assert_eq!(cf.get("b"), Some(&json!("2")));
    }

    // ============ T-110:tags 字段持久化测试 ============
    // 根因复盘:T-108 把内置「标签」列种成 builtin=true(前端走顶层 patch.tags),
    //          但 UpdateWorkTaskRequest 没 tags 字段 → serde 静默丢弃。
    // 修复:补 work_tasks.tags 列 + WorkTask/Create/Update 加字段 + INSERT/UPDATE 走 schema。

    #[tokio::test]
    async fn create_with_tags_persists() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t110a", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x","tags":["基金","论文"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        let tags = j["item"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], "基金");
        assert_eq!(tags[1], "论文");
    }

    #[tokio::test]
    async fn patch_tags_persists_and_reloads() {
        // 这是 T-110 修复的核心 bug 用例:多选标签 + PATCH + GET 验证持久化
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t110b", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 1) 建任务
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"y"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // 2) PATCH tags(模拟前端选标签 confirm)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"tags":["行政","教学"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        // 关键断言:PATCH 响应里 tags 已落实(在 T-110 修复前这里会是空数组,bug)
        let tags = j["item"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&json!("行政")));
        assert!(tags.contains(&json!("教学")));

        // 3) GET list 再确认(模拟刷新页面)
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let item = &j["items"][0];
        let tags = item["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2, "刷新后 tags 必须还在(T-110 修复点)");
    }

    #[tokio::test]
    async fn patch_tags_replaces_not_merges() {
        // tags 整体替换语义(与 custom_fields merge 不同) — 选完整集合
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t110c", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"z","tags":["a","b","c"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // PATCH 替换为 ["x"]
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"tags":["x"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let tags = j["item"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "x");
    }

    #[tokio::test]
    async fn patch_tags_empty_array_clears() {
        // 空数组 → 清空 tags
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t110d", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"w","tags":["k1","k2"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"tags":[]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["tags"].as_array().unwrap().len(), 0);
    }

    // ============ T-119:collaborators 字段测试 ============

    #[tokio::test]
    async fn create_with_collaborators_persists() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t119a", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"所务会","assignee":"陈大民","collaborators":["王主任","李秘书"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        let collab = j["item"]["collaborators"].as_array().unwrap();
        assert_eq!(collab.len(), 2);
        assert!(collab.contains(&json!("王主任")));
        assert!(collab.contains(&json!("李秘书")));
    }

    #[tokio::test]
    async fn patch_collaborators_replaces() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t119b", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"title":"x","assignee":"陈","collaborators":["a","b","c"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // PATCH 替换为 ["d"]
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"collaborators":["d"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let collab = j["item"]["collaborators"].as_array().unwrap();
        assert_eq!(collab.len(), 1);
        assert_eq!(collab[0], "d");
    }

    #[tokio::test]
    async fn collaborator_equal_to_assignee_is_deduped_on_create() {
        // 创建时 collaborators 含主责任人 → 后端自动去重
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t119c", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"title":"x","assignee":"陈","collaborators":["陈","王"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let collab = j["item"]["collaborators"].as_array().unwrap();
        assert_eq!(collab.len(), 1, "主责任人应自动从 collaborators 去重");
        assert_eq!(collab[0], "王");
    }

    #[tokio::test]
    async fn patch_assignee_dedupes_existing_collaborator() {
        // PATCH 把 assignee 改成 "王",而 collaborators 已含 "王" → 去重
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t119d", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"title":"x","assignee":"陈","collaborators":["王","李"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // PATCH:换主责任人为"王",同时显式更新 collaborators(含"王")
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"assignee":"王","collaborators":["王","李","陈"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let collab = j["item"]["collaborators"].as_array().unwrap();
        // "王" 应被去重(因为是新 assignee);"李"、"陈"保留
        assert!(!collab.iter().any(|v| v == "王"));
        assert!(collab.iter().any(|v| v == "李"));
        assert!(collab.iter().any(|v| v == "陈"));
        assert_eq!(j["item"]["assignee"], "王");
    }

    #[tokio::test]
    async fn query_filter_by_collaborator() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t119e", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 3 条:王主任协作 2 条,李秘书协作 1 条
        for body in [
            r#"{"title":"a","assignee":"陈","collaborators":["王主任","李秘书"]}"#,
            r#"{"title":"b","assignee":"赵","collaborators":["王主任"]}"#,
            r#"{"title":"c","assignee":"孙","collaborators":["李秘书"]}"#,
        ] {
            let _ = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/work/tasks")
                        .header("Cookie", &cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        // 按 collaborator=王主任 筛选 → 2 条
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks?collaborator=王主任")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let items = j["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
    }

    // ============ T-101:query 过滤器测试 ============

    #[tokio::test]
    async fn query_filters_assignee_and_status_not() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "qfilter", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 准备 3 条:陈老师/todo,陈老师/done,王主任/todo
        for body in [
            r#"{"title":"a","assignee":"陈老师","status":"todo"}"#,
            r#"{"title":"b","assignee":"陈老师","status":"done","progress":100}"#,
            r#"{"title":"c","assignee":"王主任","status":"todo"}"#,
        ] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/work/tasks")
                        .header("Cookie", &session_cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        // ?assignee=陈老师&status_not=done → 应只剩 "a"
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks?assignee=%E9%99%88%E8%80%81%E5%B8%88&status_not=done")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let items = j["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "a");
    }

    #[tokio::test]
    async fn query_filters_q_matches_title_or_desc() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "qq", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        for body in [
            r#"{"title":"季度经费报表","desc":""}"#,
            r#"{"title":"院评审准备","desc":"含经费明细附件"}"#,
            r#"{"title":"复印资料","desc":""}"#,
        ] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/work/tasks")
                        .header("Cookie", &session_cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        // ?q=经费 → 应匹配 1+2(标题/简介任一含)
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks?q=%E7%BB%8F%E8%B4%B9")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let items = body_json(resp).await["items"].clone();
        assert_eq!(items.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn query_summary_counts_overdue_p0_by_status() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "qsum", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 1 条逾期 + 1 条 P0(未来) + 1 条普通
        for body in [
            r#"{"title":"逾期","due":"2020-01-01","status":"todo","priority":"high"}"#,
            r#"{"title":"P0未来","due":"2099-01-01","priority":"high"}"#,
            r#"{"title":"普通","priority":"mid"}"#,
        ] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/work/tasks")
                        .header("Cookie", &session_cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 3);
        assert_eq!(j["summary"]["overdue"], 1); // 第一条逾期(且未 done)
        assert_eq!(j["summary"]["p0"], 2); // 两条 high
        assert_eq!(j["summary"]["by_status"]["todo"], 3);
    }

    #[tokio::test]
    async fn create_accepts_p0_alias_and_normalizes_to_high() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "pp0", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x","priority":"P0"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["priority"], "high");
    }

    #[tokio::test]
    async fn query_has_overdue_filter() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "qover", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        for body in [
            r#"{"title":"oldA","due":"2020-01-01","status":"todo"}"#,
            r#"{"title":"oldDone","due":"2020-01-01","status":"done","progress":100}"#,
            r#"{"title":"future","due":"2099-01-01","status":"todo"}"#,
        ] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/work/tasks")
                        .header("Cookie", &session_cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks?has_overdue=true")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let items = body_json(resp).await["items"].clone();
        let arr = items.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["title"], "oldA"); // 只有未完成的逾期
    }

    // ============ T-131:周期复发测试 ============

    #[test]
    fn next_cycle_week_day_month_quarter() {
        assert_eq!(next_cycle_due(Some("2026-05-25"), "每周"), "2026-06-01");
        assert_eq!(next_cycle_due(Some("2026-05-25"), "每日"), "2026-05-26");
        assert_eq!(next_cycle_due(Some("2026-05-25"), "每月"), "2026-06-25");
        assert_eq!(next_cycle_due(Some("2026-05-25"), "每季"), "2026-08-25");
        // 月末越界 clamp:1/31 + 1月 → 2/28(2026 非闰年)
        assert_eq!(next_cycle_due(Some("2026-01-31"), "每月"), "2026-02-28");
        // 跨年
        assert_eq!(next_cycle_due(Some("2026-12-15"), "每月"), "2027-01-15");
    }

    #[tokio::test]
    async fn periodic_task_recurs_not_archived_on_complete() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t131p", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 建 freq=每月、due=2026-05-25
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"title":"月报","freq":"每月","due":"2026-05-25"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // 完成(PATCH status=done)→ 周期任务不归档:status=todo, progress=0, due 顺延
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"done"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "todo");
        assert_eq!(j["item"]["progress"], 0);
        assert_eq!(j["item"]["due"], "2026-06-25");

        // 完成历史 1 条,cycleDueDate = 完成时的 due
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/work/tasks/{id}/completions"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let jc = body_json(resp).await;
        assert_eq!(jc["count"], 1);
        assert_eq!(jc["items"][0]["cycleDueDate"], "2026-05-25");
    }

    #[tokio::test]
    async fn oneoff_task_archives_normally_unchanged() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "t131o", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 默认 freq 空 = 一次性
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"一次性活"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"done"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "done");
        assert_eq!(j["item"]["progress"], 100); // 一次性完成自动 100,行为不变
    }
}
