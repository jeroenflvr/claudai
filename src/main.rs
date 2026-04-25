use askama_axum::IntoResponse;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::info;

// ---------------------------------------------------------------------------
// Template structs
// ---------------------------------------------------------------------------

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexTemplate;

#[derive(askama::Template)]
#[template(path = "response.html")]
struct ResponseTemplate {
    content: String,
    history_json: String,
}

// ---------------------------------------------------------------------------
// API types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
    #[serde(default)]
    history: Vec<Message>,
    /// Client-generated UUID, stable for the browser tab's lifetime.
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    text: String,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct AppState {
    api_key: String,
    model: String,
    http: reqwest::Client,
    db: Mutex<Connection>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn index() -> impl IntoResponse {
    IndexTemplate
}

async fn chat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // The client sends a POST with X-HTTP-Method-Override: GET.
    // This is the standard workaround when GET-with-body is desired but
    // browsers (Fetch spec §4.1.5) and header size limits both prevent it.
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

    // Build the full message list: history + new user turn
    let mut messages = req.history.clone();
    messages.push(Message {
        role: "user".to_string(),
        content: req.message.clone(),
    });

    info!(
        "Sending {} message(s) to Claude (model: {})",
        messages.len(),
        state.model
    );

    // Call the Anthropic Messages API
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
        let text = api_resp.text().await.unwrap_or_default();
        return Err((StatusCode::BAD_GATEWAY, format!("Claude API error {status}: {text}")));
    }

    let claude: ClaudeResponse = api_resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to parse Claude response: {e}")))?;

    let content = claude
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .unwrap_or_else(|| "(no response)".to_string());

    // Append assistant turn so the client can carry context forward
    messages.push(Message {
        role: "assistant".to_string(),
        content: content.clone(),
    });

    // ── persist this turn to DuckDB ──────────────────────────────────────
    let turn = (messages.len() / 2) as i32; // each turn = 1 user + 1 assistant
    let now = Utc::now().to_rfc3339();
    {
        let db = state.db.lock().map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lock error: {e}"))
        })?;
        db.execute(
            "INSERT INTO turns (session_id, turn, user_message, assistant_response, created_at)
             VALUES (?, ?, ?, ?, ?)",
            duckdb::params![req.session_id, turn, req.message, content, now],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB write error: {e}")))?;
    }
    info!(session = %req.session_id, turn, "Turn logged to DB");
    // ─────────────────────────────────────────────────────────────────────

    let history_json = serde_json::to_string(&messages)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialisation error: {e}")))?;

    Ok(ResponseTemplate {
        content,
        history_json,
    })
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "claudai=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    let model = std::env::var("CLAUDE_MODEL")
        .unwrap_or_else(|_| "claude-opus-4-5".to_string());

    info!("Using model: {model}");

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "claudai.duckdb".to_string());
    let db = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("Failed to open DuckDB at {db_path}: {e}"));
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS turns (
            session_id        VARCHAR   NOT NULL,
            turn              INTEGER   NOT NULL,
            user_message      TEXT      NOT NULL,
            assistant_response TEXT     NOT NULL,
            created_at        VARCHAR   NOT NULL
        );",
    )
    .expect("Failed to create turns table");
    info!("DuckDB open at {db_path}");

    let state = Arc::new(AppState {
        api_key,
        model,
        http: reqwest::Client::new(),
        db: Mutex::new(db),
    });

    let app = Router::new()
        .route("/", get(index))
        // Registered as POST so the browser can send a body (GET bodies are
        // rejected by the Fetch spec).  The client sets
        // X-HTTP-Method-Override: GET to declare the semantic intent.
        .route("/chat", post(chat))
        .with_state(state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
