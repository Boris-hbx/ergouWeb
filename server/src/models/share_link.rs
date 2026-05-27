//! `ShareLink` — 一个 Report 的公开分享 token(spec § 4 / § 5.5)。
//!
//! - token: 32 字节 URL-safe 随机
//! - 撤销 → revoked_at 非 NULL → `GET /r/{token}` 返回 **410 Gone**(明确"已撤销",非 404)
//! - 绑定到具体 report.id,分享内容不变(spec 原则 3)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLink {
    pub token: String,
    #[serde(rename = "insightId")]
    pub insight_id: i64,
    #[serde(rename = "reportId")]
    pub report_id: i64,
    #[serde(rename = "showNotes")]
    pub show_notes: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "revokedAt", skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

/// POST /api/insights/:id/share 请求体。
#[derive(Debug, Deserialize, Default)]
pub struct CreateShareRequest {
    /// 绑定到的 report id;不传则用 insight.current_report_id
    #[serde(rename = "reportId")]
    pub report_id: Option<i64>,
    /// 是否在分享页显示 source.note(私货备注)
    #[serde(rename = "showNotes", default)]
    pub show_notes: bool,
}
