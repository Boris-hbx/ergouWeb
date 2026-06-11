//! Insight Factory worker dispatcher and Codex provider (T-209).
//!
//! The worker owns the async transition from `factory_jobs.pending` to a
//! persisted `factory_reports` row. It deliberately has no OpenAI API fallback:
//! if an API billing path is detected, the job is blocked before generation.

use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct InsightJobContext {
    pub task_id: i64,
    pub job_id: i64,
    pub user_id: String,
    pub mode: String,
    pub input_type: String,
    pub input_content: String,
    pub source_snapshot: String,
    pub template: String,
    pub memory_context_md: String,
    pub previous_report_md: String,
    pub feedback_note: String,
    pub parent_report_id: Option<i64>,
    pub retry_of_job_id: Option<i64>,
    pub report_contract_md: String,
}

#[derive(Debug, Clone)]
pub struct ReportResult {
    pub status: ProviderStatus,
    pub template: String,
    pub content_md: String,
    pub model_used: String,
    pub provider: String,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    Done,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealth {
    pub provider: String,
    pub status: String,
    #[serde(rename = "quotaGate")]
    pub quota_gate: String,
    #[serde(rename = "apiKeyFallback")]
    pub api_key_fallback: bool,
    #[serde(rename = "apiKeyDetected")]
    pub api_key_detected: bool,
    #[serde(rename = "cliAvailable")]
    pub cli_available: bool,
    #[serde(rename = "versionSummary")]
    pub version_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerRunSummary {
    pub processed: usize,
    #[serde(rename = "jobId", skip_serializing_if = "Option::is_none")]
    pub job_id: Option<i64>,
    pub status: String,
}

pub trait FactoryReportProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = ProviderHealth> + Send + 'a>>;
    fn generate<'a>(
        &'a self,
        ctx: &'a InsightJobContext,
    ) -> Pin<Box<dyn Future<Output = ReportResult> + Send + 'a>>;
}

#[derive(Debug, Clone)]
pub struct CodexProvider {
    command: String,
    cwd: Option<PathBuf>,
    timeout: Duration,
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self {
            command: std::env::var("INSIGHT_FACTORY_CODEX_BIN")
                .unwrap_or_else(|_| default_codex_command()),
            cwd: std::env::current_dir().ok(),
            timeout: Duration::from_secs(
                std::env::var("INSIGHT_FACTORY_CODEX_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(300),
            ),
        }
    }
}

impl FactoryReportProvider for CodexProvider {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = ProviderHealth> + Send + 'a>> {
        Box::pin(async move { self.health_check().await })
    }

    fn generate<'a>(
        &'a self,
        ctx: &'a InsightJobContext,
    ) -> Pin<Box<dyn Future<Output = ReportResult> + Send + 'a>> {
        Box::pin(async move { self.generate_report(ctx).await })
    }
}

impl CodexProvider {
    async fn health_check(&self) -> ProviderHealth {
        let api_key_detected = api_key_detected();
        if api_key_detected {
            return ProviderHealth {
                provider: "codex".to_string(),
                status: "blocked".to_string(),
                quota_gate: "api_key_detected".to_string(),
                api_key_fallback: false,
                api_key_detected: true,
                cli_available: false,
                version_summary: String::new(),
                error: Some("OPENAI_API_KEY detected; API billing path is disabled".to_string()),
            };
        }

        let mut cmd = Command::new(&self.command);
        cmd.arg("--version");
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        match tokio::time::timeout(Duration::from_secs(10), cmd.output()).await {
            Ok(Ok(out)) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                ProviderHealth {
                    provider: "codex".to_string(),
                    status: "ready".to_string(),
                    quota_gate: "chatgpt_auth_assumed_from_T-208".to_string(),
                    api_key_fallback: false,
                    api_key_detected: false,
                    cli_available: true,
                    version_summary: sanitize_error(&version),
                    error: None,
                }
            }
            Ok(Ok(out)) => ProviderHealth {
                provider: "codex".to_string(),
                status: "blocked".to_string(),
                quota_gate: "cli_unavailable".to_string(),
                api_key_fallback: false,
                api_key_detected: false,
                cli_available: false,
                version_summary: String::new(),
                error: Some(sanitize_error(&String::from_utf8_lossy(&out.stderr))),
            },
            Ok(Err(e)) => ProviderHealth {
                provider: "codex".to_string(),
                status: "blocked".to_string(),
                quota_gate: "cli_unavailable".to_string(),
                api_key_fallback: false,
                api_key_detected: false,
                cli_available: false,
                version_summary: String::new(),
                error: Some(sanitize_error(&e.to_string())),
            },
            Err(_) => ProviderHealth {
                provider: "codex".to_string(),
                status: "blocked".to_string(),
                quota_gate: "cli_timeout".to_string(),
                api_key_fallback: false,
                api_key_detected: false,
                cli_available: false,
                version_summary: String::new(),
                error: Some("codex --version timed out".to_string()),
            },
        }
    }

    async fn generate_report(&self, ctx: &InsightJobContext) -> ReportResult {
        if api_key_detected() {
            return ReportResult {
                status: ProviderStatus::Blocked,
                template: ctx.template.clone(),
                content_md: String::new(),
                model_used: String::new(),
                provider: "codex".to_string(),
                error_message: "OPENAI_API_KEY detected; API billing path is disabled".to_string(),
            };
        }

        let prompt = build_prompt(ctx);
        let output_path = std::env::temp_dir().join(format!(
            "ergou-insight-factory-{}-{}.md",
            ctx.job_id,
            uuid::Uuid::new_v4()
        ));
        let mut cmd = Command::new(&self.command);
        cmd.arg("exec")
            .arg("--ephemeral")
            .arg("-s")
            .arg("read-only")
            .arg("--skip-git-repo-check")
            .arg("--output-last-message")
            .arg(&output_path)
            .arg("-")
            .stdin(Stdio::piped());
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }

        let child = match cmd.spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
                        return ReportResult {
                            status: ProviderStatus::Failed,
                            template: ctx.template.clone(),
                            content_md: String::new(),
                            model_used: "codex-cli".to_string(),
                            provider: "codex".to_string(),
                            error_message: sanitize_error(&format!("write prompt: {e}")),
                        };
                    }
                }
                child
            }
            Err(e) => {
                return ReportResult {
                    status: ProviderStatus::Failed,
                    template: ctx.template.clone(),
                    content_md: String::new(),
                    model_used: "codex-cli".to_string(),
                    provider: "codex".to_string(),
                    error_message: sanitize_error(&e.to_string()),
                };
            }
        };

        match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(Ok(out)) if out.status.success() => {
                let file_output = tokio::fs::read_to_string(&output_path).await.ok();
                let _ = tokio::fs::remove_file(&output_path).await;
                let content = normalize_codex_output(
                    file_output
                        .as_deref()
                        .unwrap_or_else(|| std::str::from_utf8(&out.stdout).unwrap_or("")),
                );
                if !looks_like_markdown_report(&content) {
                    return ReportResult {
                        status: ProviderStatus::Failed,
                        template: ctx.template.clone(),
                        content_md: String::new(),
                        model_used: "codex-cli".to_string(),
                        provider: "codex".to_string(),
                        error_message: "Codex output was empty or not a complete Markdown report"
                            .to_string(),
                    };
                }
                ReportResult {
                    status: ProviderStatus::Done,
                    template: ctx.template.clone(),
                    content_md: content,
                    model_used: "codex-cli".to_string(),
                    provider: "codex".to_string(),
                    error_message: String::new(),
                }
            }
            Ok(Ok(out)) => ReportResult {
                status: ProviderStatus::Failed,
                template: ctx.template.clone(),
                content_md: String::new(),
                model_used: "codex-cli".to_string(),
                provider: "codex".to_string(),
                error_message: sanitize_error(&String::from_utf8_lossy(&out.stderr)),
            },
            Ok(Err(e)) => ReportResult {
                status: ProviderStatus::Failed,
                template: ctx.template.clone(),
                content_md: String::new(),
                model_used: "codex-cli".to_string(),
                provider: "codex".to_string(),
                error_message: sanitize_error(&e.to_string()),
            },
            Err(_) => ReportResult {
                status: ProviderStatus::Failed,
                template: ctx.template.clone(),
                content_md: String::new(),
                model_used: "codex-cli".to_string(),
                provider: "codex".to_string(),
                error_message: "Codex generation timed out".to_string(),
            },
        }
    }
}

pub async fn codex_health() -> ProviderHealth {
    CodexProvider::default().health().await
}

pub fn spawn_worker(db: Arc<Mutex<Connection>>) {
    if std::env::var("INSIGHT_FACTORY_WORKER")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        info!(target: "insight_factory_worker", "worker disabled by INSIGHT_FACTORY_WORKER");
        return;
    }

    tokio::spawn(async move {
        let provider = CodexProvider::default();
        let interval_secs = std::env::var("INSIGHT_FACTORY_WORKER_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        loop {
            match run_once(db.clone(), &provider).await {
                Ok(summary) if summary.processed > 0 => {
                    info!(target: "insight_factory_worker", ?summary, "processed job");
                }
                Ok(_) => {}
                Err(e) => {
                    error!(target: "insight_factory_worker", error = %e, "worker run failed");
                }
            }
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    });
}

pub async fn run_once<P: FactoryReportProvider>(
    db: Arc<Mutex<Connection>>,
    provider: &P,
) -> Result<WorkerRunSummary, String> {
    let Some(ctx) = claim_next_job(db.clone(), provider.name())? else {
        return Ok(WorkerRunSummary {
            processed: 0,
            job_id: None,
            status: "idle".to_string(),
        });
    };

    let result = provider.generate(&ctx).await;
    let status = match result.status {
        ProviderStatus::Done => {
            persist_report(db.clone(), &ctx, result)?;
            "done"
        }
        ProviderStatus::Blocked => {
            mark_job_terminal(db.clone(), &ctx, "blocked", &result.error_message)?;
            "blocked"
        }
        ProviderStatus::Failed => {
            mark_job_terminal(db.clone(), &ctx, "failed", &result.error_message)?;
            "failed"
        }
    };

    Ok(WorkerRunSummary {
        processed: 1,
        job_id: Some(ctx.job_id),
        status: status.to_string(),
    })
}

fn claim_next_job(
    db: Arc<Mutex<Connection>>,
    provider_name: &str,
) -> Result<Option<InsightJobContext>, String> {
    let mut db = db.lock();
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let job_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM factory_jobs
             WHERE status = 'pending' AND provider = ?1
             ORDER BY created_at ASC, id ASC LIMIT 1",
            params![provider_name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(job_id) = job_id else {
        return Ok(None);
    };

    let now = now_rfc3339();
    let changed = tx
        .execute(
            "UPDATE factory_jobs
             SET status = 'running', started_at = ?1, error_message = ''
             WHERE id = ?2 AND status = 'pending'",
            params![now, job_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Ok(None);
    }

    let ctx = load_context(&tx, job_id)?;
    tx.execute(
        "UPDATE factory_tasks
         SET status = 'running', updated_at = ?1
         WHERE id = ?2 AND user_id = ?3",
        params![now, ctx.task_id, ctx.user_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Some(ctx))
}

fn load_context(db: &Connection, job_id: i64) -> Result<InsightJobContext, String> {
    let row = db
        .query_row(
            "SELECT j.id, j.task_id, j.user_id, j.mode, j.template, j.feedback_note,
                    j.parent_report_id, j.retry_of_job_id,
                    t.input_type, t.input_content, t.source_snapshot, t.template
             FROM factory_jobs j
             JOIN factory_tasks t ON t.id = j.task_id AND t.user_id = j.user_id
             WHERE j.id = ?1 AND t.deleted = 0",
            params![job_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, Option<String>>(11)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    let (
        job_id,
        task_id,
        user_id,
        mode,
        job_template,
        feedback_note,
        parent_report_id,
        retry_of_job_id,
        input_type,
        input_content,
        source_snapshot,
        task_template,
    ) = row;
    let template = job_template
        .or(task_template)
        .unwrap_or_else(|| "survey".to_string());
    let previous_report_md = match parent_report_id {
        Some(id) => db
            .query_row(
                "SELECT content_md FROM factory_reports WHERE id = ?1 AND task_id = ?2",
                params![id, task_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_default(),
        None => String::new(),
    };

    Ok(InsightJobContext {
        task_id,
        job_id,
        user_id: user_id.clone(),
        mode,
        input_type,
        input_content,
        source_snapshot,
        template,
        memory_context_md: load_memory_context(db, &user_id)?,
        previous_report_md,
        feedback_note,
        parent_report_id,
        retry_of_job_id,
        report_contract_md: report_contract_md(),
    })
}

fn load_memory_context(db: &Connection, user_id: &str) -> Result<String, String> {
    let mut stmt = db
        .prepare(
            "SELECT type, title, body
             FROM factory_memories
             WHERE user_id = ?1 AND enabled = 1
             ORDER BY CASE type
                WHEN 'project_fact' THEN 1
                WHEN 'boris_profile' THEN 2
                WHEN 'report_preference' THEN 3
                WHEN 'insight_summary' THEN 4
                ELSE 9
             END, importance DESC, updated_at DESC
             LIMIT 40",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![user_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = String::new();
    for row in rows {
        let (kind, title, body) = row.map_err(|e| e.to_string())?;
        out.push_str(&format!("- [{}] {}: {}\n", kind, title, body));
    }
    Ok(out)
}

fn persist_report(
    db: Arc<Mutex<Connection>>,
    ctx: &InsightJobContext,
    result: ReportResult,
) -> Result<(), String> {
    let mut db = db.lock();
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let now = now_rfc3339();
    let next_version: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM factory_reports WHERE task_id = ?1",
            params![ctx.task_id],
            |r| r.get(0),
        )
        .unwrap_or(1);
    tx.execute(
        "INSERT INTO factory_reports
            (task_id, job_id, user_id, version, template, content_md, parent_report_id,
             revision_note, generated_by, provider, model_used, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'factory-worker', ?9, ?10, ?11)",
        params![
            ctx.task_id,
            ctx.job_id,
            ctx.user_id,
            next_version,
            result.template,
            result.content_md,
            ctx.parent_report_id,
            ctx.feedback_note,
            result.provider,
            result.model_used,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    let report_id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE factory_jobs SET status = 'done', finished_at = ?1, error_message = '' WHERE id = ?2",
        params![now, ctx.job_id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE factory_tasks
         SET status = 'done', current_report_id = ?1, template = COALESCE(template, ?2), updated_at = ?3
         WHERE id = ?4 AND user_id = ?5",
        params![report_id, result.template, now, ctx.task_id, ctx.user_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn mark_job_terminal(
    db: Arc<Mutex<Connection>>,
    ctx: &InsightJobContext,
    status: &str,
    error_message: &str,
) -> Result<(), String> {
    let db = db.lock();
    let now = now_rfc3339();
    let clean = sanitize_error(error_message);
    db.execute(
        "UPDATE factory_jobs
         SET status = ?1, finished_at = ?2, error_message = ?3
         WHERE id = ?4",
        params![status, now, clean, ctx.job_id],
    )
    .map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE factory_tasks SET status = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
        params![status, now, ctx.task_id, ctx.user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn build_prompt(ctx: &InsightJobContext) -> String {
    let effective_mode = effective_job_mode(ctx);
    let retry_note = retry_instruction(ctx, effective_mode);
    format!(
        r#"你是洞察工厂的报告生成 worker。只输出一份完整 Markdown 报告，不要输出解释、diff、JSON 或代码块围栏。

{contract}

## Job
- task_id: {task_id}
- job_id: {job_id}
- mode: {mode}
- stored_mode: {stored_mode}
- retry_of_job_id: {retry_of_job_id}
- template: {template}
- input_type: {input_type}
{retry_note}

## Input
{input}

## Source Snapshot
{source_snapshot}

## Factory Memories
{memories}

## Previous Report
{previous_report}

## Feedback Note
{feedback}
"#,
        contract = ctx.report_contract_md,
        task_id = ctx.task_id,
        job_id = ctx.job_id,
        mode = effective_mode,
        stored_mode = ctx.mode,
        retry_of_job_id = ctx
            .retry_of_job_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "(none)".to_string()),
        template = ctx.template,
        input_type = ctx.input_type,
        retry_note = retry_note,
        input = ctx.input_content,
        source_snapshot = empty_marker(&ctx.source_snapshot),
        memories = empty_marker(&ctx.memory_context_md),
        previous_report = empty_marker(&ctx.previous_report_md),
        feedback = empty_marker(&ctx.feedback_note),
    )
}

fn effective_job_mode(ctx: &InsightJobContext) -> &'static str {
    match ctx.mode.as_str() {
        "retry" if ctx.parent_report_id.is_some() || !ctx.feedback_note.trim().is_empty() => {
            "revise"
        }
        "retry" => "generate",
        "revise" => "revise",
        _ => "generate",
    }
}

fn retry_instruction(ctx: &InsightJobContext, effective_mode: &str) -> String {
    match ctx.retry_of_job_id {
        Some(id) => format!(
            "- retry_note: retrying failed job #{id}; execute it as a {effective_mode} job using the copied context below."
        ),
        None => String::new(),
    }
}

fn report_contract_md() -> String {
    [
        "## 报告契约",
        "- 输出必须是完整 Markdown 报告。",
        "- 首屏给出明确标题和结论摘要。",
        "- 保留关键事实、推理链、风险、反方观点和可执行建议。",
        "- 修订任务必须吸收 feedback_note，输出完整新版，不输出 diff。",
        "- 不要编造来源；不确定处明确标注。",
    ]
    .join("\n")
}

fn empty_marker(s: &str) -> &str {
    if s.trim().is_empty() {
        "(none)"
    } else {
        s
    }
}

fn normalize_codex_output(s: &str) -> String {
    s.trim()
        .trim_start_matches("```markdown")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}

fn looks_like_markdown_report(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.chars().count() >= 80 && trimmed.contains('#')
}

fn api_key_detected() -> bool {
    std::env::var("OPENAI_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn sanitize_error(raw: &str) -> String {
    let mut s = raw.replace(['\n', '\r'], " ");
    for marker in ["OPENAI_API_KEY", "sk-", "session", "token", "api key"] {
        s = s.replace(marker, "[redacted]");
    }
    if s.chars().count() > 300 {
        s = s.chars().take(300).collect();
        s.push_str("...");
    }
    if s.trim().is_empty() {
        "worker failed".to_string()
    } else {
        s.trim().to_string()
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn default_codex_command() -> String {
    if cfg!(windows) {
        "codex.cmd".to_string()
    } else {
        "codex".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{create_test_user, test_state};

    struct FakeProvider {
        status: ProviderStatus,
    }

    impl FactoryReportProvider for FakeProvider {
        fn name(&self) -> &'static str {
            "codex"
        }

        fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = ProviderHealth> + Send + 'a>> {
            Box::pin(async {
                ProviderHealth {
                    provider: "codex".to_string(),
                    status: "ready".to_string(),
                    quota_gate: "test".to_string(),
                    api_key_fallback: false,
                    api_key_detected: false,
                    cli_available: true,
                    version_summary: "test".to_string(),
                    error: None,
                }
            })
        }

        fn generate<'a>(
            &'a self,
            ctx: &'a InsightJobContext,
        ) -> Pin<Box<dyn Future<Output = ReportResult> + Send + 'a>> {
            Box::pin(async move {
                match self.status {
                    ProviderStatus::Done => ReportResult {
                        status: ProviderStatus::Done,
                        template: ctx.template.clone(),
                        content_md: format!(
                            "# Factory Report\n\nmode={} task={} memory={}\n\nThis is a complete Markdown report with enough body to pass validation.",
                            ctx.mode,
                            ctx.task_id,
                            ctx.memory_context_md
                        ),
                        model_used: "fake".to_string(),
                        provider: "codex".to_string(),
                        error_message: String::new(),
                    },
                    ProviderStatus::Blocked => ReportResult {
                        status: ProviderStatus::Blocked,
                        template: ctx.template.clone(),
                        content_md: String::new(),
                        model_used: String::new(),
                        provider: "codex".to_string(),
                        error_message: "OPENAI_API_KEY detected".to_string(),
                    },
                    ProviderStatus::Failed => ReportResult {
                        status: ProviderStatus::Failed,
                        template: ctx.template.clone(),
                        content_md: String::new(),
                        model_used: String::new(),
                        provider: "codex".to_string(),
                        error_message: "boom".to_string(),
                    },
                }
            })
        }
    }

    fn create_task_with_job(state: &crate::state::AppState, user_id: &str) -> (i64, i64) {
        let db = state.db.lock();
        let now = now_rfc3339();
        db.execute(
            "INSERT INTO factory_tasks
                (user_id, title, input_type, input_content, template, status, source_snapshot, created_at, updated_at)
             VALUES (?1, 'T209', 'topic', 'subscription worker test', 'survey', 'pending', 'snapshot', ?2, ?2)",
            params![user_id, now],
        )
        .unwrap();
        let task_id = db.last_insert_rowid();
        db.execute(
            "INSERT INTO factory_jobs
                (task_id, user_id, mode, status, provider, template, created_at)
             VALUES (?1, ?2, 'generate', 'pending', 'codex', 'survey', ?3)",
            params![task_id, user_id, now],
        )
        .unwrap();
        (task_id, db.last_insert_rowid())
    }

    #[tokio::test]
    async fn dispatcher_writes_v1_then_v2_report() {
        let state = test_state();
        let (user_id, _) = create_test_user(&state, "factory-worker-ok", "Pa55word1");
        let (task_id, _job_id) = create_task_with_job(&state, &user_id);
        {
            let db = state.db.lock();
            let now = now_rfc3339();
            db.execute(
                "INSERT INTO factory_memories
                    (user_id, type, title, body, source, importance, enabled, created_at, updated_at)
                 VALUES (?1, 'project_fact', 'Fact', 'Memory Body', 'test', 5, 1, ?2, ?2)",
                params![user_id, now],
            )
            .unwrap();
        }

        let provider = FakeProvider {
            status: ProviderStatus::Done,
        };
        let first = run_once(state.db.clone(), &provider).await.unwrap();
        assert_eq!(first.processed, 1);
        assert_eq!(first.status, "done");

        let parent_report_id: i64 = {
            let db = state.db.lock();
            db.query_row(
                "SELECT current_report_id FROM factory_tasks WHERE id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .unwrap()
        };

        {
            let db = state.db.lock();
            let now = now_rfc3339();
            db.execute(
                "INSERT INTO factory_jobs
                    (task_id, user_id, mode, status, provider, template, feedback_note, parent_report_id, created_at)
                 VALUES (?1, ?2, 'revise', 'pending', 'codex', 'survey', 'make it sharper', ?3, ?4)",
                params![task_id, user_id, parent_report_id, now],
            )
            .unwrap();
            db.execute(
                "UPDATE factory_tasks SET status = 'pending' WHERE id = ?1",
                params![task_id],
            )
            .unwrap();
        }

        let second = run_once(state.db.clone(), &provider).await.unwrap();
        assert_eq!(second.processed, 1);
        let db = state.db.lock();
        let versions: i64 = db
            .query_row(
                "SELECT MAX(version) FROM factory_reports WHERE task_id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .unwrap();
        let revision: String = db
            .query_row(
                "SELECT revision_note FROM factory_reports WHERE task_id = ?1 AND version = 2",
                params![task_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(versions, 2);
        assert_eq!(revision, "make it sharper");
    }

    #[tokio::test]
    async fn dispatcher_blocks_job_when_provider_blocks_api_key_path() {
        let state = test_state();
        let (user_id, _) = create_test_user(&state, "factory-worker-block", "Pa55word1");
        let (task_id, job_id) = create_task_with_job(&state, &user_id);
        let provider = FakeProvider {
            status: ProviderStatus::Blocked,
        };

        let summary = run_once(state.db.clone(), &provider).await.unwrap();
        assert_eq!(summary.processed, 1);
        assert_eq!(summary.status, "blocked");

        let db = state.db.lock();
        let status: String = db
            .query_row(
                "SELECT status FROM factory_jobs WHERE id = ?1",
                params![job_id],
                |r| r.get(0),
            )
            .unwrap();
        let task_status: String = db
            .query_row(
                "SELECT status FROM factory_tasks WHERE id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .unwrap();
        let reports: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM factory_reports WHERE task_id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "blocked");
        assert_eq!(task_status, "blocked");
        assert_eq!(reports, 0);
    }

    #[test]
    fn retry_prompt_preserves_effective_revise_semantics() {
        let ctx = InsightJobContext {
            task_id: 1,
            job_id: 3,
            user_id: "u".to_string(),
            mode: "retry".to_string(),
            input_type: "topic".to_string(),
            input_content: "AI search".to_string(),
            source_snapshot: String::new(),
            template: "survey".to_string(),
            memory_context_md: String::new(),
            previous_report_md: "# Previous\n\nBody".to_string(),
            feedback_note: "make it sharper".to_string(),
            parent_report_id: Some(2),
            retry_of_job_id: Some(2),
            report_contract_md: report_contract_md(),
        };

        let prompt = build_prompt(&ctx);
        assert!(prompt.contains("- mode: revise"), "{prompt}");
        assert!(prompt.contains("- stored_mode: retry"), "{prompt}");
        assert!(prompt.contains("- retry_of_job_id: 2"), "{prompt}");
        assert!(prompt.contains("execute it as a revise job"), "{prompt}");
    }

    #[test]
    fn memory_context_uses_spec_order_and_skips_disabled_items() {
        let state = test_state();
        let (user_id, _) = create_test_user(&state, "factory-memory-order", "Pa55word1");
        let db = state.db.lock();
        let now = now_rfc3339();
        let rows = [
            ("insight_summary", "S", "summary", 5, 1),
            ("report_preference", "R", "preference", 1, 1),
            ("project_fact", "P", "fact", 1, 1),
            ("boris_profile", "B", "profile", 1, 1),
            ("project_fact", "P-off", "disabled fact", 5, 0),
        ];
        for (kind, title, body, importance, enabled) in rows {
            db.execute(
                "INSERT INTO factory_memories
                    (user_id, type, title, body, source, importance, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'test', ?5, ?6, ?7, ?7)",
                params![user_id, kind, title, body, importance, enabled, now],
            )
            .unwrap();
        }

        let ctx = load_memory_context(&db, &user_id).unwrap();
        let p = ctx.find("[project_fact]").unwrap();
        let b = ctx.find("[boris_profile]").unwrap();
        let r = ctx.find("[report_preference]").unwrap();
        let s = ctx.find("[insight_summary]").unwrap();
        assert!(p < b && b < r && r < s, "unexpected context order: {ctx}");
        assert!(!ctx.contains("disabled fact"));
    }
}
