//! `Source` — Insight 的单份素材(blog / x / github / youtube / pdf / 粘贴文本)。
//!
//! 抓取后 `content` 冻结存储(spec 原则 2)。
//! `insight_id` NULL = 未归属候选;非 NULL = 已挂到某 Insight。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: i64,
    #[serde(rename = "insightId", skip_serializing_if = "Option::is_none")]
    pub insight_id: Option<i64>,
    pub kind: String, // blog / x / github / youtube / pdf / text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub title: String,
    pub author: String,
    pub content: String,
    pub note: String,
    pub starred: bool,
    #[serde(rename = "fetchStatus")]
    pub fetch_status: String, // pending / ok / failed / manual
    #[serde(rename = "fetchError", skip_serializing_if = "Option::is_none")]
    pub fetch_error: Option<String>,
    #[serde(rename = "fetchedAt", skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// POST /api/sources 请求体。
/// - url 非空 → 后端推断 kind,异步抓取
/// - url 空 + content 非空 → kind=text,fetch_status=manual
#[derive(Debug, Deserialize, Default)]
pub struct CreateSourceRequest {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "insightId", default)]
    pub insight_id: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateSourceRequest {
    pub title: Option<String>,
    pub author: Option<String>,
    pub content: Option<String>,
    pub note: Option<String>,
    pub starred: Option<bool>,
    /// 拖动归属:`Some(Some(id))` 挂到 id;`Some(None)` 取消归属;`None` 不动。
    /// JSON null → Some(None);省略 → None。
    #[serde(default, deserialize_with = "deserialize_optional_optional")]
    #[serde(rename = "insightId")]
    pub insight_id: Option<Option<i64>>,
}

/// `Option<Option<T>>` 反序列化:区分"缺省"(`None`) 与 "JSON null"(`Some(None)`)
fn deserialize_optional_optional<'de, D>(d: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(Some(Option::<i64>::deserialize(d)?))
}

/// 根据 URL 推断 kind(spec § 4)
pub fn infer_kind(url: Option<&str>, _has_content: bool) -> &'static str {
    let url = match url {
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => return "text",
    };
    let lower = url.to_ascii_lowercase();
    if lower.contains("youtube.com/watch") || lower.contains("youtu.be/") {
        return "youtube";
    }
    if lower.contains("twitter.com/") && lower.contains("/status/") {
        return "x";
    }
    if lower.contains("x.com/") && lower.contains("/status/") {
        return "x";
    }
    // GitHub repo:github.com/{owner}/{repo}(不是 issues/pulls 子页)
    // 这里粗略:domain 是 github.com 且不是 gist 等子域,落 github;
    // 实际 issues / blob 等子页也归 github(用 blog readability 抓也能用)
    if lower.contains("github.com/") {
        return "github";
    }
    if lower.ends_with(".pdf") {
        return "pdf";
    }
    "blog"
}

#[allow(dead_code)]
pub const VALID_KINDS: &[&str] = &["blog", "x", "github", "youtube", "pdf", "text"];
#[allow(dead_code)]
pub const VALID_FETCH_STATUSES: &[&str] = &["pending", "ok", "failed", "manual"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_kind_youtube() {
        assert_eq!(
            infer_kind(Some("https://www.youtube.com/watch?v=abc"), false),
            "youtube"
        );
        assert_eq!(infer_kind(Some("https://youtu.be/abc"), false), "youtube");
    }

    #[test]
    fn infer_kind_x() {
        assert_eq!(
            infer_kind(Some("https://twitter.com/user/status/123"), false),
            "x"
        );
        assert_eq!(
            infer_kind(Some("https://x.com/user/status/123"), false),
            "x"
        );
    }

    #[test]
    fn infer_kind_github() {
        assert_eq!(
            infer_kind(Some("https://github.com/rust-lang/rust"), false),
            "github"
        );
    }

    #[test]
    fn infer_kind_pdf() {
        assert_eq!(
            infer_kind(Some("https://example.com/paper.pdf"), false),
            "pdf"
        );
    }

    #[test]
    fn infer_kind_blog_default() {
        assert_eq!(infer_kind(Some("https://example.com/post"), false), "blog");
    }

    #[test]
    fn infer_kind_text_no_url() {
        assert_eq!(infer_kind(None, true), "text");
        assert_eq!(infer_kind(Some("  "), true), "text");
    }
}
