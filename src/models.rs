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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_serialises_and_roundtrips() {
        let msg = Message { role: "user".into(), content: "hello".into() };
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, "user");
        assert_eq!(back.content, "hello");
    }

    #[test]
    fn chat_request_history_defaults_to_empty() {
        let json = r#"{"message":"hi","session_id":"abc-123"}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "hi");
        assert_eq!(req.session_id, "abc-123");
        assert!(req.history.is_empty());
    }

    #[test]
    fn chat_request_with_history_parses() {
        let json = r#"{"message":"follow-up","session_id":"s1","history":[{"role":"user","content":"first"},{"role":"assistant","content":"reply"}]}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.history.len(), 2);
        assert_eq!(req.history[0].role, "user");
        assert_eq!(req.history[1].role, "assistant");
    }

    #[test]
    fn claude_api_response_parses_text_content() {
        let json = r#"{"content":[{"type":"text","text":"Hello!"}]}"#;
        let resp: ClaudeApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.content[0].text, "Hello!");
    }

    #[test]
    fn session_summary_serialises() {
        let s = SessionSummary {
            session_id:    "abc".into(),
            first_message: "hello".into(),
            turn_count:    3,
            started_at:    "2024-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"turn_count\":3"));
        assert!(json.contains("\"session_id\":\"abc\""));
    }
}
