use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseEntry {
    pub id: String,
    pub amount: f64,
    pub date: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ai_processed: bool,
    #[serde(default = "default_cad")]
    pub currency: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub photo_count: i64,
    #[serde(default)]
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseItem {
    pub id: String,
    pub entry_id: String,
    pub name: String,
    #[serde(default = "default_one")]
    pub quantity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<f64>,
    pub amount: f64,
    #[serde(default)]
    pub specs: String,
    #[serde(default)]
    pub sort_order: i32,
}

fn default_one() -> f64 {
    1.0
}

fn default_cad() -> String {
    "CAD".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpensePhoto {
    pub id: String,
    pub entry_id: String,
    pub filename: String,
    pub file_size: i64,
    pub mime_type: String,
    pub created_at: String,
    #[serde(default)]
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseEntryDetail {
    #[serde(flatten)]
    pub entry: ExpenseEntry,
    pub items: Vec<ExpenseItem>,
    pub photos: Vec<ExpensePhoto>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExpenseRequest {
    pub amount: f64,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub items: Option<Vec<CreateItemRequest>>,
    #[serde(default)]
    pub ai_processed: Option<bool>,
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateItemRequest {
    pub name: String,
    #[serde(default = "default_one")]
    pub quantity: f64,
    #[serde(default)]
    pub unit_price: Option<f64>,
    pub amount: f64,
    #[serde(default)]
    pub specs: Option<String>,
}

// ===== Parse Preview types =====

#[derive(Debug, Deserialize)]
pub struct ParsePreviewImage {
    pub data: String,
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ParsePreviewRequest {
    #[serde(default)]
    pub images: Vec<ParsePreviewImage>,
    /// User-provided text (notes, amount description) for AI analysis
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ParsePreviewResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<PreviewData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_remaining: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct PreviewData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub items: Vec<PreviewItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtotal: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_amount: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PreviewItem {
    pub name: String,
    pub quantity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<f64>,
    pub amount: f64,
    #[serde(default)]
    pub specs: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExpenseRequest {
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseListQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseSummaryQuery {
    #[serde(default = "default_period")]
    pub period: String,
    pub date: Option<String>,
}

fn default_period() -> String {
    "day".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ExpenseAnalyticsQuery {
    pub period: String,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseStatsQuery {
    #[serde(default = "default_stats_period")]
    pub period: String,
    pub date: Option<String>,
}

fn default_stats_period() -> String {
    "month".to_string()
}

#[derive(Debug, Serialize)]
pub struct ExpenseStats {
    pub period: String,
    pub from: String,
    pub to: String,
    pub total_amount: f64,
    pub entry_count: i64,
    pub tag_totals: Vec<TagTotal>,
    pub category_totals: Vec<CategoryTotal>,
    pub daily: Vec<DailyTotal>,
    pub comparison: Comparison,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub prev_total: f64,
    pub change_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct CategoryTotal {
    pub category: String,
    pub amount: f64,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct DailyTotal {
    pub date: String,
    pub amount: f64,
}

#[derive(Debug, Serialize)]
pub struct ExpenseSummary {
    pub total_amount: f64,
    pub entry_count: i64,
    pub period: String,
    pub from: String,
    pub to: String,
    pub tag_totals: Vec<TagTotal>,
}

#[derive(Debug, Serialize)]
pub struct TagTotal {
    pub tag: String,
    pub amount: f64,
    pub count: i64,
}
