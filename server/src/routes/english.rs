use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::{check_guest_ai_quota, extract_client_ip, ActiveUserId, UserId};
use crate::models::english::*;
use crate::services::llm::LlmClient;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ScenariosResponse {
    pub success: bool,
    pub items: Vec<EnglishScenario>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScenarioResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<EnglishScenario>,
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
    #[serde(default)]
    pub archived: Option<i32>,
    #[serde(default)]
    pub category: Option<String>,
}

fn row_to_scenario(row: &rusqlite::Row) -> rusqlite::Result<EnglishScenario> {
    let archived_int: i32 = row.get(8)?;
    Ok(EnglishScenario {
        id: row.get(0)?,
        title: row.get(1)?,
        title_en: row.get::<_, String>(2).unwrap_or_default(),
        description: row.get::<_, String>(3).unwrap_or_default(),
        icon: row.get::<_, String>(4).unwrap_or_else(|_| "📖".into()),
        content: row.get::<_, String>(5).unwrap_or_default(),
        status: row.get::<_, String>(6).unwrap_or_else(|_| "draft".into()),
        created_at: row.get(7)?,
        updated_at: row.get(9)?,
        archived: archived_int != 0,
        category: row.get::<_, String>(10).unwrap_or_else(|_| "英语".into()),
        notes: row.get::<_, String>(11).unwrap_or_default(),
    })
}

const SCENARIO_COLUMNS: &str =
    "id, title, title_en, description, icon, content, status, created_at, archived, updated_at, category, notes";

pub async fn list_scenarios(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<ListQuery>,
) -> (StatusCode, Json<ScenariosResponse>) {
    let db = state.db.lock();
    let archived = query.archived.unwrap_or(0);

    let (sql, items) = if let Some(ref cat) = query.category {
        let sql = format!(
            "SELECT {} FROM english_scenarios WHERE user_id = ?1 AND archived = ?2 AND category = ?3 ORDER BY updated_at DESC",
            SCENARIO_COLUMNS
        );
        let Ok(mut stmt) = db.prepare(&sql) else {
            return (
                StatusCode::OK,
                Json(ScenariosResponse {
                    success: true,
                    items: vec![],
                    message: None,
                }),
            );
        };
        let items: Vec<EnglishScenario> = stmt
            .query_map(rusqlite::params![user_id.0, archived, cat], row_to_scenario)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        (sql, items)
    } else {
        let sql = format!(
            "SELECT {} FROM english_scenarios WHERE user_id = ?1 AND archived = ?2 ORDER BY updated_at DESC",
            SCENARIO_COLUMNS
        );
        let Ok(mut stmt) = db.prepare(&sql) else {
            return (
                StatusCode::OK,
                Json(ScenariosResponse {
                    success: true,
                    items: vec![],
                    message: None,
                }),
            );
        };
        let items: Vec<EnglishScenario> = stmt
            .query_map(rusqlite::params![user_id.0, archived], row_to_scenario)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        (sql, items)
    };
    let _ = sql;

    (
        StatusCode::OK,
        Json(ScenariosResponse {
            success: true,
            items,
            message: None,
        }),
    )
}

pub async fn create_scenario(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<CreateScenarioRequest>,
) -> (StatusCode, Json<ScenarioResponse>) {
    if req.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ScenarioResponse {
                success: false,
                item: None,
                message: Some("标题不能为空".into()),
            }),
        );
    }
    if req.title.len() > 200 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ScenarioResponse {
                success: false,
                item: None,
                message: Some("标题不能超过 200 字符".into()),
            }),
        );
    }
    if req.description.as_ref().map(|d| d.len()).unwrap_or(0) > 2000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ScenarioResponse {
                success: false,
                item: None,
                message: Some("描述不能超过 2000 字符".into()),
            }),
        );
    }

    let db = state.db.lock();
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let icon = req.icon.unwrap_or_else(|| "📖".into());
    let description = req.description.unwrap_or_default();
    let category = req.category.unwrap_or_else(|| "英语".into());
    let content = req.content.unwrap_or_default();
    let status = if content.is_empty() { "draft" } else { "ready" };

    if let Err(e) = db.execute(
        "INSERT INTO english_scenarios (id, user_id, title, title_en, description, icon, content, status, archived, created_at, updated_at, category, notes) VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, '')",
        rusqlite::params![id, user_id.0, req.title, description, icon, content, status, now, now, category],
    ) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ScenarioResponse {
            success: false, item: None,
            message: Some(format!("数据库写入失败: {}", e)),
        }));
    }

    let item = EnglishScenario {
        id,
        title: req.title,
        title_en: String::new(),
        description,
        icon,
        content,
        status: status.into(),
        archived: false,
        category,
        notes: String::new(),
        created_at: now.clone(),
        updated_at: now,
    };

    (
        StatusCode::OK,
        Json(ScenarioResponse {
            success: true,
            item: Some(item),
            message: Some("场景创建成功".into()),
        }),
    )
}

pub async fn get_scenario(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<ScenarioResponse>) {
    let db = state.db.lock();

    let result = db.query_row(
        &format!(
            "SELECT {} FROM english_scenarios WHERE id = ?1 AND user_id = ?2",
            SCENARIO_COLUMNS
        ),
        rusqlite::params![id, user_id.0],
        row_to_scenario,
    );

    match result {
        Ok(item) => (
            StatusCode::OK,
            Json(ScenarioResponse {
                success: true,
                item: Some(item),
                message: None,
            }),
        ),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ScenarioResponse {
                success: false,
                item: None,
                message: Some("场景不存在".into()),
            }),
        ),
    }
}

pub async fn update_scenario(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
    Json(req): Json<UpdateScenarioRequest>,
) -> (StatusCode, Json<ScenarioResponse>) {
    let db = state.db.lock();

    let existing = db.query_row(
        &format!(
            "SELECT {} FROM english_scenarios WHERE id = ?1 AND user_id = ?2",
            SCENARIO_COLUMNS
        ),
        rusqlite::params![id, user_id.0],
        row_to_scenario,
    );

    let mut item = match existing {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ScenarioResponse {
                    success: false,
                    item: None,
                    message: Some("场景不存在".into()),
                }),
            )
        }
    };

    if let Some(title) = req.title {
        item.title = title;
    }
    if let Some(title_en) = req.title_en {
        item.title_en = title_en;
    }
    if let Some(description) = req.description {
        item.description = description;
    }
    if let Some(icon) = req.icon {
        item.icon = icon;
    }
    if let Some(content) = req.content {
        item.content = content;
    }
    if let Some(category) = req.category {
        item.category = category;
    }
    if let Some(notes) = req.notes {
        item.notes = notes;
    }
    if let Some(status) = req.status {
        item.status = status;
    }

    let now = chrono::Utc::now().to_rfc3339();
    item.updated_at = now;

    if let Err(e) = db.execute(
        "UPDATE english_scenarios SET title=?1, title_en=?2, description=?3, icon=?4, content=?5, updated_at=?6, category=?9, notes=?10, status=?11 WHERE id=?7 AND user_id=?8",
        rusqlite::params![item.title, item.title_en, item.description, item.icon, item.content, item.updated_at, id, user_id.0, item.category, item.notes, item.status],
    ) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ScenarioResponse {
            success: false, item: None,
            message: Some(format!("数据库写入失败: {}", e)),
        }));
    }

    (
        StatusCode::OK,
        Json(ScenarioResponse {
            success: true,
            item: Some(item),
            message: Some("已更新".into()),
        }),
    )
}

pub async fn delete_scenario(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    let rows = db
        .execute(
            "DELETE FROM english_scenarios WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("场景不存在".into()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已删除".into()),
        }),
    )
}

pub async fn archive_scenario(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    let rows = db
        .execute(
            "UPDATE english_scenarios SET archived = 1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
            rusqlite::params![now, id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("场景不存在".into()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已归档".into()),
        }),
    )
}

/// POST /api/english/scenarios/:id/generate — call Claude to generate content
pub async fn generate_scenario(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    headers: http::HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Guest AI quota check (T-089: IP aggregate)
    let client_ip = extract_client_ip(&headers);
    let ai_remaining = match check_guest_ai_quota(&state, &user_id.0, &client_ip) {
        Ok(remaining) => remaining,
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(ScenarioResponse {
                    success: false,
                    item: None,
                    message: Some("AI 体验次数已用完，注册解锁无限使用".into()),
                }),
            )
                .into_response();
        }
    };

    // Rate limit: 1 generation per 30 seconds per user
    {
        let mut limits = state.ai_rate_limits.lock();
        if let Some(last) = limits.get(&user_id.0) {
            if last.elapsed().as_secs() < 30 {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(ScenarioResponse {
                        success: false,
                        item: None,
                        message: Some("生成过于频繁，请 30 秒后再试".into()),
                    }),
                )
                    .into_response();
            }
        }
        limits.insert(user_id.0.clone(), std::time::Instant::now());
    }

    // Read scenario info
    let (title, description, category) = {
        let db = state.db.lock();
        let result = db.query_row(
            "SELECT title, description, COALESCE(category, '英语') FROM english_scenarios WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id.0],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1).unwrap_or_default(),
                    row.get::<_, String>(2).unwrap_or_else(|_| "英语".into()),
                ))
            },
        );
        match result {
            Ok(r) => r,
            Err(_) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ScenarioResponse {
                        success: false,
                        item: None,
                        message: Some("场景不存在".into()),
                    }),
                )
                    .into_response()
            }
        }
    };

    // Set status to generating
    {
        let db = state.db.lock();
        db.execute(
            "UPDATE english_scenarios SET status = 'generating', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![chrono::Utc::now().to_rfc3339(), id],
        )
        .ok();
    }

    // Call LLM API
    let llm = match LlmClient::for_user(&state.db.lock(), &user_id.0) {
        Some(c) => c,
        None => {
            let db = state.db.lock();
            db.execute(
                "UPDATE english_scenarios SET status = 'error', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![chrono::Utc::now().to_rfc3339(), id],
            )
            .ok();
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ScenarioResponse {
                    success: false,
                    item: None,
                    message: Some("AI 服务未配置".into()),
                }),
            )
                .into_response();
        }
    };

    let extra_info = if description.is_empty() {
        String::new()
    } else {
        format!("\n用户补充说明：{}", description)
    };

    let system_prompt = build_generation_prompt(&title, &extra_info, &category);

    let messages =
        vec![json!({"role": "user", "content": format!("请生成关于「{}」的学习内容。", title)})];

    let result = llm
        .chat(&system_prompt, messages, &[], |_, _| json!({}))
        .await;

    match result {
        Ok(chat_result) => {
            let now = chrono::Utc::now().to_rfc3339();
            let db = state.db.lock();
            db.execute(
                "UPDATE english_scenarios SET content = ?1, status = 'ready', updated_at = ?2 WHERE id = ?3",
                rusqlite::params![chat_result.text, now, id],
            )
            .ok();

            let item = db
                .query_row(
                    &format!(
                        "SELECT {} FROM english_scenarios WHERE id = ?1",
                        SCENARIO_COLUMNS
                    ),
                    [&id],
                    row_to_scenario,
                )
                .ok();

            let mut resp = json!({
                "success": true,
                "item": item,
                "message": "内容生成完成",
            });
            if ai_remaining < 999 {
                resp["ai_remaining"] = json!(ai_remaining);
            }
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(err) => {
            let db = state.db.lock();
            db.execute(
                "UPDATE english_scenarios SET status = 'error', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![chrono::Utc::now().to_rfc3339(), id],
            )
            .ok();

            (
                StatusCode::OK,
                Json(ScenarioResponse {
                    success: false,
                    item: None,
                    message: Some({
                        eprintln!("[English] generate_scenario failed: {}", err);
                        "内容生成失败，请稍后重试".to_string()
                    }),
                }),
            )
                .into_response()
        }
    }
}

fn build_generation_prompt(title: &str, extra_info: &str, category: &str) -> String {
    match category {
        "英语" => format!(
            r#"你是一个专业的英语教学助手。请为用户生成一个关于"{title}"的日常英语场景教学内容。{extra_info}

请用 Markdown 格式输出，结构如下：

## 🎬 场景介绍
用2-3句话描述这个场景，帮助学习者理解背景。

## 💬 核心对话
写一段3-5轮的真实对话，每轮包含：
- **角色标注**（如 Customer / Teller）
- **英文原文**
- **中文翻译**（紧跟英文后面，用括号）

格式示例：
**Customer:** I'd like to open a savings account, please.
（我想开一个储蓄账户。）

**Teller:** Sure! Do you have your ID with you?
（当然可以！您带身份证了吗？）

## 📝 常用词汇
列出8-12个该场景常用词汇，格式：
- **英文** /音标/ — 中文释义

## 💡 实用表达
列出5-8个实用句型或表达，格式：
- **英文句型** — 中文含义 — 使用场景说明

保持内容实用、地道、贴近真实生活。对话要自然，不要太书面化。"#
        ),
        "编程" => format!(
            r#"你是一个资深的编程导师。请为用户生成关于"{title}"的编程学习内容。{extra_info}

请用 Markdown 格式输出，结构如下：

## 📖 概念介绍
用3-5句话解释这个概念/技术是什么，为什么重要。

## 💻 代码示例
提供2-3个由浅入深的代码示例，每个包含：
- 简短说明
- 代码块（标注语言）
- 运行结果或效果说明

## 📝 核心要点
列出5-8个关键知识点，格式：
- **要点名称** — 详细解释

## 🚀 实践建议
列出3-5个动手练习建议，帮助巩固学习。

内容要准确、实用，代码示例要可运行。"#
        ),
        "职场" => format!(
            r#"你是一个职场发展顾问。请为用户生成关于"{title}"的职场学习内容。{extra_info}

请用 Markdown 格式输出，结构如下：

## 📋 情境分析
用3-5句话描述这个职场情境/话题的背景和重要性。

## 💡 技巧要点
列出5-8个关键技巧或策略，格式：
- **技巧名称** — 具体做法和注意事项

## 📌 案例参考
给出1-2个具体的场景案例，展示如何应用上述技巧。

## ⚠️ 常见误区
列出3-5个常见的错误做法，以及正确的应对方式。

内容要务实、接地气，避免空洞的理论说教。"#
        ),
        "生活" => format!(
            r#"你是一个博学的生活达人。请为用户生成关于"{title}"的生活知识内容。{extra_info}

请用 Markdown 格式输出，结构如下：

## 🔍 知识科普
用3-5句话介绍这个话题的背景知识。

## 📝 实操步骤
列出具体的操作步骤或方法，格式清晰易懂。

## ⚠️ 注意事项
列出5-8个需要注意的要点或常见问题。

## 💡 小贴士
给出3-5个实用的额外建议。

内容要实用、通俗易懂、贴近日常生活。"#
        ),
        _ => format!(
            r#"你是一个知识渊博的学习助手。请为用户生成关于"{title}"的学习内容。{extra_info}

请用 Markdown 格式输出，结构如下：

## 📖 主题介绍
用3-5句话介绍这个话题。

## 📝 核心内容
详细展开主题的关键知识点，条理清晰。

## 💡 要点总结
列出5-8个关键要点。

## 🚀 实践建议
给出3-5个具体的实践建议。

内容要准确、有深度、易于理解。"#
        ),
    }
}
