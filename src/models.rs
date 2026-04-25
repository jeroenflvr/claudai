use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id:    String,
    pub first_message: String,
    pub turn_count:    i64,
    pub started_at:    String,
}

#[derive(Debug, Serialize)]
pub struct TurnRow {
    pub turn:               i32,
    pub user_message:       String,
    pub assistant_response: String,
    pub created_at:         String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub role:    String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message:    String,
    #[serde(default)]
    pub history:    Vec<Message>,
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeApiResponse {
    pub content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeContent {
    pub text: String,
}
