//! `/api/praxis/health/*` — Praxis 健康板块 (身体健康子板块, T-296).
//!
//! See SPEC `C:\Project\ergouPM\specs\praxis\spec.md` §12 + 设计底本
//! `C:\Project\ergouPM\docs\praxis-health-dimensions-v1.md`.
//!
//! 维度目录(可自定义)、每日习惯打卡、周期指标/体检录入、靶盘 board 装配、
//! journal 派生健康信号、AI 打分(可解释/重奖趋势/核心加权总分)。
//! 全部端点 `AdminUserId` 守卫，数据按 `user_id` 隔离。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tracing::{error, warn};

use crate::auth::AdminUserId;
use crate::models::praxis_health::{
    CreateDimRequest, HealthDim, HealthMark, HealthMetric, MarkRequest, MetricRequest,
    UpdateDimRequest, KINDS, RINGS, SEED_DIMS, SECTORS,
};
use crate::services::llm::LlmClient;
use crate::state::AppState;

const DIM_COLS: &str = "id, dim_key, name, sector, ring, kind, unit, target_floor, target_goal, cadence, seeded, sort_order, archived";

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}
fn today_str() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}
/// ISO 周键，如 `2026-W27`（AI 打分快照按周留档）。
fn iso_week_of(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| {
            let w = d.iso_week();
            format!("{}-W{:02}", w.year(), w.week())
        })
        .unwrap_or_else(|_| "0000-W00".to_string())
}
fn this_week() -> String {
    iso_week_of(&today_str())
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "praxis_health", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn bad(msg: &str) -> (StatusCode, Json<JsonValue>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "success": false, "error": msg })),
    )
}

fn validate_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|_| s.to_string())
        .map_err(|_| "日期须为 YYYY-MM-DD".to_string())
}

fn one_of(val: &str, allowed: &[&str]) -> bool {
    allowed.contains(&val)
}

// ===================== 维度目录 dims =====================

fn row_to_dim(row: &rusqlite::Row) -> rusqlite::Result<HealthDim> {
    Ok(HealthDim {
        id: row.get(0)?,
        dim_key: row.get(1)?,
        name: row.get(2)?,
        sector: row.get(3)?,
        ring: row.get(4)?,
        kind: row.get(5)?,
        unit: row.get(6)?,
        target_floor: row.get(7)?,
        target_goal: row.get(8)?,
        cadence: row.get(9)?,
        seeded: row.get::<_, i64>(10)? != 0,
        sort_order: row.get(11)?,
        archived: row.get::<_, i64>(12)? != 0,
    })
}

/// 首次访问时把系统种子维度写入该用户（对齐靶盘原型 17 维）。幂等。
/// T-297⑥：计数**含软删行** —— 用户曾种过再删光，不再重播种（只种"从未有过"的用户）。
fn ensure_seeded(db: &Connection, user_id: &str) -> Result<(), String> {
    let cnt: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM praxis_health_dims WHERE user_id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("count dims: {e}"))?;
    if cnt > 0 {
        return Ok(());
    }
    let now = now_rfc3339();
    for (i, (key, name, sector, ring, kind, unit, floor)) in SEED_DIMS.iter().enumerate() {
        db.execute(
            "INSERT INTO praxis_health_dims
               (user_id, dim_key, name, sector, ring, kind, unit, target_floor, target_goal, cadence, seeded, sort_order, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'', ?9, 1, ?10, ?11, ?11)",
            params![
                user_id, key, name, sector, ring, kind, unit, floor,
                cadence_for(kind), i as f64, now
            ],
        )
        .map_err(|e| format!("seed dim {key}: {e}"))?;
    }
    Ok(())
}

fn cadence_for(kind: &str) -> &'static str {
    match kind {
        "metric" => "quarterly",
        "signal" => "adhoc",
        _ => "daily",
    }
}

fn list_dims_impl(db: &Connection, user_id: &str) -> Result<Vec<HealthDim>, String> {
    ensure_seeded(db, user_id)?;
    let sql = format!(
        "SELECT {DIM_COLS} FROM praxis_health_dims WHERE user_id = ?1 AND deleted = 0 ORDER BY sort_order, id"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params![user_id], row_to_dim)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn load_dim(db: &Connection, user_id: &str, id: i64) -> Option<HealthDim> {
    let sql = format!(
        "SELECT {DIM_COLS} FROM praxis_health_dims WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_dim).ok()
}

fn create_dim_impl(
    db: &Connection,
    user_id: &str,
    req: &CreateDimRequest,
) -> Result<HealthDim, String> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err("请填维度名称".into());
    }
    if name.chars().count() > 40 {
        return Err("名称过长".into());
    }
    let sector = req.sector.clone().unwrap_or_else(|| "move".into());
    let ring = req.ring.clone().unwrap_or_else(|| "mid".into());
    let kind = req.kind.clone().unwrap_or_else(|| "habit".into());
    if !one_of(&sector, SECTORS) {
        return Err("类别非法".into());
    }
    if !one_of(&ring, RINGS) {
        return Err("圈层非法".into());
    }
    if !one_of(&kind, KINDS) {
        return Err("采集方式非法".into());
    }
    let cadence = req
        .cadence
        .clone()
        .unwrap_or_else(|| cadence_for(&kind).to_string());
    let now = now_rfc3339();
    // dim_key：自定义维度用 custom-<时间> 保证唯一但不撞种子 key。
    let key = format!("custom-{}", Utc::now().timestamp_millis());
    let next_order: f64 = db
        .query_row(
            "SELECT COALESCE(MAX(sort_order),0)+1 FROM praxis_health_dims WHERE user_id=?1",
            params![user_id],
            |r| r.get(0),
        )
        .unwrap_or(999.0);
    db.execute(
        "INSERT INTO praxis_health_dims
           (user_id, dim_key, name, sector, ring, kind, unit, target_floor, target_goal, cadence, seeded, sort_order, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10, 0, ?11, ?12, ?12)",
        params![
            user_id, key, name, sector, ring, kind,
            req.unit.clone().unwrap_or_default(),
            req.target_floor.clone().unwrap_or_default(),
            req.target_goal.clone().unwrap_or_default(),
            cadence, next_order, now
        ],
    )
    .map_err(|e| format!("insert dim: {e}"))?;
    let id = db.last_insert_rowid();
    load_dim(db, user_id, id).ok_or_else(|| "reload failed".into())
}

fn update_dim_impl(
    db: &Connection,
    user_id: &str,
    id: i64,
    patch: &UpdateDimRequest,
) -> Result<Option<HealthDim>, String> {
    let Some(mut d) = load_dim(db, user_id, id) else {
        return Ok(None);
    };
    let f = &patch.fields;
    if let Some(v) = f.get("name").and_then(|v| v.as_str()) {
        let v = v.trim();
        if v.is_empty() || v.chars().count() > 40 {
            return Err("名称非法".into());
        }
        d.name = v.to_string();
    }
    if let Some(v) = f.get("sector").and_then(|v| v.as_str()) {
        if !one_of(v, SECTORS) {
            return Err("类别非法".into());
        }
        d.sector = v.to_string();
    }
    if let Some(v) = f.get("ring").and_then(|v| v.as_str()) {
        if !one_of(v, RINGS) {
            return Err("圈层非法".into());
        }
        d.ring = v.to_string();
    }
    if let Some(v) = f.get("kind").and_then(|v| v.as_str()) {
        if !one_of(v, KINDS) {
            return Err("采集方式非法".into());
        }
        d.kind = v.to_string();
    }
    if let Some(v) = f.get("unit").and_then(|v| v.as_str()) {
        d.unit = v.to_string();
    }
    if let Some(v) = f.get("targetFloor").and_then(|v| v.as_str()) {
        d.target_floor = v.to_string();
    }
    if let Some(v) = f.get("targetGoal").and_then(|v| v.as_str()) {
        d.target_goal = v.to_string();
    }
    if let Some(v) = f.get("cadence").and_then(|v| v.as_str()) {
        d.cadence = v.to_string();
    }
    if let Some(v) = f.get("archived").and_then(|v| v.as_bool()) {
        d.archived = v;
    }
    let now = now_rfc3339();
    db.execute(
        "UPDATE praxis_health_dims
         SET name=?1, sector=?2, ring=?3, kind=?4, unit=?5, target_floor=?6, target_goal=?7, cadence=?8, archived=?9, updated_at=?10
         WHERE id=?11 AND user_id=?12 AND deleted=0",
        params![
            d.name, d.sector, d.ring, d.kind, d.unit, d.target_floor, d.target_goal, d.cadence,
            d.archived as i64, now, id, user_id
        ],
    )
    .map_err(|e| format!("update dim: {e}"))?;
    Ok(load_dim(db, user_id, id))
}

// ===================== 每日习惯打卡 marks =====================

fn upsert_mark_impl(
    db: &Connection,
    user_id: &str,
    req: &MarkRequest,
) -> Result<HealthMark, String> {
    if load_dim(db, user_id, req.dim_id).is_none() {
        return Err("维度不存在".into());
    }
    let date = match req.mark_date.as_deref().filter(|s| !s.is_empty()) {
        Some(d) => validate_date(d)?,
        None => today_str(),
    };
    let now = now_rfc3339();
    db.execute(
        "INSERT INTO praxis_health_marks (user_id, dim_id, mark_date, value, done, note, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?7)
         ON CONFLICT(user_id, dim_id, mark_date)
         DO UPDATE SET value=excluded.value, done=excluded.done, note=excluded.note, updated_at=excluded.updated_at",
        params![
            user_id, req.dim_id, date, req.value, req.done as i64,
            req.note.clone().unwrap_or_default(), now
        ],
    )
    .map_err(|e| format!("upsert mark: {e}"))?;
    db.query_row(
        "SELECT id, dim_id, mark_date, value, done, note FROM praxis_health_marks
         WHERE user_id=?1 AND dim_id=?2 AND mark_date=?3",
        params![user_id, req.dim_id, date],
        |r| {
            Ok(HealthMark {
                id: r.get(0)?,
                dim_id: r.get(1)?,
                mark_date: r.get(2)?,
                value: r.get(3)?,
                done: r.get::<_, i64>(4)? != 0,
                note: r.get(5)?,
            })
        },
    )
    .map_err(|e| format!("reload mark: {e}"))
}

fn list_marks_impl(
    db: &Connection,
    user_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<HealthMark>, String> {
    let from = from.filter(|s| !s.is_empty()).unwrap_or("0000-01-01");
    let to = to.filter(|s| !s.is_empty()).unwrap_or("9999-12-31");
    let mut stmt = db
        .prepare(
            "SELECT id, dim_id, mark_date, value, done, note FROM praxis_health_marks
             WHERE user_id=?1 AND mark_date>=?2 AND mark_date<=?3 ORDER BY mark_date DESC, dim_id",
        )
        .map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params![user_id, from, to], |r| {
            Ok(HealthMark {
                id: r.get(0)?,
                dim_id: r.get(1)?,
                mark_date: r.get(2)?,
                value: r.get(3)?,
                done: r.get::<_, i64>(4)? != 0,
                note: r.get(5)?,
            })
        })
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ===================== 周期指标 / 体检 metrics =====================

fn create_metric_impl(
    db: &Connection,
    user_id: &str,
    req: &MetricRequest,
) -> Result<HealthMetric, String> {
    if load_dim(db, user_id, req.dim_id).is_none() {
        return Err("维度不存在".into());
    }
    let date = match req.measured_at.as_deref().filter(|s| !s.is_empty()) {
        Some(d) => validate_date(d)?,
        None => today_str(),
    };
    let source = req.source.clone().unwrap_or_else(|| "self".into());
    let now = now_rfc3339();
    db.execute(
        "INSERT INTO praxis_health_metrics
           (user_id, dim_id, measured_at, value, text_value, unit, source, note, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            user_id, req.dim_id, date, req.value,
            req.text_value.clone().unwrap_or_default(),
            req.unit.clone().unwrap_or_default(), source,
            req.note.clone().unwrap_or_default(), now
        ],
    )
    .map_err(|e| format!("insert metric: {e}"))?;
    let id = db.last_insert_rowid();
    db.query_row(
        "SELECT id, dim_id, measured_at, value, text_value, unit, source, note FROM praxis_health_metrics WHERE id=?1",
        params![id],
        row_to_metric,
    )
    .map_err(|e| format!("reload metric: {e}"))
}

fn row_to_metric(r: &rusqlite::Row) -> rusqlite::Result<HealthMetric> {
    Ok(HealthMetric {
        id: r.get(0)?,
        dim_id: r.get(1)?,
        measured_at: r.get(2)?,
        value: r.get(3)?,
        text_value: r.get(4)?,
        unit: r.get(5)?,
        source: r.get(6)?,
        note: r.get(7)?,
    })
}

fn list_metrics_impl(
    db: &Connection,
    user_id: &str,
    dim_id: Option<i64>,
) -> Result<Vec<HealthMetric>, String> {
    let (sql, use_dim) = match dim_id {
        Some(_) => (
            "SELECT id, dim_id, measured_at, value, text_value, unit, source, note FROM praxis_health_metrics
             WHERE user_id=?1 AND dim_id=?2 AND deleted=0 ORDER BY measured_at DESC, id DESC".to_string(),
            true,
        ),
        None => (
            "SELECT id, dim_id, measured_at, value, text_value, unit, source, note FROM praxis_health_metrics
             WHERE user_id=?1 AND deleted=0 ORDER BY measured_at DESC, id DESC".to_string(),
            false,
        ),
    };
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = if use_dim {
        stmt.query_map(params![user_id, dim_id.unwrap()], row_to_metric)
    } else {
        stmt.query_map(params![user_id], row_to_metric)
    }
    .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ===================== 靶盘 board 装配 =====================

/// 近 `days` 天该维已打卡(done)的连续 streak（从今天往回数）。
fn streak_for(db: &Connection, user_id: &str, dim_id: i64) -> i64 {
    let mut stmt = match db.prepare(
        "SELECT mark_date FROM praxis_health_marks
         WHERE user_id=?1 AND dim_id=?2 AND done=1 ORDER BY mark_date DESC LIMIT 60",
    ) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let dates: Vec<String> = stmt
        .query_map(params![user_id, dim_id], |r| r.get::<_, String>(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let mut streak = 0i64;
    let mut cursor = Utc::now().date_naive();
    for ds in dates {
        let Ok(d) = NaiveDate::parse_from_str(&ds, "%Y-%m-%d") else {
            break;
        };
        // 允许今天还没打（从昨天算起也接），否则中断。
        if d == cursor || d == cursor - Duration::days(1) {
            streak += 1;
            cursor = d - Duration::days(1);
        } else {
            break;
        }
    }
    streak
}

/// 该维本周 AI 分（scores 表），没有则 None。
fn latest_score(db: &Connection, user_id: &str, week: &str, dim_id: i64) -> Option<(Option<i64>, String, String)> {
    db.query_row(
        "SELECT score, trend, explain FROM praxis_health_scores
         WHERE user_id=?1 AND week=?2 AND dim_id=?3",
        params![user_id, week, dim_id],
        |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
    )
    .optional()
    .ok()
    .flatten()
}

fn build_board(db: &Connection, user_id: &str, week: &str) -> Result<JsonValue, String> {
    let dims = list_dims_impl(db, user_id)?;
    let mut dim_json = Vec::new();
    for d in &dims {
        if d.archived {
            continue;
        }
        let sc = latest_score(db, user_id, week, d.id);
        let (score, trend, explain) = match sc {
            Some((s, t, e)) => (s, t, e),
            None => (None, String::new(), String::new()),
        };
        let streak = streak_for(db, user_id, d.id);
        // 最近一次指标（用于 watch 层展示实测值）
        let last_metric: Option<String> = db
            .query_row(
                "SELECT COALESCE(NULLIF(text_value,''), CAST(value AS TEXT)) FROM praxis_health_metrics
                 WHERE user_id=?1 AND dim_id=?2 AND deleted=0 ORDER BY measured_at DESC LIMIT 1",
                params![user_id, d.id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten();
        dim_json.push(json!({
            "id": d.id, "dimKey": d.dim_key, "name": d.name,
            "sector": d.sector, "ring": d.ring, "kind": d.kind,
            "unit": d.unit, "targetFloor": d.target_floor, "targetGoal": d.target_goal,
            "seeded": d.seeded,
            "score": score, "trend": trend, "explain": explain,
            "streak": streak, "lastMetric": last_metric,
        }));
    }
    let total = latest_score(db, user_id, week, 0)
        .map(|(s, t, e)| json!({ "score": s, "trend": t, "explain": e }))
        .unwrap_or_else(|| json!({ "score": null, "trend": "", "explain": "" }));
    Ok(json!({ "week": week, "dims": dim_json, "total": total }))
}

// ===================== M5 AI 打分 =====================

const SCORE_SYSTEM_PROMPT: &str = r#"你是「Praxis · 健康」板块的 AI 健康教练。用户把自己当一家公司经营，健康是可持续经营的基础资本。
你要给每个健康维度打「健康度」分——衡量的是「这块经营得健康/可持续吗」，不是「做得好不好」。

**打分硬规则**：
1. 可解释：每个分给一句"为什么"（引用留痕，如"光照 6/7 天↑、维D 0 次↓、心肺 3 周未测"）。绝不黑箱。
2. 重奖趋势与坚持(streak)，不只看绝对值：起点低不该一直打低分打击人——做了就涨。
3. 无留痕即无分：完全没有打卡/指标/信号的维度，score 给 null（前端显示"待测"），不要假装。
4. 负信号非对称：一条未处理的风险信号压过多条正向。

**趋势符号**只用其一：↑(明显向好) / ↗(缓升) / →(持平) / ↘(缓降) / ↓(明显恶化) / 空(无数据)。

只输出一个严格 JSON 对象，无任何解释或 Markdown：
{
  "dims": [ { "dimKey": "维度key", "score": 0-100 或 null, "trend": "↑/↗/→/↘/↓/空", "explain": "≤30字为什么" } ],
  "summary": "≤60字总评：先点最该管的，语气是邀请不是审判"
}
只给我传入维度列表里出现的 dimKey，分数是整数或 null。"#;

/// 汇总每维近况证据，喂给打分模型。
fn build_score_input(db: &Connection, user_id: &str, dims: &[HealthDim]) -> String {
    let today = Utc::now().date_naive();
    let since = (today - Duration::days(14)).format("%Y-%m-%d").to_string();
    let mut lines = vec![format!("今天 {}，以下是各维度近 14 天留痕：", today_str())];
    for d in dims {
        if d.archived {
            continue;
        }
        let streak = streak_for(db, user_id, d.id);
        let marks: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM praxis_health_marks WHERE user_id=?1 AND dim_id=?2 AND done=1 AND mark_date>=?3",
                params![user_id, d.id, since],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let last_metric: Option<String> = db
            .query_row(
                "SELECT measured_at || ' ' || COALESCE(NULLIF(text_value,''), CAST(value AS TEXT)) FROM praxis_health_metrics
                 WHERE user_id=?1 AND dim_id=?2 AND deleted=0 ORDER BY measured_at DESC LIMIT 1",
                params![user_id, d.id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional().ok().flatten().flatten();
        let mut ev = format!(
            "- {} [{}·{}·{}]：近14天打卡 {} 次、连续 {} 天",
            d.name, sector_cn(&d.sector), ring_cn(&d.ring), kind_cn(&d.kind), marks, streak
        );
        if let Some(m) = last_metric {
            ev.push_str(&format!("；最近实测 {m}"));
        }
        // 信号维度带上最近一条留痕的内容，AI 才能据此判断（T-297①，否则只见次数）。
        if d.kind == "signal" {
            let last_note: Option<String> = db
                .query_row(
                    "SELECT note FROM praxis_health_marks WHERE user_id=?1 AND dim_id=?2 AND note != ''
                     ORDER BY mark_date DESC LIMIT 1",
                    params![user_id, d.id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .optional().ok().flatten().flatten();
            if let Some(n) = last_note {
                ev.push_str(&format!("；最近信号「{n}」"));
            }
        }
        if !d.target_floor.is_empty() {
            ev.push_str(&format!("；基线 {}", d.target_floor));
        }
        ev.push_str(&format!("（key={}）", d.dim_key));
        lines.push(ev);
    }
    lines.join("\n")
}

fn sector_cn(s: &str) -> &'static str {
    match s {
        "move" => "动",
        "eat" => "吃",
        "rest" => "睡·恢复",
        _ => "体征·信号",
    }
}
fn ring_cn(s: &str) -> &'static str {
    match s {
        "core" => "核心",
        "mid" => "常规",
        _ => "观察",
    }
}
fn kind_cn(s: &str) -> &'static str {
    match s {
        "habit" => "每日打卡",
        "metric" => "周期自测",
        _ => "信号/自评",
    }
}
fn ring_weight(ring: &str) -> f64 {
    match ring {
        "core" => 3.0,
        "mid" => 2.0,
        _ => 1.0,
    }
}

fn parse_obj(text: &str) -> Option<JsonValue> {
    let t = text.trim();
    if let Ok(v @ JsonValue::Object(_)) = serde_json::from_str::<JsonValue>(t) {
        return Some(v);
    }
    let s = t.find('{')?;
    let e = t.rfind('}')?;
    if e <= s {
        return None;
    }
    serde_json::from_str::<JsonValue>(&t[s..=e]).ok()
}

// ===================== journal 派生信号 (M4) =====================

/// 自由文本身体信号 → 信号维度 key（T-297①）。仅命中已存在的信号维度。
fn signal_to_dimkey(sig: &str) -> Option<&'static str> {
    if sig.contains("掉发") || sig.contains("脱发") {
        Some("hairloss")
    } else if sig.contains("消化") || sig.contains("肠胃") || sig.contains("胃")
        || sig.contains("便秘") || sig.contains("腹泻") || sig.contains("拉肚")
    {
        Some("digestion")
    } else if sig.contains("精力") || sig.contains("疲") || sig.contains("累") || sig.contains("乏") {
        Some("energy")
    } else {
        None
    }
}

/// 只在该 (维度,日期) 尚无任何打卡时插入派生值（T-297⑥：不覆盖手动/已有值）。
/// 返回是否真的写入了一行。
fn derive_mark_if_absent(
    db: &Connection,
    user_id: &str,
    dim_id: i64,
    date: &str,
    value: Option<f64>,
    done: i64,
    note: &str,
) -> bool {
    let now = now_rfc3339();
    db.execute(
        "INSERT INTO praxis_health_marks (user_id, dim_id, mark_date, value, done, note, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?7)
         ON CONFLICT(user_id, dim_id, mark_date) DO NOTHING",
        params![user_id, dim_id, date, value, done, note, now],
    )
    .unwrap_or(0)
        > 0
}

/// 从已分析 journal 的 `structured.health` 派生身体信号 → 写入信号维度打卡。
/// 复用 `praxis_journal.structured`（analyze 提示产出可空 health 块）。
/// 映射：energy→精力(1-5) / sleep→睡眠 / moved→运动 / **signals[]→掉发/消化/精力等信号维度(T-297①)**。
/// 不覆盖已有打卡（T-297⑥），只补空缺。
fn derive_from_journals(db: &Connection, user_id: &str) -> Result<i64, String> {
    let dims = list_dims_impl(db, user_id)?;
    let key_id = |k: &str| dims.iter().find(|d| d.dim_key == k).map(|d| d.id);

    let mut stmt = db
        .prepare(
            "SELECT entry_date, structured FROM praxis_journal
             WHERE user_id=?1 AND deleted=0 AND structured != '' ORDER BY entry_date DESC LIMIT 120",
        )
        .map_err(|e| format!("prepare: {e}"))?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![user_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| format!("query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut written = 0i64;
    for (date, structured_s) in rows {
        let Ok(structured) = serde_json::from_str::<JsonValue>(&structured_s) else {
            continue;
        };
        let Some(h) = structured.get("health").filter(|v| v.is_object()) else {
            continue;
        };
        // energy: 1-5
        if let (Some(id), Some(e)) = (key_id("energy"), h.get("energy").and_then(|v| v.as_i64())) {
            if (1..=5).contains(&e)
                && derive_mark_if_absent(db, user_id, id, &date, Some(e as f64), 1, "今日经营派生")
            {
                written += 1;
            }
        }
        // moved: bool → 运动打卡
        if let (Some(id), Some(true)) = (key_id("move"), h.get("moved").and_then(|v| v.as_bool())) {
            if derive_mark_if_absent(db, user_id, id, &date, None, 1, "今日经营派生") {
                written += 1;
            }
        }
        // sleep: good/fair/poor → 睡眠（poor 记 done=0 表示有留痕但质量差）
        if let (Some(id), Some(sl)) = (key_id("sleep"), h.get("sleep").and_then(|v| v.as_str())) {
            let done = if sl == "poor" { 0 } else { 1 };
            if derive_mark_if_absent(db, user_id, id, &date, None, done, &format!("今日经营派生:{sl}")) {
                written += 1;
            }
        }
        // signals[]：自由文本身体信号 → 对应信号维度留痕（T-297①：修信号维度恒待测）。
        if let Some(sigs) = h.get("signals").and_then(|v| v.as_array()) {
            for sig in sigs {
                let Some(text) = sig.as_str().filter(|s| !s.trim().is_empty()) else {
                    continue;
                };
                let Some(k) = signal_to_dimkey(text) else { continue };
                if let Some(id) = key_id(k) {
                    if derive_mark_if_absent(db, user_id, id, &date, None, 1, &format!("今日经营派生:{text}")) {
                        written += 1;
                    }
                }
            }
        }
    }
    Ok(written)
}

// ===================== handlers =====================

#[derive(Debug, Deserialize, Default)]
pub struct BoardQuery {
    pub week: Option<String>,
}
#[derive(Debug, Deserialize, Default)]
pub struct MarksQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}
#[derive(Debug, Deserialize, Default)]
pub struct MetricsQuery {
    #[serde(rename = "dimId")]
    pub dim_id: Option<i64>,
}

/// String 错误 → 500 响应（内部错误一律脱敏）。
fn internal(ctx: &str, e: String) -> (StatusCode, Json<JsonValue>) {
    error!(target: "praxis_health", "{}: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

pub async fn list_dims(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match list_dims_impl(&db, &admin.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items })),
        ),
        Err(e) => internal("list_dims", e),
    }
}

pub async fn create_dim(
    State(state): State<AppState>,
    admin: AdminUserId,
    Json(req): Json<CreateDimRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match create_dim_impl(&db, &admin.0, &req) {
        Ok(item) => (StatusCode::OK, Json(json!({ "success": true, "item": item }))),
        Err(e) if is_user_err(&e) => bad(&e),
        Err(e) => {
            error!(target: "praxis_health", "create_dim: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn update_dim(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateDimRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match update_dim_impl(&db, &admin.0, id, &patch) {
        Ok(Some(item)) => (StatusCode::OK, Json(json!({ "success": true, "item": item }))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "未找到维度" }))),
        Err(e) if is_user_err(&e) => bad(&e),
        Err(e) => {
            error!(target: "praxis_health", "update_dim: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn delete_dim(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    match db.execute(
        "UPDATE praxis_health_dims SET deleted=1, updated_at=?1 WHERE id=?2 AND user_id=?3 AND deleted=0",
        params![now, id, &admin.0],
    ) {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "未找到维度" }))),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete_dim", e),
    }
}

pub async fn list_marks(
    State(state): State<AppState>,
    admin: AdminUserId,
    Query(q): Query<MarksQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match list_marks_impl(&db, &admin.0, q.from.as_deref(), q.to.as_deref()) {
        Ok(items) => (StatusCode::OK, Json(json!({ "success": true, "items": items }))),
        Err(e) => {
            error!(target: "praxis_health", "list_marks: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn upsert_mark(
    State(state): State<AppState>,
    admin: AdminUserId,
    Json(req): Json<MarkRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match upsert_mark_impl(&db, &admin.0, &req) {
        Ok(item) => (StatusCode::OK, Json(json!({ "success": true, "item": item }))),
        Err(e) if is_user_err(&e) => bad(&e),
        Err(e) => {
            error!(target: "praxis_health", "upsert_mark: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn list_metrics(
    State(state): State<AppState>,
    admin: AdminUserId,
    Query(q): Query<MetricsQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match list_metrics_impl(&db, &admin.0, q.dim_id) {
        Ok(items) => (StatusCode::OK, Json(json!({ "success": true, "items": items }))),
        Err(e) => {
            error!(target: "praxis_health", "list_metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn create_metric(
    State(state): State<AppState>,
    admin: AdminUserId,
    Json(req): Json<MetricRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match create_metric_impl(&db, &admin.0, &req) {
        Ok(item) => (StatusCode::OK, Json(json!({ "success": true, "item": item }))),
        Err(e) if is_user_err(&e) => bad(&e),
        Err(e) => {
            error!(target: "praxis_health", "create_metric: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn delete_metric(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    match db.execute(
        "UPDATE praxis_health_metrics SET deleted=1 WHERE id=?1 AND user_id=?2 AND deleted=0",
        params![id, &admin.0],
    ) {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "未找到记录" }))),
        Ok(_) => {
            let _ = now;
            (StatusCode::OK, Json(json!({ "success": true })))
        }
        Err(e) => db_error("delete_metric", e),
    }
}

pub async fn get_board(
    State(state): State<AppState>,
    admin: AdminUserId,
    Query(q): Query<BoardQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let week = q.week.filter(|s| !s.is_empty()).unwrap_or_else(this_week);
    match build_board(&db, &admin.0, &week) {
        Ok(board) => (StatusCode::OK, Json(json!({ "success": true, "board": board }))),
        Err(e) => {
            error!(target: "praxis_health", "get_board: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

pub async fn derive_signals(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match derive_from_journals(&db, &admin.0) {
        Ok(n) => (StatusCode::OK, Json(json!({ "success": true, "derived": n }))),
        Err(e) => {
            error!(target: "praxis_health", "derive: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

/// POST /health/score — 汇总留痕 → 调 LLM 打分 → 写 scores（含加权总分）→ 回 board。
pub async fn score_health(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> (StatusCode, Json<JsonValue>) {
    // 1. 载入维度 + 组织证据（短锁）。
    let (dims, prompt_input) = {
        let db = state.db.lock();
        let dims = match list_dims_impl(&db, &admin.0) {
            Ok(d) => d,
            Err(e) => {
                error!(target: "praxis_health", "score list dims: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })));
            }
        };
        let input = build_score_input(&db, &admin.0, &dims);
        (dims, input)
    };

    // 2. LLM 客户端（同二狗对话 key）。
    let client = match LlmClient::for_user(&state.db.lock(), &admin.0) {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "success": false, "error": "AI 服务未配置" })),
            )
        }
    };

    // 3. 调用（不持锁 await）。
    let reply = match client.simple_generate(SCORE_SYSTEM_PROMPT, &prompt_input, 1500).await {
        Ok(t) => t,
        Err(e) => {
            warn!(target: "praxis_health", "score llm error: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({ "success": false, "error": e, "retryable": true })));
        }
    };
    let parsed = match parse_obj(&reply) {
        Some(v) => v,
        None => {
            warn!(target: "praxis_health", "score parse fail");
            return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "success": false, "error": "AI 返回格式异常，可重试", "retryable": true })));
        }
    };

    // 4. 写分数（按 dimKey 匹配），并用 core3/mid2/watch1 加权算总分（规则3 落地）。
    let week = this_week();
    let now = now_rfc3339();
    let empty = vec![];
    let ai_dims = parsed.get("dims").and_then(|v| v.as_array()).unwrap_or(&empty);
    let mut weighted_sum = 0.0f64;
    let mut weight_total = 0.0f64;
    {
        let db = state.db.lock();
        for d in &dims {
            let Some(entry) = ai_dims.iter().find(|e| {
                e.get("dimKey").and_then(|k| k.as_str()) == Some(d.dim_key.as_str())
            }) else {
                continue;
            };
            let score = entry.get("score").and_then(|v| v.as_i64());
            let trend = entry.get("trend").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let explain = entry.get("explain").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if let Err(e) = db.execute(
                "INSERT INTO praxis_health_scores (user_id, week, dim_id, score, trend, explain, computed_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(user_id, week, dim_id) DO UPDATE SET score=excluded.score, trend=excluded.trend, explain=excluded.explain, computed_at=excluded.computed_at",
                params![&admin.0, week, d.id, score, trend, explain, now],
            ) {
                return db_error("score write", e);
            }
            if let Some(s) = score {
                let w = ring_weight(&d.ring);
                weighted_sum += s as f64 * w;
                weight_total += w;
                // T-297③ 动态优先级：仅系统种子维度、每次至多挪一层——
                // 做绿(≥80)的核心维移出到常规；恶化(<40)的常规维移进核心。用户自定义维度不动。
                if d.seeded {
                    let new_ring = match (d.ring.as_str(), s) {
                        ("core", s) if s >= 80 => Some("mid"),
                        ("mid", s) if s < 40 => Some("core"),
                        _ => None,
                    };
                    if let Some(nr) = new_ring {
                        let _ = db.execute(
                            "UPDATE praxis_health_dims SET ring=?1, updated_at=?2 WHERE id=?3 AND user_id=?4 AND deleted=0",
                            params![nr, now, d.id, &admin.0],
                        );
                    }
                }
            }
        }
        // 总分：加权平均（四舍五入），explain 用 AI summary。
        let total_score = if weight_total > 0.0 {
            Some((weighted_sum / weight_total).round() as i64)
        } else {
            None
        };
        let summary = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if let Err(e) = db.execute(
            "INSERT INTO praxis_health_scores (user_id, week, dim_id, score, trend, explain, computed_at)
             VALUES (?1,?2,0,?3,'',?4,?5)
             ON CONFLICT(user_id, week, dim_id) DO UPDATE SET score=excluded.score, explain=excluded.explain, computed_at=excluded.computed_at",
            params![&admin.0, week, total_score, summary, now],
        ) {
            return db_error("total write", e);
        }
    }

    // 5. 回 board。
    let db = state.db.lock();
    match build_board(&db, &admin.0, &week) {
        Ok(board) => (StatusCode::OK, Json(json!({ "success": true, "board": board }))),
        Err(e) => {
            error!(target: "praxis_health", "score board: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "内部错误" })))
        }
    }
}

/// 用户可读错误（→400）判定。
fn is_user_err(e: &str) -> bool {
    e.contains("请填")
        || e.contains("非法")
        || e.contains("过长")
        || e.contains("不存在")
        || e.contains("须为")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_admin_user, create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn health_requires_admin() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "ph-user", "Pa55word1");
        let app = crate::build_app(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/health/dims")
                    .header("Cookie", auth_cookie(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn health_seeds_dims_and_marks_and_board() {
        let state = test_state();
        let (_aid, token) = create_admin_user(&state, "ph-admin", "Pa55word1");
        let app = crate::build_app(state);

        // 首次列维度 → 自动种子 17 维
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/health/dims")
                    .header("Cookie", auth_cookie(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["items"].as_array().unwrap().len(), 17);
        let light = j["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["dimKey"] == "light")
            .unwrap();
        let light_id = light["id"].as_i64().unwrap();
        assert_eq!(light["sector"], "rest");
        assert_eq!(light["ring"], "core");

        // 打卡今日光照
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/health/marks")
                    .header("Cookie", auth_cookie(&token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"dimId":{light_id},"value":25,"done":true}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);

        // board 含该维 streak≥1
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/health/board")
                    .header("Cookie", auth_cookie(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let b = body_json(resp).await;
        assert_eq!(b["success"], true);
        let dims = b["board"]["dims"].as_array().unwrap();
        let light = dims.iter().find(|d| d["dimKey"] == "light").unwrap();
        assert!(light["streak"].as_i64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn health_custom_dim_and_metric_and_isolation() {
        let state = test_state();
        let (_aid, token) = create_admin_user(&state, "ph-a2", "Pa55word1");
        let (_oid, other) = create_admin_user(&state, "ph-o2", "Pa55word1");
        let app = crate::build_app(state);

        // 自定义维度
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/health/dims")
                    .header("Cookie", auth_cookie(&token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"name":"冥想","sector":"rest","ring":"mid","kind":"habit","unit":"分钟"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["name"], "冥想");
        assert_eq!(j["item"]["seeded"], false);
        let dim_id = j["item"]["id"].as_i64().unwrap();

        // 录一条指标
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/health/metrics")
                    .header("Cookie", auth_cookie(&token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"dimId":{dim_id},"value":10,"unit":"分钟","source":"self"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);

        // 别的 admin 看到的是自己独立种子的 17 维（隔离）
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/health/dims")
                    .header("Cookie", auth_cookie(&other))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j2 = body_json(resp).await;
        assert_eq!(j2["items"].as_array().unwrap().len(), 17);
        assert!(!j2["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["name"] == "冥想"));
    }

    #[test]
    fn iso_week_format() {
        assert_eq!(iso_week_of("2026-07-02"), "2026-W27");
        assert_eq!(ring_weight("core"), 3.0);
    }

    #[test]
    fn signal_keyword_mapping() {
        assert_eq!(signal_to_dimkey("最近掉发严重"), Some("hairloss"));
        assert_eq!(signal_to_dimkey("消化不良"), Some("digestion"));
        assert_eq!(signal_to_dimkey("很累精力差"), Some("energy"));
        assert_eq!(signal_to_dimkey("心情不错"), None);
    }

    // T-297⑥：删光维度后再访问不重播种（种过的用户不再种）。
    #[tokio::test]
    async fn health_no_reseed_after_delete_all() {
        let state = test_state();
        let (_aid, token) = create_admin_user(&state, "ph-reseed", "Pa55word1");
        let app = crate::build_app(state);

        // 首访种 17 维
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/health/dims")
                    .header("Cookie", auth_cookie(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let ids: Vec<i64> = body_json(resp).await["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id"].as_i64().unwrap())
            .collect();
        assert_eq!(ids.len(), 17);

        // 删光
        for id in &ids {
            let _ = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri(format!("/api/praxis/health/dims/{id}"))
                        .header("Cookie", auth_cookie(&token))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        // 再访问：不应重新种子（保持空）
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/health/dims")
                    .header("Cookie", auth_cookie(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["items"].as_array().unwrap().len(), 0);
    }
}
