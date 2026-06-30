use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    #[default]
    Today,
    Week,
    Month,
}

impl Tab {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Today => "today",
            Tab::Week => "week",
            Tab::Month => "month",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "today" => Tab::Today,
            "week" => Tab::Week,
            "month" => Tab::Month,
            _ => Tab::Today,
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Quadrant {
    #[serde(rename = "important-urgent")]
    ImportantUrgent,
    #[serde(rename = "important-not-urgent")]
    ImportantNotUrgent,
    #[serde(rename = "not-important-urgent")]
    NotImportantUrgent,
    #[serde(rename = "not-important-not-urgent")]
    #[default]
    NotImportantNotUrgent,
}

impl Quadrant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Quadrant::ImportantUrgent => "important-urgent",
            Quadrant::ImportantNotUrgent => "important-not-urgent",
            Quadrant::NotImportantUrgent => "not-important-urgent",
            Quadrant::NotImportantNotUrgent => "not-important-not-urgent",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "important-urgent" => Quadrant::ImportantUrgent,
            "important-not-urgent" => Quadrant::ImportantNotUrgent,
            "not-important-urgent" => Quadrant::NotImportantUrgent,
            "not-important-not-urgent" => Quadrant::NotImportantNotUrgent,
            _ => Quadrant::NotImportantNotUrgent,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Quadrant::ImportantUrgent => "优先处理",
            Quadrant::ImportantNotUrgent => "就等你翻牌子了",
            Quadrant::NotImportantUrgent => "待分类",
            Quadrant::NotImportantNotUrgent => "短平快",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub field: String,
    pub label: String,
    #[serde(alias = "from", alias = "from_val")]
    pub old_value: String,
    #[serde(alias = "to", alias = "to_val")]
    pub new_value: String,
    #[serde(alias = "time")]
    pub timestamp: String,
}

impl ChangeEntry {
    #[allow(dead_code)]
    pub fn new(field: &str, label: &str, old_value: &str, new_value: &str) -> Self {
        Self {
            field: field.to_string(),
            label: label.to_string(),
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tab: Tab,
    #[serde(default)]
    pub quadrant: Quadrant,
    #[serde(default)]
    pub progress: u8,
    #[serde(default)]
    pub completed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(default)]
    pub assignee: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub changelog: Vec<ChangeEntry>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
    #[serde(rename = "upgradedToWork", default)]
    pub upgraded_to_work: bool,
    #[serde(
        rename = "workTaskId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub work_task_id: Option<String>,
    #[serde(
        rename = "upgradedAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub upgraded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_reminder: Option<TodoReminder>,
}

/// Minimal reminder info attached to a todo card
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoReminder {
    pub id: String,
    pub remind_at: String,
    pub status: String,
}

impl Todo {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    pub fn field_label(field: &str) -> &str {
        match field {
            "tab" => "时间维度",
            "quadrant" => "象限",
            "progress" => "进度",
            "completed" => "完成状态",
            "assignee" => "负责人",
            "due_date" => "截止日期",
            "tags" => "标签",
            "text" => "标题",
            "content" => "内容",
            _ => field,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoUpdate {
    pub text: Option<String>,
    pub content: Option<String>,
    pub tab: Option<String>,
    pub quadrant: Option<String>,
    pub progress: Option<u8>,
    pub completed: Option<bool>,
    pub due_date: Option<String>,
    pub assignee: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTodoRequest {
    pub text: String,
    #[serde(default = "default_tab")]
    pub tab: String,
    #[serde(default = "default_quadrant")]
    pub quadrant: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

fn default_tab() -> String {
    "today".to_string()
}

fn default_quadrant() -> String {
    "not-important-not-urgent".to_string()
}

#[derive(Debug, Deserialize)]
pub struct BatchUpdateItem {
    pub id: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub quadrant: Option<String>,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub completed: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_from_str_known() {
        assert_eq!(Tab::parse("today"), Tab::Today);
        assert_eq!(Tab::parse("week"), Tab::Week);
        assert_eq!(Tab::parse("month"), Tab::Month);
    }

    #[test]
    fn test_tab_from_str_unknown_defaults_today() {
        assert_eq!(Tab::parse("unknown"), Tab::Today);
        assert_eq!(Tab::parse(""), Tab::Today);
    }

    #[test]
    fn test_quadrant_from_str_known() {
        assert_eq!(
            Quadrant::parse("important-urgent"),
            Quadrant::ImportantUrgent
        );
        assert_eq!(
            Quadrant::parse("important-not-urgent"),
            Quadrant::ImportantNotUrgent
        );
        assert_eq!(
            Quadrant::parse("not-important-urgent"),
            Quadrant::NotImportantUrgent
        );
        assert_eq!(
            Quadrant::parse("not-important-not-urgent"),
            Quadrant::NotImportantNotUrgent
        );
    }

    #[test]
    fn test_quadrant_from_str_unknown_defaults() {
        assert_eq!(Quadrant::parse("invalid"), Quadrant::NotImportantNotUrgent);
        assert_eq!(Quadrant::parse(""), Quadrant::NotImportantNotUrgent);
    }
}
