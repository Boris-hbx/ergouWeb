//! `Report` — Insight 的一个版本化报告快照(spec 原则 3)。
//!
//! Claude Code 写入 → 出新 version;
//! Boris 手改 `content_md` → 保留当前 version,只更新 updated_at(不开新版本)。

use serde::{Deserialize, Serialize};

/// v0.2.1 citation 引用条目(spec § 四 reports.citations 语义)
/// JSON 形如 `{"ref":"1","sourceId":12,"quote":"30-100 字原文片段..."}`。
/// `ref` 用 raw identifier 因为是 Rust 保留字;序列化 / 反序列化都保持 "ref" 名字。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    #[serde(rename = "ref")]
    pub r#ref: String,
    #[serde(rename = "sourceId")]
    pub source_id: i64,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub id: i64,
    #[serde(rename = "insightId")]
    pub insight_id: i64,
    pub version: i64,
    pub template: String,
    #[serde(rename = "contentMd")]
    pub content_md: String,
    /// 引用的 source.id 数组(JSON 反序列化后)
    #[serde(rename = "sourceIds")]
    pub source_ids: Vec<i64>,
    /// v0.2.1 引用条目数组(JSON 反序列化后);CC 生成报告时同步输出,
    /// 让前端在 `[^N]` 上挂可交互 popover。旧 report 或 CC 未输出时为空数组。
    #[serde(default)]
    pub citations: Vec<Citation>,
    /// v0.2 新增:生成此版本时 Boris 给 CC 的修订指示(v1 / 首次生成为空)
    #[serde(rename = "revisionNote", default)]
    pub revision_note: String,
    /// v0.2 新增:上一版 id(首次生成为 None);CC 修订模式的依据
    #[serde(rename = "parentReportId", skip_serializing_if = "Option::is_none")]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "generatedBy")]
    pub generated_by: String, // "claude-code" / "boris-inline" / "web-api"
    #[serde(rename = "modelUsed")]
    pub model_used: String,
    /// v0.2 新增:此版本发布时间(NULL = 未发过)
    #[serde(rename = "publishedAt", skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// v0.2 新增:此版本撤回时间(NULL = 未撤回)
    #[serde(rename = "retractedAt", skip_serializing_if = "Option::is_none")]
    pub retracted_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// POST /api/insights/:id/reports 请求体(Claude Code 提交完整报告)。
#[derive(Debug, Deserialize)]
pub struct CreateReportRequest {
    pub template: String,
    #[serde(rename = "contentMd")]
    pub content_md: String,
    #[serde(rename = "sourceIds", default)]
    pub source_ids: Vec<i64>,
    /// v0.2.1 CC 输出的引用条目数组;首版可以是空(没引用就不输出)
    #[serde(default)]
    pub citations: Vec<Citation>,
    /// v0.2 新增:CC 提交时把 Boris 的 pending_revision_note 拷过来记入此版本
    #[serde(rename = "revisionNote", default)]
    pub revision_note: String,
    /// v0.2 新增:修订模式下指向上一版;首次生成为 None
    #[serde(rename = "parentReportId", default)]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "generatedBy", default)]
    pub generated_by: String,
    #[serde(rename = "modelUsed", default)]
    pub model_used: String,
}

/// PATCH /api/insights/:id/reports/:version 请求体(Boris 手改 MD)。
#[derive(Debug, Deserialize, Default)]
pub struct UpdateReportRequest {
    #[serde(rename = "contentMd")]
    pub content_md: Option<String>,
}
