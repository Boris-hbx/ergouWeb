//! `Annotation` — 锚定到具体 report 版本的备注(T-107 / SPEC insight v0.2 § 4)。
//!
//! Boris 在 editing 状态下读报告 → 点段落 → 加备注。
//! Claude Code 修订模式下读 open annotations 作为修订输入。
//!
//! 锚点格式(MVP 仅 2 种):
//! - 段落: `{"kind":"paragraph","index":N}` — 第 N 段(从 0 起按 markdown 顶层 block 数)
//! - 标题: `{"kind":"heading","slug":"群-a-性能优先派"}` — slug 由前端生成
//! - 跨段 range:Phase 2,MVP 不支持

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: i64,
    #[serde(rename = "insightId")]
    pub insight_id: i64,
    #[serde(rename = "reportId")]
    pub report_id: i64,
    /// JSON 字符串(由前端构造),保留原样存储
    pub anchor: String,
    pub body: String,
    pub kind: String,   // question / suggestion / factcheck / other
    pub status: String, // open / resolved
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnotationRequest {
    #[serde(rename = "reportId")]
    pub report_id: i64,
    pub anchor: String,
    pub body: String,
    #[serde(default = "default_kind")]
    pub kind: String,
}

/// PATCH /api/annotations/:id —— 只能改 body / kind / status;
/// anchor 与 report_id 不可改(spec § 8.5 注),要改锚就删了重建。
#[derive(Debug, Deserialize, Default)]
pub struct UpdateAnnotationRequest {
    pub body: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
}

fn default_kind() -> String {
    "other".to_string()
}

pub const VALID_KINDS: &[&str] = &["question", "suggestion", "factcheck", "other"];
pub const VALID_STATUSES: &[&str] = &["open", "resolved"];

pub fn is_valid_kind(s: &str) -> bool {
    VALID_KINDS.contains(&s)
}
pub fn is_valid_status(s: &str) -> bool {
    VALID_STATUSES.contains(&s)
}
