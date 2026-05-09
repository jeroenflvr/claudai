use crate::{
    auth::require_auth,
    models::{ChatRequest, Message},
    state::AppState,
};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use chrono::Utc;
use futures::StreamExt;
use std::{convert::Infallible, sync::Arc};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tower_sessions::Session;
use tracing::info;

pub async fn chat(
    State(state): State<Arc<AppState>>,
    session: Session,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, (StatusCode, String)> {
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

    info!("Streaming {} message(s) to Claude (model: {})", messages.len(), state.model);

    let api_resp = state
        .http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &state.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": state.model,
            "max_tokens": 8096,
            "stream": true,
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

    let (tx, rx)     = mpsc::channel::<Result<Event, Infallible>>(64);
    let session_id   = req.session_id.clone();
    let user_message = req.message.clone();
    let state        = state.clone();

    tokio::spawn(async move {
        let mut full_text   = String::new();
        let mut buf         = String::new();
        let mut byte_stream = api_resp.bytes_stream();

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c)  => c,
                Err(e) => { tracing::error!("Stream read error: {e}"); break; }
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));

            // consume all complete lines from the buffer
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim_end_matches('\r').to_string();
                buf = buf[pos + 1..].to_string();

                let Some(data) = line.strip_prefix("data: ") else { continue };

                let val = match serde_json::from_str::<serde_json::Value>(data) {
                    Ok(v)  => v,
                    Err(_) => continue,
                };

                match val["type"].as_str() {
                    Some("content_block_delta") => {
                        if let Some(text) = val["delta"]["text"].as_str() {
                            full_text.push_str(text);
                            let payload = serde_json::json!({"type":"delta","text":text}).to_string();
                            let _ = tx.send(Ok(Event::default().data(payload))).await;
                        }
                    }
                    Some("message_stop") => {
                        messages.push(Message {
                            role:    "assistant".to_string(),
                            content: full_text.clone(),
                        });
                        let turn = (messages.len() / 2) as i32;
                        let now  = Utc::now().to_rfc3339();
                        if let Ok(db) = state.db.lock() {
                            let _ = db.execute(
                                "INSERT INTO turns (session_id, turn, user_message, assistant_response, created_at)
                                 VALUES (?, ?, ?, ?, ?)",
                                duckdb::params![session_id, turn, user_message, full_text, now],
                            );
                        }
                        info!(session = %session_id, turn, "Turn logged to DB");
                        let payload = serde_json::json!({"type":"done","history":messages}).to_string();
                        let _ = tx.send(Ok(Event::default().data(payload))).await;
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
