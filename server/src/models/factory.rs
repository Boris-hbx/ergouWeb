//! Insight Factory P0 data contracts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryTask {
    pub id: i64,
    pub title: String,
    #[serde(rename = "inputType")]
    pub input_type: String,
    #[serde(rename = "inputContent")]
    pub input_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub status: String,
    #[serde(rename = "currentReportId", skip_serializing_if = "Option::is_none")]
    pub current_report_id: Option<i64>,
    #[serde(rename = "sourceSnapshot", default)]
    pub source_snapshot: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryJob {
    pub id: i64,
    #[serde(rename = "taskId")]
    pub task_id: i64,
    pub mode: String,
    pub status: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(rename = "feedbackNote")]
    pub feedback_note: String,
    #[serde(rename = "parentReportId", skip_serializing_if = "Option::is_none")]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "retryOfJobId", skip_serializing_if = "Option::is_none")]
    pub retry_of_job_id: Option<i64>,
    #[serde(rename = "errorMessage")]
    pub error_message: String,
    #[serde(rename = "startedAt", skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(rename = "finishedAt", skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryReport {
    pub id: i64,
    #[serde(rename = "taskId")]
    pub task_id: i64,
    #[serde(rename = "jobId")]
    pub job_id: i64,
    pub version: i64,
    pub template: String,
    #[serde(rename = "contentMd")]
    pub content_md: String,
    #[serde(rename = "parentReportId", skip_serializing_if = "Option::is_none")]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "revisionNote")]
    pub revision_note: String,
    #[serde(rename = "generatedBy")]
    pub generated_by: String,
    pub provider: String,
    #[serde(rename = "modelUsed")]
    pub model_used: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryMemory {
    pub id: i64,
    #[serde(rename = "type")]
    pub memory_type: String,
    pub title: String,
    pub body: String,
    pub source: String,
    #[serde(rename = "sourceRef")]
    pub source_ref: String,
    pub importance: i64,
    pub enabled: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateFactoryTaskRequest {
    #[serde(default)]
    pub title: String,
    #[serde(rename = "inputType", default)]
    pub input_type: Option<String>,
    #[serde(rename = "inputContent", default)]
    pub input_content: String,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(rename = "sourceSnapshot", default)]
    pub source_snapshot: String,
    #[serde(rename = "createGenerateJob", default)]
    pub create_generate_job: bool,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateFactoryTaskRequest {
    pub title: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateJobRequest {
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FeedbackRequest {
    #[serde(rename = "feedbackNote", default)]
    pub feedback_note: String,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFactoryReportRequest {
    #[serde(rename = "jobId")]
    pub job_id: i64,
    pub template: String,
    #[serde(rename = "contentMd")]
    pub content_md: String,
    #[serde(rename = "parentReportId", default)]
    pub parent_report_id: Option<i64>,
    #[serde(rename = "revisionNote", default)]
    pub revision_note: String,
    #[serde(rename = "generatedBy", default)]
    pub generated_by: String,
    #[serde(default)]
    pub provider: String,
    #[serde(rename = "modelUsed", default)]
    pub model_used: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateMemoryRequest {
    #[serde(rename = "type", default)]
    pub memory_type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub source: String,
    #[serde(rename = "sourceRef", default)]
    pub source_ref: String,
    #[serde(default)]
    pub importance: Option<i64>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateMemoryRequest {
    #[serde(rename = "type")]
    pub memory_type: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "sourceRef")]
    pub source_ref: Option<String>,
    pub importance: Option<i64>,
    pub enabled: Option<bool>,
}

pub const VALID_INPUT_TYPES: &[&str] = &["url", "topic", "prompt", "note"];
pub const VALID_TEMPLATES: &[&str] = &["survey", "decision", "watch"];
pub const VALID_MEMORY_TYPES: &[&str] = &[
    "project_fact",
    "boris_profile",
    "insight_summary",
    "report_preference",
];

pub fn is_valid_input_type(s: &str) -> bool {
    VALID_INPUT_TYPES.contains(&s)
}

pub fn is_valid_template(s: &str) -> bool {
    VALID_TEMPLATES.contains(&s)
}

pub fn is_valid_memory_type(s: &str) -> bool {
    VALID_MEMORY_TYPES.contains(&s)
}

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

pub fn detect_input_type(text: &str) -> &'static str {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    if (lower.contains("http://") || lower.contains("https://"))
        && trimmed.split_whitespace().count() == 1
    {
        return "url";
    }
    if trimmed.chars().count() <= 80 {
        return "topic";
    }
    if IMPERATIVE_MARKERS.iter().any(|m| trimmed.contains(m)) {
        "prompt"
    } else {
        "note"
    }
}

pub fn derive_title(input_content: &str) -> String {
    let first_line = input_content.trim().lines().next().unwrap_or("").trim();
    let s: String = first_line.chars().take(40).collect();
    if first_line.chars().count() > 40 {
        format!("{s}…")
    } else {
        s
    }
}
