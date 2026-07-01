//! `insight_fetcher` — Blog/text 抓取适配器(T-105 MVP)。
//!
//! 设计取舍(spec § 7.1):MVP **不引入 readability 库**,用纯 Rust 状态机扫一遍 HTML:
//! 1. 跳过 `<script>` / `<style>` / `<noscript>` 内容
//! 2. 抓取 `<title>` 作为标题
//! 3. 其它标签 → 空格分隔
//! 4. 解码常见实体(`&amp;` `&lt;` `&gt;` `&quot;` `&#39;` `&nbsp;`)
//! 5. 合并连续空白
//!
//! 90% 文章 / 博客这套就够。失败时把错误写回 `sources.fetch_error`,
//! Boris 走粘贴文本兜底通道(spec 原则 7)。
//!
//! 超时:30s(spec 开放问题 #2 建议)。

use std::time::Duration;

pub struct FetchResult {
    pub title: String,
    pub content: String,
}

/// 抓 URL → 抽 (title, content)。失败返回错误字符串(写入 fetch_error)。
pub async fn fetch_blog(url: &str) -> Result<FetchResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; ergouWeb-insight-fetcher/1.0)")
        .build()
        .map_err(|e| format!("client build: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("fetch: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }

    // 30s 超时 + 限制 5MB 上限(防巨型页面 OOM)
    let bytes = resp.bytes().await.map_err(|e| format!("read body: {e}"))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(format!("payload too large: {} bytes", bytes.len()));
    }

    let html = String::from_utf8_lossy(&bytes).into_owned();
    let title = extract_title(&html);
    let content = extract_text(&html);
    if content.trim().is_empty() {
        return Err("content empty after extraction".into());
    }
    // 截断超长正文(防一次塞 1MB+ 进 DB);保留前 200K 字符。
    let content = if content.chars().count() > 200_000 {
        content.chars().take(200_000).collect::<String>() + "\n\n…(已截断,源文档过长)"
    } else {
        content
    };
    Ok(FetchResult { title, content })
}

/// 抽取 `<title>` 内文本
pub fn extract_title(html: &str) -> String {
    extract_tag_text(html, "title").unwrap_or_default()
}

/// 抽取页面正文(剔除 script/style/noscript,标签转空格)
pub fn extract_text(html: &str) -> String {
    // 优先在 <article> 或 <main> 范围内抽,失败则用整页
    let scope: &str = extract_tag_slice(html, "article")
        .or_else(|| extract_tag_slice(html, "main"))
        .unwrap_or(html);
    strip_html(scope)
}

/// 在 `html` 中找到首个 `<tag>...</tag>` 的内层字符串(用于 title)
fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    let s = extract_tag_slice(html, tag)?;
    Some(
        decode_entities(&collapse_ws(&strip_tags_naive(s)))
            .trim()
            .to_string(),
    )
}

/// 找到首个 `<tag ...>` 后到 `</tag>` 之间的子串(返回原 HTML 片段)
fn extract_tag_slice<'a>(html: &'a str, tag: &str) -> Option<&'a str> {
    let lower = html.to_ascii_lowercase();
    let open_marker = format!("<{}", tag);
    let close_marker = format!("</{}", tag);
    let start = lower.find(&open_marker)?;
    // 找到 '>' 结束开标签
    let after_open = html[start..].find('>')? + start + 1;
    let end = lower[after_open..].find(&close_marker)? + after_open;
    Some(&html[after_open..end])
}

/// 简单标签剥离:不区分 script/style — 给 title 用够了
fn strip_tags_naive(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// 主力剥离器:跳过 script/style/noscript 块整体
fn strip_html(s: &str) -> String {
    let bytes = s.as_bytes();
    let lower = s.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let mut out = String::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'<' {
            // 看是否进入 script/style/noscript
            if let Some(skip_to) = find_block_close(lower_bytes, i, "script")
                .or_else(|| find_block_close(lower_bytes, i, "style"))
                .or_else(|| find_block_close(lower_bytes, i, "noscript"))
            {
                i = skip_to;
                continue;
            }
            // 普通标签:跳到 '>'
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // 越过 '>'
            }
            out.push(' ');
        } else {
            // 普通字符 — 处理多字节 UTF-8(直接 push 字节,字符串构造时 lossy 已确保有效)
            // 找到下一个字符边界
            let len = char_len_at(&bytes[i..]);
            if let Ok(c) = std::str::from_utf8(&bytes[i..i + len]) {
                out.push_str(c);
            }
            i += len;
        }
    }
    let decoded = decode_entities(&out);
    collapse_ws(&decoded).trim().to_string()
}

fn char_len_at(s: &[u8]) -> usize {
    if s.is_empty() {
        return 0;
    }
    let b = s[0];
    if b < 0xC0 {
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// 如果 `lower_bytes[i..]` 以 `<{tag}` 开头,找到 `</{tag}>` 的结束位置并返回 i_end + 1
fn find_block_close(lower_bytes: &[u8], i: usize, tag: &str) -> Option<usize> {
    let prefix = format!("<{}", tag);
    if !lower_bytes[i..].starts_with(prefix.as_bytes()) {
        return None;
    }
    // 确认下一个字符是 ' ' 或 '>' 或 '/' (排除 <scriptfoo)
    let next_idx = i + prefix.len();
    if next_idx >= lower_bytes.len() {
        return None;
    }
    let nb = lower_bytes[next_idx];
    if nb != b' ' && nb != b'>' && nb != b'/' && nb != b'\t' && nb != b'\n' && nb != b'\r' {
        return None;
    }
    // 找 </tag>
    let close_pat = format!("</{}>", tag);
    let lower_str = std::str::from_utf8(lower_bytes).ok()?;
    let close_idx = lower_str[i..].find(&close_pat)? + i + close_pat.len();
    Some(close_idx)
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&ndash;", "–")
        .replace("&mdash;", "—")
        .replace("&hellip;", "…")
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_title_basic() {
        let html = r#"<html><head><title>Hello World</title></head><body>x</body></html>"#;
        assert_eq!(extract_title(html), "Hello World");
    }

    #[test]
    fn extract_title_with_entity() {
        let html = r#"<title>Foo &amp; Bar</title>"#;
        assert_eq!(extract_title(html), "Foo & Bar");
    }

    #[test]
    fn extract_text_strips_script_style() {
        let html = r#"<html><body><script>alert('x')</script><p>Hello</p><style>p{}</style><p>World</p></body></html>"#;
        assert_eq!(extract_text(html), "Hello World");
    }

    #[test]
    fn extract_text_prefers_article() {
        let html = r#"<html><body><nav>Nav</nav><article>Body content</article><footer>Foot</footer></body></html>"#;
        assert_eq!(extract_text(html), "Body content");
    }

    #[test]
    fn extract_text_falls_back_to_main() {
        let html = r#"<html><body><nav>Nav</nav><main>Main content</main></body></html>"#;
        assert_eq!(extract_text(html), "Main content");
    }

    #[test]
    fn extract_text_falls_back_to_body() {
        let html = r#"<html><body><p>Plain body</p></body></html>"#;
        let t = extract_text(html);
        assert!(t.contains("Plain body"));
    }

    #[test]
    fn decode_entities_common() {
        assert_eq!(decode_entities("a&amp;b"), "a&b");
        assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_entities("don&#39;t"), "don't");
    }

    #[test]
    fn collapse_ws_strips_runs() {
        assert_eq!(collapse_ws("a  \n\t b"), "a b");
    }
}
