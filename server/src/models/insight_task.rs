//! `InsightTask` / `InsightReport` — 洞察 v0.3 单层数据模型(T-122)。
//!
//! See SPEC: `C:\Project\ergouPM\specs\insight\spec.md` v0.3
//!
//! v0.3 大重构:废弃 v0.2 的 source + insight 双层模型 → 统一 `insight_task` 单表;
//! 废弃 annotation anchor → 单 `feedback_note` 文本框;3 档状态(ready/processing/done)。
//!
//! 字段命名:SQL `snake_case` ↔ API JSON `camelCase`,与 work_task / insight(v0.2)通约。

use serde::{Deserialize, Serialize};

/// 完整的 InsightTask 视图(API 返回)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightTask {
    pub id: i64,
    pub title: String,
    /// url | topic | prompt | note
    #[serde(rename = "inputType")]
    pub input_type: String,
    /// 原始输入(URL / 主题 / prompt / 随想文本)
    #[serde(rename = "inputContent")]
    pub input_content: String,
    /// NULL = 让 LLM 选;写入后固定(survey / decision / watch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// ready | processing | done
    pub status: String,
    #[serde(rename = "currentReportId", skip_serializing_if = "Option::is_none")]
    pub current_report_id: Option<i64>,
    /// Boris 最近写的反馈;非空 = 待修订
    #[serde(rename = "feedbackNote")]
    pub feedback_note: String,
    /// input_type=url 时抓回的内容快照(列表接口不返回,详情才带)
    #[serde(
        rename = "sourceSnapshot",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub source_snapshot: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// 一个版本化报告快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightReport {
    pub id: i64,
    #[serde(rename = "taskId")]
    pub task_id: i64,
    pub version: i64,
    pub template: String,
    #[serde(rename = "contentMd")]
    pub content_md: String,
    #[serde(rename = "parentReportId", skip_serializing_if = "Option::is_none")]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "revisionNote", default)]
    pub revision_note: String,
    #[serde(rename = "generatedBy")]
    pub generated_by: String,
    #[serde(rename = "modelUsed")]
    pub model_used: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// POST /api/insight-tasks 请求体。
#[derive(Debug, Deserialize, Default)]
pub struct CreateTaskRequest {
    #[serde(default)]
    pub title: String,
    /// 前端识别后的类型;留空则后端自动识别(spec § 五,双重保险)
    #[serde(rename = "inputType", default)]
    pub input_type: Option<String>,
    #[serde(rename = "inputContent", default)]
    pub input_content: String,
    /// 可选;NULL = 让 LLM 选
    #[serde(default)]
    pub template: Option<String>,
}

/// PATCH /api/insight-tasks/:id 请求体。
#[derive(Debug, Deserialize, Default)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub template: Option<String>,
    /// 写入非空 → 若当前 done 自动回 ready(spec § 七)
    #[serde(rename = "feedbackNote")]
    pub feedback_note: Option<String>,
}

/// POST /api/insight-tasks/:id/reports 请求体(Claude Code 提交报告)。
#[derive(Debug, Deserialize)]
pub struct CreateReportRequest {
    pub template: String,
    #[serde(rename = "contentMd")]
    pub content_md: String,
    #[serde(rename = "parentReportId", default)]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "revisionNote", default)]
    pub revision_note: String,
    #[serde(rename = "generatedBy", default)]
    pub generated_by: String,
    #[serde(rename = "modelUsed", default)]
    pub model_used: String,
}

// ============ 校验 ============

pub const VALID_INPUT_TYPES: &[&str] = &["url", "topic", "prompt", "note"];

pub fn is_valid_input_type(s: &str) -> bool {
    VALID_INPUT_TYPES.contains(&s)
}

pub const VALID_TEMPLATES: &[&str] = &["survey", "decision", "watch"];

pub fn is_valid_template(s: &str) -> bool {
    VALID_TEMPLATES.contains(&s)
}

pub const VALID_STATUSES: &[&str] = &["ready", "processing", "done"];

pub fn is_valid_status(s: &str) -> bool {
    VALID_STATUSES.contains(&s)
}

/// 指令性动词(spec § 五:prompt vs note 判定)。
const IMPERATIVE_MARKERS: &[&str] = &[
    "帮我",
    "请",
    "分析",
    "总结",
    "对比",
    "写一份",
    "写一篇",
    "整理",
    "给我",
    "梳理",
    "评估",
];

/// 后端自动识别 input_type(spec § 五,与前端规则一致,作为双重保险兜底)。
///
/// 规则优先级:
/// 1. 含 `http://` / `https://` 且能解析出 host → url
/// 2. ≤ 80 字符 → topic
/// 3. > 80 字符且含指令性动词 → prompt
/// 4. > 80 字符且不含指令性动词(陈述性)→ note
pub fn detect_input_type(text: &str) -> &'static str {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    if (lower.contains("http://") || lower.contains("https://"))
        && trimmed.split_whitespace().count() == 1
    {
        return "url";
    }
    let char_count = trimmed.chars().count();
    if char_count <= 80 {
        return "topic";
    }
    if IMPERATIVE_MARKERS.iter().any(|m| trimmed.contains(m)) {
        "prompt"
    } else {
        "note"
    }
}

/// 从原始输入派生标题(用户没填 title 时):取首行,截断到 40 字符。
pub fn derive_title(input_content: &str) -> String {
    let first_line = input_content.trim().lines().next().unwrap_or("").trim();
    let s: String = first_line.chars().take(40).collect();
    if first_line.chars().count() > 40 {
        format!("{s}…")
    } else {
        s
    }
}
