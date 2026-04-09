use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub is_archived: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content_text: Option<String>,
    pub content_json: Option<String>,
    pub tool_name: Option<String>,
    pub token_count: Option<i64>,
    pub image_uris: Option<String>,
    pub created_at: String,
    pub sequence: i64,
}
