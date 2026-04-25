use crate::{
    auth::require_auth,
    models::{ChatRequest, ClaudeApiResponse, Message},
    state::AppState,
    templates::ResponseTemplate,
};
use askama_axum::IntoResponse;
use axum::{body::Bytes, extract::State, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use std::sync::Arc;
use tower_sessions::Session;
use tracing::info;

pub async fn chat(
    State(state): State<Arc<AppState>>,
    session: Session,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    require_auth(&session, &state.base_path).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let override_hdr = headers
        .get("x-http-method-override")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !override_hdr.eq_ignore_ascii_case("GET") {
        return Err((
            StatusCode::BAD_REQUEST,
            "X-HTTP-Method-Override: GET header required".into(),
        ));
    }

    let req: ChatRequest = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Bad request JSON: {e}")))?;

    if req.message.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Empty message".into()));
    }

    let mut messages = req.history.clone();
    messages.push(Message { role: "user".to_string(), content: req.message.clone() });

    info!("Sending {} message(s) to Claude (model: {})", messages.len(), state.model);

    let api_resp = state
        .http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &state.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": state.model,
            "max_tokens": 8096,
            "messages": messages,
        }))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("API request failed: {e}")))?;

    if !api_resp.status().is_success() {
        let status = api_resp.status();
        let text   = api_resp.text().await.unwrap_or_default();
        return Err((StatusCode::BAD_GATEWAY, format!("Claude API error {status}: {text}")));
    }

    let claude: ClaudeApiResponse = api_resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to parse Claude response: {e}")))?;

    let content = claude
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .unwrap_or_else(|| "(no response)".to_string());

    messages.push(Message { role: "assistant".to_string(), content: content.clone() });

    let turn = (messages.len() / 2) as i32;
    let now  = Utc::now().to_rfc3339();
    {
        let db = state.db.lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lock error: {e}")))?;
        db.execute(
            "INSERT INTO turns (session_id, turn, user_message, assistant_response, created_at)
             VALUES (?, ?, ?, ?, ?)",
            duckdb::params![req.session_id, turn, req.message, content, now],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB write error: {e}")))?;
    }
    info!(session = %req.session_id, turn, "Turn logged to DB");

    let history_json = serde_json::to_string(&messages)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialisation error: {e}")))?;

    Ok(ResponseTemplate { content, history_json })
}
