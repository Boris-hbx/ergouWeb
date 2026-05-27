//! `Insight` — 一个研究主题(由多份 source 整合而成一份可分享报告)。
//!
//! See SPEC: `C:\Project\ergouPM\specs\insight\spec.md`
//! Task ticket: T-105
//!
//! Field naming: SQL `created_at` ↔ API JSON `createdAt`,与 work_task 通约。

use serde::{Deserialize, Serialize};

/// 完整的 Insight 视图(API 返回)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub id: i64,
    pub title: String,
    pub topic: String,
    pub template: String, // survey / decision / watch
    /// v0.2 6 态:collecting / ready / processing / editing / published / archived
    pub status: String,
    #[serde(rename = "currentReportId", skip_serializing_if = "Option::is_none")]
    pub current_report_id: Option<i64>,
    /// v0.2 新增:Boris 触发"让 CC 改一版"时暂存的修订指示;CC 写回 report 后清空
    #[serde(rename = "pendingRevisionNote", default)]
    pub pending_revision_note: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// POST /api/insights 请求体。
#[derive(Debug, Deserialize, Default)]
pub struct CreateInsightRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default = "default_template")]
    pub template: String,
}

/// PATCH /api/insights/:id 请求体。
#[derive(Debug, Deserialize, Default)]
pub struct UpdateInsightRequest {
    pub title: Option<String>,
    pub topic: Option<String>,
    pub template: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "currentReportId")]
    pub current_report_id: Option<i64>,
}

fn default_template() -> String {
    "survey".to_string()
}

/// 合法的 status 转换(防 LLM / 前端乱传)。v0.2 6 态。
/// v0.1 的 `drafting` 已迁移到 `collecting`(无报告) / `editing`(有报告);
/// 不在白名单内,db.rs run_migrations 一次性 UPDATE。
pub const VALID_STATUSES: &[&str] = &[
    "collecting",
    "ready",
    "processing",
    "editing",
    "published",
    "archived",
];

pub fn is_valid_status(s: &str) -> bool {
    VALID_STATUSES.contains(&s)
}

pub const VALID_TEMPLATES: &[&str] = &["survey", "decision", "watch"];

pub fn is_valid_template(s: &str) -> bool {
    VALID_TEMPLATES.contains(&s)
}
