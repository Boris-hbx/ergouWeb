//! Work task table — independent task domain for organizational/recurring work.
//!
//! See SPEC: `C:\Project\ergouPM\specs\work-task-table\spec.md`
//! HTML preview (source of truth for UI): `frontend/work-board-preview.html`
//!
//! Field naming: SQL columns use snake_case (`due_date`, `created_at`),
//! API JSON exposes them as `due`, `createdAt` to match HTML preview's `t.due`.
//!
//! Custom column values are stored in the `custom_fields` JSON column
//! (object with column key → string value).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkTask {
    pub id: i64,
    pub title: String,
    /// Long text "简介" — shown as preview + expand button in table view.
    #[serde(default)]
    pub desc: String,
    /// Free-text label (NOT linked to a real user account).
    #[serde(default)]
    pub assignee: String,
    /// 院 / 所 / 组 / 个人 — value matches one of WT_LEVELS options.
    #[serde(default)]
    pub level: String,
    /// 一次性 / 每周 / 每月 ... — pure label, does NOT trigger recurrence
    /// (recurrence belongs to the 例行 module, not this one).
    #[serde(default)]
    pub freq: String,
    /// `todo` / `doing` / `blocked` / `done` — drives the board view columns.
    #[serde(default = "default_status")]
    pub status: String,
    /// `high` / `mid` / `low` — drives the priority pill color.
    #[serde(default = "default_priority")]
    pub priority: String,
    /// Date as `YYYY-MM-DD` or `MM-DD`. Exposed as `due` in JSON to match HTML preview.
    #[serde(rename = "due", default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// 0..100. Auto-set to 100 when status flips to `done`.
    #[serde(default)]
    pub progress: i32,
    /// T-110:内置 multi「标签」列(T-108 加入 work_columns 内置种子)对应的独立 schema 字段。
    /// 存储为 JSON 数组,API 反序列化为 Vec<String>。
    /// (用户自定义的 multi 列仍走 custom_fields;只有内置「标签」走此独立字段。)
    #[serde(default)]
    pub tags: Vec<String>,
    /// T-119:协作者(Linear 风格"主+协")。assignee 是主责任人(单值);
    /// collaborators 是协作者数组,纯文本标识,不关联账号。
    #[serde(default)]
    pub collaborators: Vec<String>,
    /// Custom column values: { column_key: string_value }.
    /// For multi-select custom cols, the value is a JSON array of strings.
    #[serde(rename = "customFields", default)]
    pub custom_fields: Map<String, JsonValue>,
    #[serde(default)]
    pub sort_order: f64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
}

/// POST /api/work/tasks request body.
/// All fields optional except sensible defaults; title can be empty (matches HTML
/// preview which shows `(无标题)` placeholder for blank titles).
#[derive(Debug, Deserialize, Default)]
pub struct CreateWorkTaskRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub assignee: String,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub freq: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(rename = "due", default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub progress: i32,
    /// T-110:内置「标签」列(builtin=true, multi 类型)对应的字段。
    #[serde(default)]
    pub tags: Vec<String>,
    /// T-119:协作者数组(Linear 风格;assignee 是主责任人,collaborators 是参与方)
    #[serde(default)]
    pub collaborators: Vec<String>,
    #[serde(rename = "customFields", default)]
    pub custom_fields: Option<Map<String, JsonValue>>,
}

/// PATCH /api/work/tasks/{id} request body.
/// Only fields present in the JSON are updated. To clear `due_date`, send empty string.
#[derive(Debug, Deserialize, Default)]
pub struct UpdateWorkTaskRequest {
    pub title: Option<String>,
    pub desc: Option<String>,
    pub assignee: Option<String>,
    pub level: Option<String>,
    pub freq: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    #[serde(rename = "due")]
    pub due_date: Option<String>,
    pub progress: Option<i32>,
    /// T-110:内置「标签」列(builtin=true, multi 类型) — 整体替换,不 merge(语义同 status/level 等单选)。
    pub tags: Option<Vec<String>>,
    /// T-119:协作者数组(整体替换,不 merge)
    pub collaborators: Option<Vec<String>>,
    #[serde(rename = "customFields")]
    pub custom_fields: Option<Map<String, JsonValue>>,
    pub sort_order: Option<f64>,
}

fn default_status() -> String {
    "todo".to_string()
}
fn default_priority() -> String {
    "mid".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_req_defaults() {
        let req: CreateWorkTaskRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.status, "todo");
        assert_eq!(req.priority, "mid");
        assert_eq!(req.progress, 0);
        assert!(req.due_date.is_none());
    }

    #[test]
    fn task_due_serializes_as_due_not_due_date() {
        let t = WorkTask {
            id: 1,
            title: "x".into(),
            desc: String::new(),
            assignee: String::new(),
            level: String::new(),
            freq: String::new(),
            status: "todo".into(),
            priority: "mid".into(),
            due_date: Some("05-25".into()),
            progress: 0,
            tags: Vec::new(),
            collaborators: Vec::new(),
            custom_fields: Map::new(),
            sort_order: 0.0,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let j = serde_json::to_string(&t).unwrap();
        assert!(j.contains("\"due\":\"05-25\""));
        assert!(!j.contains("due_date"));
    }

    #[test]
    fn update_req_partial() {
        let req: UpdateWorkTaskRequest =
            serde_json::from_str(r#"{"status":"done","progress":100}"#).unwrap();
        assert_eq!(req.status.as_deref(), Some("done"));
        assert_eq!(req.progress, Some(100));
        assert!(req.title.is_none());
    }
}
