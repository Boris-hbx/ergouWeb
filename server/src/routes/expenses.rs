use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Datelike;
use serde::Serialize;
use serde_json::json;

use crate::auth::{check_guest_ai_quota, ActiveUserId, UserId};
use crate::models::expense::*;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ExpenseListResponse {
    pub success: bool,
    pub entries: Vec<ExpenseEntry>,
}

#[derive(Debug, Serialize)]
pub struct ExpenseDetailResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<ExpenseEntryDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExpenseSimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<ExpenseEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TagsResponse {
    pub success: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// Shared receipt parsing system prompt — used by both parse_receipts and parse_preview
const RECEIPT_PARSE_PROMPT: &str = r#"你是消费记录分析专家。根据提供的信息（图片、文字描述、或两者结合）提取消费信息。

核心原则：**无论输入是什么形式，都必须输出 JSON，绝不输出其他文字。**

输入形式：
- 可能只有收据/账单照片
- 可能只有用户的文字描述（如"星巴克拿铁 35元"）
- 可能同时有照片和文字描述，此时综合分析，文字可补充照片信息

处理规则：
1. 超市/商场收据（有商品明细）：逐项提取所有商品到 items 数组。
2. 餐厅账单：如果有菜品明细就逐项提取；如果只有总计，items 放一项概括（如"餐饮消费"）。
3. 加油/停车/单项消费：items 放一项，specs 里写详情（如升数、单价）。
4. 付款凭证（只有金额没有商品）：items 放一项，name 写商家或消费类型。
5. 有小费的单据：total_amount 填实际刷卡金额（含小费），tip 填小费金额。
6. 多张照片涉及不同日期的单据：每个 item 加上 "date": "YYYY-MM-DD" 字段。顶层 date 填最早日期。
7. 纯文字描述：根据描述尽可能提取商家、金额、标签等信息。items 至少放一项。

商品提取规则：
- 多张图片可能是同一张长收据的不同部分，有重叠。按 amount 去重，不要遗漏。
- name 优先用中文名，英文名放 specs。只有英文则 name 用英文。
- 称重商品：quantity=重量(kg)，unit_price=每kg单价，specs 写原始描述。
- 每项 amount 直接抄收据数字，不要自己算。

必须严格输出以下 JSON（不要输出任何其他文字）：
{
  "merchant": "商家名称",
  "date": "YYYY-MM-DD",
  "currency": "CAD",
  "tags": ["超市", "肉类"],
  "items": [
    { "name": "商品名", "quantity": 1, "unit_price": 12.5, "amount": 12.5, "specs": "", "date": "2026-02-11" }
  ],
  "subtotal": 100.00,
  "tax": 0.58,
  "tip": 0,
  "total_amount": 100.58
}

total_amount = 最终刷卡/支付金额（含税含小费）。
tags：场景标签（超市、餐饮、加油）+ 内容标签（肉类、蔬菜等）。"#;

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<ExpenseEntry> {
    let tags_json: String = row.get(4)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let ai_int: i32 = row.get(5)?;

    Ok(ExpenseEntry {
        id: row.get(0)?,
        amount: row.get(1)?,
        date: row.get(2)?,
        notes: row.get(3)?,
        tags,
        ai_processed: ai_int != 0,
        currency: row.get::<_, String>(6).unwrap_or_else(|_| "CAD".into()),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        photo_count: row.get(9).unwrap_or(0),
        item_count: row.get(10).unwrap_or(0),
    })
}

// ===== List entries =====
pub async fn list_entries(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<ExpenseListQuery>,
) -> (StatusCode, Json<ExpenseListResponse>) {
    let db = state.db.lock();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let from = query.from.unwrap_or_else(|| "2020-01-01".to_string());
    let to = query.to.unwrap_or(today);

    let sql = "
        SELECT e.id, e.amount, e.date, e.notes, e.tags, e.ai_processed, e.currency, e.created_at, e.updated_at,
               (SELECT COUNT(*) FROM expense_photos WHERE entry_id = e.id) as photo_count,
               (SELECT COUNT(*) FROM expense_items WHERE entry_id = e.id) as item_count
        FROM expense_entries e
        WHERE e.user_id = ?1 AND e.date >= ?2 AND e.date <= ?3
        ORDER BY e.date DESC, e.created_at DESC
    ";

    let mut stmt = match db.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Expense] list error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExpenseListResponse {
                    success: false,
                    entries: vec![],
                }),
            );
        }
    };

    let entries: Vec<ExpenseEntry> = match stmt
        .query_map(rusqlite::params![user_id.0, from, to], |row| {
            row_to_entry(row)
        }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[Expense] query error: {}", e);
            vec![]
        }
    };

    // Filter by tags if specified
    let entries = if let Some(tag_filter) = &query.tags {
        let filter_tags: Vec<&str> = tag_filter.split(',').collect();
        entries
            .into_iter()
            .filter(|e| filter_tags.iter().any(|ft| e.tags.iter().any(|t| t == ft)))
            .collect()
    } else {
        entries
    };

    (
        StatusCode::OK,
        Json(ExpenseListResponse {
            success: true,
            entries,
        }),
    )
}

// ===== Create entry =====
pub async fn create_entry(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<CreateExpenseRequest>,
) -> (StatusCode, Json<ExpenseSimpleResponse>) {
    if req.amount <= 0.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ExpenseSimpleResponse {
                success: false,
                entry: None,
                message: Some("金额必须大于0".into()),
            }),
        );
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let date = req
        .date
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let notes = req.notes.unwrap_or_default();
    let tags = req.tags.unwrap_or_default();
    let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into());

    let ai_processed = req.ai_processed.unwrap_or(false);
    let ai_flag: i32 = if ai_processed { 1 } else { 0 };
    let currency = req.currency.as_deref().unwrap_or("CAD");

    let db = state.db.lock();
    let result = db.execute(
        "INSERT INTO expense_entries (id, user_id, amount, date, notes, tags, ai_processed, currency, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        rusqlite::params![id, user_id.0, req.amount, date, notes, tags_json, ai_flag, currency, now],
    );

    match result {
        Ok(_) => {
            // Insert items if provided (from preview)
            let item_count = if let Some(items) = &req.items {
                for (i, item) in items.iter().enumerate() {
                    let item_id = uuid::Uuid::new_v4().to_string();
                    db.execute(
                        "INSERT INTO expense_items (id, entry_id, name, quantity, unit_price, amount, specs, sort_order)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        rusqlite::params![
                            item_id,
                            id,
                            item.name,
                            item.quantity,
                            item.unit_price,
                            item.amount,
                            item.specs.as_deref().unwrap_or(""),
                            i as i32
                        ],
                    )
                    .ok();
                }
                items.len() as i64
            } else {
                0
            };

            // If no tags and notes are present, trigger auto-tagging via text
            let should_auto_tag = tags.is_empty() && !notes.is_empty() && !ai_processed;
            if should_auto_tag {
                let entry_id = id.clone();
                let amount = req.amount;
                let notes_clone = notes.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    auto_tag_from_text(&state_clone, &entry_id, amount, &notes_clone).await;
                });
            }

            (
                StatusCode::CREATED,
                Json(ExpenseSimpleResponse {
                    success: true,
                    entry: Some(ExpenseEntry {
                        id,
                        amount: req.amount,
                        date,
                        notes,
                        tags,
                        ai_processed,
                        currency: currency.to_string(),
                        created_at: now.clone(),
                        updated_at: now,
                        photo_count: 0,
                        item_count,
                    }),
                    message: None,
                }),
            )
        }
        Err(e) => {
            eprintln!("[Expense] create error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExpenseSimpleResponse {
                    success: false,
                    entry: None,
                    message: Some("创建失败".into()),
                }),
            )
        }
    }
}

// ===== Get entry detail =====
pub async fn get_entry(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<ExpenseDetailResponse>) {
    let db = state.db.lock();

    let entry = db.query_row(
        "SELECT e.id, e.amount, e.date, e.notes, e.tags, e.ai_processed, e.currency, e.created_at, e.updated_at,
                (SELECT COUNT(*) FROM expense_photos WHERE entry_id = e.id),
                (SELECT COUNT(*) FROM expense_items WHERE entry_id = e.id)
         FROM expense_entries e WHERE e.id = ?1 AND e.user_id = ?2",
        rusqlite::params![id, user_id.0],
        row_to_entry,
    );

    let entry = match entry {
        Ok(e) => e,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ExpenseDetailResponse {
                    success: false,
                    entry: None,
                    message: Some("条目不存在".into()),
                }),
            );
        }
    };

    // Get items
    let items: Vec<ExpenseItem> = db
        .prepare(
            "SELECT id, entry_id, name, quantity, unit_price, amount, specs, sort_order
             FROM expense_items WHERE entry_id = ?1 ORDER BY sort_order",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok(ExpenseItem {
                    id: row.get(0)?,
                    entry_id: row.get(1)?,
                    name: row.get(2)?,
                    quantity: row.get(3)?,
                    unit_price: row.get(4)?,
                    amount: row.get(5)?,
                    specs: row.get(6)?,
                    sort_order: row.get(7)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Get photos
    let photos: Vec<ExpensePhoto> = db
        .prepare(
            "SELECT id, entry_id, filename, file_size, mime_type, created_at, storage_path
             FROM expense_photos WHERE entry_id = ?1 ORDER BY created_at",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| {
                Ok(ExpensePhoto {
                    id: row.get(0)?,
                    entry_id: row.get(1)?,
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

    (
        StatusCode::OK,
        Json(ExpenseDetailResponse {
            success: true,
            entry: Some(ExpenseEntryDetail {
                entry,
                items,
                photos,
            }),
            message: None,
        }),
    )
}

// ===== Update entry =====
pub async fn update_entry(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
    Json(req): Json<UpdateExpenseRequest>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    // Check ownership
    let exists: bool = db
        .query_row(
            "SELECT COUNT(*) FROM expense_entries WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("条目不存在".into()),
            }),
        );
    }

    let mut sets = vec!["updated_at = ?1".to_string()];
    let mut idx = 2u32;

    // Build dynamic SET clause
    macro_rules! maybe_set {
        ($field:expr, $name:expr) => {
            if $field.is_some() {
                sets.push(format!("{} = ?{}", $name, idx));
                idx += 1;
            }
        };
    }

    maybe_set!(req.amount, "amount");
    maybe_set!(req.date, "date");
    maybe_set!(req.notes, "notes");

    let tags_json = req
        .tags
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".into()));
    if tags_json.is_some() {
        sets.push(format!("tags = ?{}", idx));
        idx += 1;
    }
    maybe_set!(req.currency, "currency");
    let _ = idx; // suppress warning

    let sql = format!(
        "UPDATE expense_entries SET {} WHERE id = ?{} AND user_id = ?{}",
        sets.join(", "),
        idx,
        idx + 1
    );

    // Build params dynamically using a trait-object vec
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
    if let Some(amount) = req.amount {
        params.push(Box::new(amount));
    }
    if let Some(date) = &req.date {
        params.push(Box::new(date.clone()));
    }
    if let Some(notes) = &req.notes {
        params.push(Box::new(notes.clone()));
    }
    if let Some(tj) = &tags_json {
        params.push(Box::new(tj.clone()));
    }
    if let Some(currency) = &req.currency {
        params.push(Box::new(currency.clone()));
    }
    params.push(Box::new(id));
    params.push(Box::new(user_id.0));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    match db.execute(&sql, param_refs.as_slice()) {
        Ok(_) => (
            StatusCode::OK,
            Json(SimpleResponse {
                success: true,
                message: None,
            }),
        ),
        Err(e) => {
            eprintln!("[Expense] update error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SimpleResponse {
                    success: false,
                    message: Some("更新失败".into()),
                }),
            )
        }
    }
}

// ===== Delete entry =====
pub async fn delete_entry(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    // Get photos to delete files
    let photos: Vec<String> = db
        .prepare("SELECT storage_path FROM expense_photos WHERE entry_id = ?1")
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let result = db.execute(
        "DELETE FROM expense_entries WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id.0],
    );

    match result {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("条目不存在".into()),
            }),
        ),
        Ok(_) => {
            // Delete photo files
            for path in photos {
                std::fs::remove_file(&path).ok();
            }
            (
                StatusCode::OK,
                Json(SimpleResponse {
                    success: true,
                    message: None,
                }),
            )
        }
        Err(e) => {
            eprintln!("[Expense] delete error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SimpleResponse {
                    success: false,
                    message: Some("删除失败".into()),
                }),
            )
        }
    }
}

// ===== Stats (unified) =====
pub async fn get_stats(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<ExpenseStatsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let today = chrono::Local::now().date_naive();
    let ref_date = match query.date.as_ref() {
        Some(d) => match chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "invalid date format, expected YYYY-MM-DD" })),
                );
            }
        },
        None => today,
    };

    let (from_date, to_date) = match query.period.as_str() {
        "day" => (ref_date, ref_date),
        "week" => {
            let weekday = ref_date.weekday().num_days_from_monday();
            let start = ref_date - chrono::Duration::days(weekday as i64);
            let end = start + chrono::Duration::days(6);
            (start, end)
        }
        "month" => {
            let start = chrono::NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 1)
                .unwrap_or(ref_date);
            let end = if ref_date.month() == 12 {
                chrono::NaiveDate::from_ymd_opt(ref_date.year() + 1, 1, 1)
            } else {
                chrono::NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month() + 1, 1)
            }
            .map(|d| d - chrono::Duration::days(1))
            .unwrap_or(ref_date);
            (start, end)
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid period, must be day/week/month" })),
            );
        }
    };

    let from = from_date.to_string();
    let to = to_date.to_string();

    let db = state.db.lock();

    // Query all entries in range
    let mut tag_map: std::collections::HashMap<String, (f64, i64)> =
        std::collections::HashMap::new();
    let mut cat_map: std::collections::HashMap<&str, (f64, i64)> =
        std::collections::HashMap::new();
    let mut day_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut total_amount: f64 = 0.0;
    let mut entry_count: i64 = 0;

    if let Ok(mut stmt) = db.prepare(
        "SELECT amount, date, tags FROM expense_entries WHERE user_id = ?1 AND date >= ?2 AND date <= ?3",
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![user_id.0, from, to], |row| {
            Ok((
                row.get::<_, f64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) {
            for row in rows.flatten() {
                let (amount, date, tags_json) = row;
                total_amount += amount;
                entry_count += 1;

                // tag_totals
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                for tag in &tags {
                    let e = tag_map.entry(tag.clone()).or_insert((0.0, 0));
                    e.0 += amount;
                    e.1 += 1;
                }

                // category_totals
                let cat = entry_category(&tags_json);
                let e = cat_map.entry(cat).or_insert((0.0, 0));
                e.0 += amount;
                e.1 += 1;

                // daily
                *day_map.entry(date).or_insert(0.0) += amount;
            }
        }
    }

    // Build tag_totals sorted by amount desc
    let mut tag_totals: Vec<TagTotal> = tag_map
        .into_iter()
        .map(|(tag, (amount, count))| TagTotal { tag, amount, count })
        .collect();
    tag_totals.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Build category_totals sorted by amount desc
    let mut category_totals: Vec<CategoryTotal> = cat_map
        .into_iter()
        .map(|(cat, (amount, count))| {
            let percentage = if total_amount > 0.0 {
                (amount / total_amount * 1000.0).round() / 10.0
            } else {
                0.0
            };
            CategoryTotal {
                category: cat.to_string(),
                amount,
                count,
                percentage,
            }
        })
        .collect();
    category_totals.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Build daily totals — every date in range, 0 for missing
    let mut daily: Vec<DailyTotal> = Vec::new();
    let mut d = from_date;
    while d <= to_date {
        let ds = d.to_string();
        let amt = day_map.get(&ds).copied().unwrap_or(0.0);
        daily.push(DailyTotal {
            date: ds,
            amount: amt,
        });
        d += chrono::Duration::days(1);
    }

    // Comparison: query previous period total
    let (prev_from, prev_to) = match query.period.as_str() {
        "day" => {
            let prev = ref_date - chrono::Duration::days(1);
            (prev, prev)
        }
        "week" => {
            let prev_start = from_date - chrono::Duration::days(7);
            let prev_end = from_date - chrono::Duration::days(1);
            (prev_start, prev_end)
        }
        _ => {
            // month: previous month
            let prev_end = from_date - chrono::Duration::days(1);
            let prev_start =
                chrono::NaiveDate::from_ymd_opt(prev_end.year(), prev_end.month(), 1)
                    .unwrap_or(prev_end);
            (prev_start, prev_end)
        }
    };

    let prev_total: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM expense_entries WHERE user_id = ?1 AND date >= ?2 AND date <= ?3",
            rusqlite::params![user_id.0, prev_from.to_string(), prev_to.to_string()],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let change_percent = if prev_total == 0.0 && total_amount > 0.0 {
        100.0
    } else if prev_total == 0.0 {
        0.0
    } else {
        ((total_amount - prev_total) / prev_total * 1000.0).round() / 10.0
    };

    let stats = ExpenseStats {
        period: query.period,
        from,
        to,
        total_amount,
        entry_count,
        tag_totals,
        category_totals,
        daily,
        comparison: Comparison {
            prev_total,
            change_percent,
        },
    };

    (StatusCode::OK, Json(json!({ "success": true, "stats": stats })))
}

// ===== List tags =====
pub async fn list_tags(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<TagsResponse>) {
    let db = state.db.lock();
    let mut all_tags: Vec<String> = Vec::new();

    if let Ok(mut stmt) = db.prepare("SELECT tags FROM expense_entries WHERE user_id = ?1") {
        if let Ok(rows) =
            stmt.query_map(rusqlite::params![user_id.0], |row| row.get::<_, String>(0))
        {
            for row in rows.flatten() {
                let tags: Vec<String> = serde_json::from_str(&row).unwrap_or_default();
                for tag in tags {
                    if !all_tags.contains(&tag) {
                        all_tags.push(tag);
                    }
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(TagsResponse {
            success: true,
            tags: all_tags,
        }),
    )
}

// ===== Upload photos =====
pub async fn upload_photos(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(entry_id): Path<String>,
    mut multipart: Multipart,
) -> (StatusCode, Json<serde_json::Value>) {
    // Check ownership
    {
        let db = state.db.lock();
        let exists: bool = db
            .query_row(
                "SELECT COUNT(*) FROM expense_entries WHERE id = ?1 AND user_id = ?2",
                rusqlite::params![entry_id, user_id.0],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "条目不存在" })),
            );
        }
    }

    // Ensure upload directory
    let upload_dir = format!(
        "{}/uploads/{}",
        std::env::var("DATABASE_PATH")
            .unwrap_or_else(|_| "data/next.db".to_string())
            .replace("/next.db", "")
            .replace("\\next.db", ""),
        user_id.0
    );
    std::fs::create_dir_all(&upload_dir).ok();

    let mut uploaded: Vec<ExpensePhoto> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field.file_name().unwrap_or("photo.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpeg").to_string();

        // Validate mime type
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
            "INSERT INTO expense_photos (id, entry_id, filename, storage_path, file_size, mime_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                photo_id, entry_id, filename, storage_path, file_size, content_type, now
            ],
        );

        if result.is_ok() {
            uploaded.push(ExpensePhoto {
                id: photo_id,
                entry_id: entry_id.clone(),
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
        Json(json!({
            "success": true,
            "photos": uploaded,
            "count": uploaded.len()
        })),
    )
}

// ===== Delete photo =====
pub async fn delete_photo(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(photo_id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    // Get photo info + verify ownership
    let photo_info: Option<String> = db
        .query_row(
            "SELECT p.storage_path FROM expense_photos p
             JOIN expense_entries e ON p.entry_id = e.id
             WHERE p.id = ?1 AND e.user_id = ?2",
            rusqlite::params![photo_id, user_id.0],
            |row| row.get(0),
        )
        .ok();

    match photo_info {
        Some(path) => {
            db.execute(
                "DELETE FROM expense_photos WHERE id = ?1",
                rusqlite::params![photo_id],
            )
            .ok();
            std::fs::remove_file(&path).ok();
            (
                StatusCode::OK,
                Json(SimpleResponse {
                    success: true,
                    message: None,
                }),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("照片不存在".into()),
            }),
        ),
    }
}

// ===== Serve photo =====
pub async fn serve_photo(
    State(state): State<AppState>,
    user_id: UserId,
    Path((path_user_id, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    // Path traversal protection
    if path_user_id.contains("..")
        || path_user_id.contains('/')
        || path_user_id.contains('\\')
        || filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Auth check: user can access their own photos, or trip collaborator photos
    if user_id.0 != path_user_id {
        // Fallback: check if user is a trip collaborator for any trip owned by path_user_id
        let db = state.db.lock();
        let is_trip_collab: bool = db
            .query_row(
                "SELECT COUNT(*) FROM trip_collaborators tc
                 JOIN trips t ON t.id = tc.trip_id
                 WHERE tc.user_id = ?1 AND t.user_id = ?2",
                rusqlite::params![user_id.0, path_user_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !is_trip_collab {
            return (StatusCode::FORBIDDEN, "Access denied").into_response();
        }
    }

    let upload_dir = format!(
        "{}/uploads/{}",
        std::env::var("DATABASE_PATH")
            .unwrap_or_else(|_| "data/next.db".to_string())
            .replace("/next.db", "")
            .replace("\\next.db", ""),
        path_user_id
    );
    let file_path = format!("{}/{}", upload_dir, filename);

    match tokio::fs::read(&file_path).await {
        Ok(data) => {
            let mime = if filename.ends_with(".png") {
                "image/png"
            } else if filename.ends_with(".gif") {
                "image/gif"
            } else if filename.ends_with(".webp") {
                "image/webp"
            } else {
                "image/jpeg"
            };

            (
                StatusCode::OK,
                [
                    (http::header::CONTENT_TYPE, mime),
                    (http::header::CACHE_CONTROL, "private, max-age=86400"),
                ],
                Body::from(data),
            )
                .into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

// ===== Parse receipts (AI) =====
pub async fn parse_receipts(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(entry_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Guest AI quota check
    let guest_ai_remaining = match check_guest_ai_quota(&state, &user_id.0) {
        Ok(r) => r,
        Err(e) => return e,
    };

    // Get photos and notes for this entry
    let photos: Vec<(String, String)>;
    let entry_notes: String;
    let entry_amount: f64;
    {
        let db = state.db.lock();

        // Verify ownership and get notes/amount
        let entry_info = db.query_row(
            "SELECT notes, amount FROM expense_entries WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![entry_id, user_id.0],
            |row| Ok((row.get::<_, String>(0).unwrap_or_default(), row.get::<_, f64>(1).unwrap_or(0.0))),
        );

        match entry_info {
            Ok((notes, amount)) => {
                entry_notes = notes;
                entry_amount = amount;
            }
            Err(_) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "success": false, "message": "条目不存在" })),
                );
            }
        }

        photos = db
            .prepare("SELECT storage_path, mime_type FROM expense_photos WHERE entry_id = ?1")
            .and_then(|mut stmt| {
                stmt.query_map(rusqlite::params![entry_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();
    }

    let has_notes = !entry_notes.trim().is_empty() || entry_amount > 0.0;

    if photos.is_empty() && !has_notes {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "没有照片或文字可分析" })),
        );
    }

    // Read photo files and encode to base64
    let mut images: Vec<(String, String)> = Vec::new();
    for (path, mime) in &photos {
        if let Ok(data) = std::fs::read(path) {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
            images.push((b64, mime.clone()));
        }
    }

    // Call LLM API
    let client = match crate::services::llm::LlmClient::for_user(&state.db.lock(), &user_id.0) {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "success": false, "message": "AI 服务未配置" })),
            );
        }
    };

    // Build user message with available context
    let text_context = build_analysis_text(&entry_notes, entry_amount);
    let user_msg = build_user_message(&text_context, &images);

    let ai_result = if images.is_empty() {
        // Text-only: use simple_generate
        client.simple_generate(RECEIPT_PARSE_PROMPT, &user_msg, 8192).await
    } else {
        // Has images (possibly with text): use vision_generate
        client.vision_generate(RECEIPT_PARSE_PROMPT, images, &user_msg, 8192).await
    };

    match ai_result {
        Ok(text) => {
            // Parse the JSON response
            let parsed = parse_ai_receipt_response(&text);
            let db = state.db.lock();

            let now = chrono::Utc::now().to_rfc3339();
            let tags_json = parsed
                .tags
                .as_ref()
                .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".into()))
                .unwrap_or_else(|| "[]".into());

            // Use total_amount for the entry amount; fallback to subtotal if total is not available
            let amount = parsed.total_amount.or(parsed.subtotal);
            let date = parsed.date.clone();

            db.execute(
                "UPDATE expense_entries SET tags = ?1, ai_processed = 1, updated_at = ?2, amount = COALESCE(?3, amount), date = COALESCE(?4, date) WHERE id = ?5",
                rusqlite::params![tags_json, now, amount, date, entry_id],
            ).ok();

            // Insert items (clear existing first)
            db.execute(
                "DELETE FROM expense_items WHERE entry_id = ?1",
                rusqlite::params![entry_id],
            )
            .ok();

            for (i, item) in parsed.items.iter().enumerate() {
                let item_id = uuid::Uuid::new_v4().to_string();
                db.execute(
                    "INSERT INTO expense_items (id, entry_id, name, quantity, unit_price, amount, specs, sort_order)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        item_id,
                        entry_id,
                        item.name,
                        item.quantity,
                        item.unit_price,
                        item.amount,
                        item.specs,
                        i as i32
                    ],
                )
                .ok();
            }

            let mut resp = json!({
                "success": true,
                "tags": parsed.tags,
                "items_count": parsed.items.len(),
                "total_amount": parsed.total_amount,
                "merchant": parsed.merchant
            });
            if guest_ai_remaining < 999 {
                resp["ai_remaining"] = json!(guest_ai_remaining);
            }
            (StatusCode::OK, Json(resp))
        }
        Err(e) => {
            eprintln!("[Expense] AI parse error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": e })),
            )
        }
    }
}

// ===== Parse Preview (AI, no DB write) =====
pub async fn parse_preview(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<ParsePreviewRequest>,
) -> (StatusCode, Json<ParsePreviewResponse>) {
    // Guest AI quota check
    let guest_ai_remaining = match check_guest_ai_quota(&state, &user_id.0) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(ParsePreviewResponse {
                    success: false,
                    preview: None,
                    message: Some("AI 体验次数已用完，注册解锁无限使用".into()),
                    ai_remaining: Some(0),
                }),
            );
        }
    };

    let has_text = req.text.as_ref().is_some_and(|t| !t.trim().is_empty());

    if req.images.is_empty() && !has_text {
        return (
            StatusCode::BAD_REQUEST,
            Json(ParsePreviewResponse {
                success: false,
                preview: None,
                message: Some("请提供照片或文字描述".into()),
                ai_remaining: None,
            }),
        );
    }

    let client = match crate::services::llm::LlmClient::for_user(&state.db.lock(), &user_id.0) {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ParsePreviewResponse {
                    success: false,
                    preview: None,
                    message: Some("AI 服务未配置".into()),
                    ai_remaining: None,
                }),
            );
        }
    };

    let images: Vec<(String, String)> = req
        .images
        .into_iter()
        .map(|img| (img.data, img.mime_type))
        .collect();

    let text_ref = req.text.as_deref().unwrap_or("");
    let user_msg = build_user_message(text_ref, &images);

    let ai_result = if images.is_empty() {
        // Text-only: use simple_generate
        client.simple_generate(RECEIPT_PARSE_PROMPT, &user_msg, 8192).await
    } else {
        // Has images (possibly with text): use vision_generate
        client.vision_generate(RECEIPT_PARSE_PROMPT, images, &user_msg, 8192).await
    };

    match ai_result {
        Ok(text) => {
            let parsed = parse_ai_receipt_response(&text);
            (
                StatusCode::OK,
                Json(ParsePreviewResponse {
                    success: true,
                    preview: Some(PreviewData {
                        merchant: parsed.merchant,
                        date: parsed.date,
                        currency: parsed.currency,
                        tags: parsed.tags.unwrap_or_default(),
                        items: parsed
                            .items
                            .into_iter()
                            .map(|i| PreviewItem {
                                name: i.name,
                                quantity: i.quantity,
                                unit_price: i.unit_price,
                                amount: i.amount,
                                specs: i.specs,
                                date: i.date,
                            })
                            .collect(),
                        subtotal: parsed.subtotal,
                        tax: parsed.tax,
                        tip: parsed.tip,
                        total_amount: parsed.total_amount,
                    }),
                    message: None,
                    ai_remaining: if guest_ai_remaining < 999 {
                        Some(guest_ai_remaining)
                    } else {
                        None
                    },
                }),
            )
        }
        Err(e) => {
            eprintln!("[Expense] parse_preview error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ParsePreviewResponse {
                    success: false,
                    preview: None,
                    message: Some(e),
                    ai_remaining: None,
                }),
            )
        }
    }
}

// ===== AI message helpers =====

/// Build text context from notes and amount for AI analysis
fn build_analysis_text(notes: &str, amount: f64) -> String {
    let mut parts = Vec::new();
    if !notes.trim().is_empty() {
        parts.push(format!("用户备注: {}", notes.trim()));
    }
    if amount > 0.0 {
        parts.push(format!("金额: {:.2}", amount));
    }
    parts.join("\n")
}

/// Build user message combining text context and image info
fn build_user_message(text_context: &str, images: &[(String, String)]) -> String {
    let has_text = !text_context.is_empty();
    let has_images = !images.is_empty();

    match (has_text, has_images) {
        (true, true) => {
            let img_hint = if images.len() > 1 {
                "多张图片是同一张收据的不同部分，有重叠，请去重。"
            } else {
                ""
            };
            format!("{}\n\n请综合分析照片和文字信息，提取消费详情。{}", text_context, img_hint)
        }
        (false, true) => {
            if images.len() > 1 {
                "请解析这张收据。多张图片是同一张收据的不同部分，有重叠，请去重。".into()
            } else {
                "请解析这张收据/账单。".into()
            }
        }
        (true, false) => {
            format!("{}\n\n请根据以上信息分析这笔消费，提取商家、金额、标签等。", text_context)
        }
        (false, false) => "请分析这笔消费。".into(),
    }
}

// ===== AI helpers =====

#[allow(dead_code)]
struct ParsedReceipt {
    merchant: Option<String>,
    date: Option<String>,
    currency: Option<String>,
    tags: Option<Vec<String>>,
    items: Vec<ParsedItem>,
    subtotal: Option<f64>,
    tax: Option<f64>,
    tip: Option<f64>,
    total_amount: Option<f64>,
}

struct ParsedItem {
    name: String,
    quantity: f64,
    unit_price: Option<f64>,
    amount: f64,
    specs: String,
    date: Option<String>,
}

fn parse_ai_receipt_response(text: &str) -> ParsedReceipt {
    // Try to extract JSON from the response (it might have markdown wrapping)
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => {
            return ParsedReceipt {
                merchant: None,
                date: None,
                currency: None,
                tags: None,
                items: vec![],
                subtotal: None,
                tax: None,
                tip: None,
                total_amount: None,
            }
        }
    };

    let merchant = parsed["merchant"].as_str().map(|s| s.to_string());
    let date = parsed["date"].as_str().map(|s| s.to_string());
    let currency = parsed["currency"].as_str().map(|s| s.to_string());
    let tags = parsed["tags"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });
    let subtotal = parsed["subtotal"].as_f64();
    let tax = parsed["tax"].as_f64();
    let tip = parsed["tip"].as_f64();
    let total_amount = parsed["total_amount"].as_f64();

    let items = parsed["items"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|item| ParsedItem {
                    name: item["name"].as_str().unwrap_or("未知商品").to_string(),
                    quantity: item["quantity"].as_f64().unwrap_or(1.0),
                    unit_price: item["unit_price"].as_f64(),
                    amount: item["amount"].as_f64().unwrap_or(0.0),
                    specs: item["specs"].as_str().unwrap_or("").to_string(),
                    date: item["date"].as_str().map(|s| s.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();

    ParsedReceipt {
        merchant,
        date,
        currency,
        tags,
        items,
        subtotal,
        tax,
        tip,
        total_amount,
    }
}

/// Auto-tag from text (notes) when no photos are available
async fn auto_tag_from_text(state: &AppState, entry_id: &str, amount: f64, notes: &str) {
    let client = match crate::services::llm::LlmClient::new("auto") {
        Some(c) => c,
        None => return,
    };

    let system = "根据消费信息生成标签。只输出 JSON 数组，不要输出其他文字。";
    let msg = format!(
        "金额: ¥{:.2}\n备注: {}\n\n请输出标签 JSON 数组，如 [\"超市\", \"日用品\"]",
        amount, notes
    );

    match client.simple_generate(system, &msg, 256).await {
        Ok(text) => {
            // Parse tags from response
            let json_str = if let Some(start) = text.find('[') {
                if let Some(end) = text.rfind(']') {
                    &text[start..=end]
                } else {
                    &text
                }
            } else {
                &text
            };

            if let Ok(tags) = serde_json::from_str::<Vec<String>>(json_str) {
                if !tags.is_empty() {
                    let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into());
                    let db = state.db.lock();
                    db.execute(
                        "UPDATE expense_entries SET tags = ?1, ai_processed = 1, updated_at = ?2 WHERE id = ?3",
                        rusqlite::params![tags_json, chrono::Utc::now().to_rfc3339(), entry_id],
                    )
                    .ok();
                }
            }
        }
        Err(e) => {
            eprintln!("[Expense] auto_tag error: {}", e);
        }
    }
}

// ===== Analytics =====

fn tag_to_category(tag: &str) -> &'static str {
    match tag {
        "超市" | "杂货" | "肉类" | "蔬菜" | "水果" | "海鲜" | "零食" | "饮料" | "奶制品"
        | "调料" | "面包" | "生鲜" | "食材" => "食品杂货",
        "餐饮" | "外卖" | "餐厅" | "咖啡" | "奶茶" | "早餐" | "午餐" | "晚餐" | "火锅" | "快餐"
        | "甜点" | "酒吧" => "餐饮",
        "交通" | "加油" | "停车" | "公交" | "地铁" | "打车" | "出租车" | "高铁" | "机票"
        | "油费" | "租车" => "交通",
        "购物" | "衣服" | "鞋子" | "电子" | "数码" | "家居" | "家电" | "日用品" | "化妆品" => {
            "购物"
        }
        "住房" | "房租" | "水电" | "网费" | "物业" | "维修" | "家具" | "电话费" => {
            "住房"
        }
        "娱乐" | "电影" | "游戏" | "旅游" | "景点" | "KTV" | "运动" | "健身" => {
            "娱乐"
        }
        "医疗" | "药品" | "看病" | "体检" | "牙科" | "保健" => "医疗",
        "教育" | "书籍" | "课程" | "培训" | "文具" => "教育",
        _ => "其他",
    }
}

fn entry_category(tags_json: &str) -> &'static str {
    let tags: Vec<String> = serde_json::from_str(tags_json).unwrap_or_default();
    for tag in &tags {
        let cat = tag_to_category(tag);
        if cat != "其他" {
            return cat;
        }
    }
    "其他"
}

// ===== Exchange rate proxy =====
pub async fn get_rates(_user_id: UserId) -> (StatusCode, Json<serde_json::Value>) {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://open.er-api.com/v6/latest/CAD")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match resp {
        Ok(r) => match r.json::<serde_json::Value>().await {
            Ok(data) => {
                let cny = data["rates"]["CNY"].as_f64().unwrap_or(0.0);
                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "base": "CAD",
                        "rates": { "CNY": cny }
                    })),
                )
            }
            Err(e) => {
                eprintln!("[Expense] rates parse error: {}", e);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "success": false, "message": "汇率数据解析失败" })),
                )
            }
        },
        Err(e) => {
            eprintln!("[Expense] rates fetch error: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "success": false, "message": "汇率服务不可用" })),
            )
        }
    }
}
