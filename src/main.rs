use askama_axum::IntoResponse;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Redirect,
    routing::{get, post},
    Form, Json, Router,
};
use chrono::Utc;
use duckdb::Connection;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message as EmailMessage, Tokio1Executor,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_sessions::{MemoryStore, Session, SessionManagerLayer};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Session keys
// ---------------------------------------------------------------------------
const SESSION_AUTH_STEP: &str = "auth_step";  // "password_ok" | "authenticated"
const SESSION_OTP: &str       = "otp";        // 6-digit code
const SESSION_OTP_EXP: &str   = "otp_exp";   // unix timestamp expiry

// ---------------------------------------------------------------------------
// Template structs
// ---------------------------------------------------------------------------

#[derive(askama::Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: String,
    #[allow(dead_code)]
    base_path: String,
}

#[derive(askama::Template)]
#[template(path = "verify.html")]
struct VerifyTemplate {
    error: String,
    #[allow(dead_code)]
    base_path: String,
}

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    base_path: String,
}

#[derive(askama::Template)]
#[template(path = "response.html")]
struct ResponseTemplate {
    content: String,
    history_json: String,
}

// ---------------------------------------------------------------------------
// DB row types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct SessionSummary {
    session_id: String,
    first_message: String,
    turn_count: i64,
    started_at: String,
}

#[derive(Debug, Serialize)]
struct TurnRow {
    turn: i32,
    user_message: String,
    assistant_response: String,
    created_at: String,
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
    // Auth
    password: String,
    smtp_host: String,
    smtp_port: u16,
    smtp_user: String,
    smtp_pass: String,
    auth_email: String,
    #[allow(dead_code)]
    app_url: String,
    base_path: String,
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..=999999))
}

async fn send_otp_email(state: &AppState, otp: &str) -> Result<(), String> {
    let email = EmailMessage::builder()
        .from(
            format!("claudia <{}>", state.smtp_user)
                .parse()
                .map_err(|e| format!("Bad from address: {e}"))?,
        )
        .to(state
            .auth_email
            .parse()
            .map_err(|e| format!("Bad to address: {e}"))?)
        .subject("claudia — your login code")
        .header(ContentType::TEXT_PLAIN)
        .body(format!(
            "Your one-time login code is:\n\n  {otp}\n\nIt expires in 5 minutes."
        ))
        .map_err(|e| format!("Build email: {e}"))?;

    let creds = Credentials::new(state.smtp_user.clone(), state.smtp_pass.clone());

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&state.smtp_host)
            .map_err(|e| format!("SMTP relay: {e}"))?
            .port(state.smtp_port)
            .credentials(creds)
            .build();

    mailer
        .send(email)
        .await
        .map_err(|e| format!("SMTP send: {e}"))?;

    Ok(())
}

/// Guard — returns Err(redirect) when the session is not fully authenticated.
async fn require_auth(session: &Session) -> Result<(), Redirect> {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    if step.as_deref() == Some("authenticated") {
        Ok(())
    } else {
        Err(Redirect::to("/login"))
    }
}

// ---------------------------------------------------------------------------
// Auth handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LoginForm {
    password: String,
}

#[derive(Deserialize)]
struct VerifyForm {
    otp: String,
}

async fn get_login(State(state): State<Arc<AppState>>, session: Session) -> impl IntoResponse {
    // If already authenticated, skip straight to the app
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    if step.as_deref() == Some("authenticated") {
        return Redirect::to("/").into_response();
    }
    LoginTemplate { error: String::new(), base_path: state.base_path.clone() }.into_response()
}

async fn post_login(
    State(state): State<Arc<AppState>>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    if form.password != state.password {
        warn!("Failed login attempt");
        return LoginTemplate {
            error: "Incorrect password.".into(),
            base_path: state.base_path.clone(),
        }
        .into_response();
    }

    let otp = generate_otp();
    let exp = Utc::now().timestamp() + 300; // 5 minutes

    session.insert(SESSION_OTP, &otp).await.ok();
    session.insert(SESSION_OTP_EXP, exp).await.ok();
    session.insert(SESSION_AUTH_STEP, "password_ok").await.ok();

    match send_otp_email(&state, &otp).await {
        Ok(()) => {
            info!("OTP sent to {}", state.auth_email);
            Redirect::to("/verify").into_response()
        }
        Err(e) => {
            warn!("Failed to send OTP email: {e}");
            // Clear the session so the user can retry
            session.delete().await.ok();
            LoginTemplate {
                error: format!("Could not send verification email: {e}"),
                base_path: state.base_path.clone(),
            }
            .into_response()
        }
    }
}

async fn get_verify(State(state): State<Arc<AppState>>, session: Session) -> impl IntoResponse {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    match step.as_deref() {
        Some("authenticated") => Redirect::to("/").into_response(),
        Some("password_ok")   => VerifyTemplate { error: String::new(), base_path: state.base_path.clone() }.into_response(),
        _                     => Redirect::to("/login").into_response(),
    }
}

async fn post_verify(
    State(state): State<Arc<AppState>>,
    session: Session,
    Form(form): Form<VerifyForm>,
) -> impl IntoResponse {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    if step.as_deref() != Some("password_ok") {
        return Redirect::to("/login").into_response();
    }

    let stored_otp: Option<String> = session.get(SESSION_OTP).await.unwrap_or(None);
    let exp: Option<i64>           = session.get(SESSION_OTP_EXP).await.unwrap_or(None);

    let now = Utc::now().timestamp();
    let expired = exp.map(|e| now > e).unwrap_or(true);

    if expired {
        session.delete().await.ok();
        return Redirect::to("/login?expired=1").into_response();
    }

    if stored_otp.as_deref() != Some(form.otp.trim()) {
        warn!("Incorrect OTP attempt");
        return VerifyTemplate {
            error: "Incorrect code. Please try again.".into(),
            base_path: state.base_path.clone(),
        }
        .into_response();
    }

    // Success — promote to fully authenticated, clean up OTP
    session.remove::<String>(SESSION_OTP).await.ok();
    session.remove::<i64>(SESSION_OTP_EXP).await.ok();
    session.insert(SESSION_AUTH_STEP, "authenticated").await.ok();
    info!("User authenticated successfully");

    Redirect::to("/").into_response()
}

async fn post_logout(session: Session) -> impl IntoResponse {
    session.delete().await.ok();
    Redirect::to("/login")
}

// ---------------------------------------------------------------------------
// App handlers (all require auth)
// ---------------------------------------------------------------------------

async fn index(State(state): State<Arc<AppState>>, session: Session) -> impl IntoResponse {
    if let Err(r) = require_auth(&session).await { return r.into_response(); }
    IndexTemplate { base_path: state.base_path.clone() }.into_response()
}

async fn api_sessions(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> Result<Json<Vec<SessionSummary>>, (StatusCode, String)> {
    require_auth(&session).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let db = state.db.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lock: {e}"))
    })?;
    let mut stmt = db
        .prepare(
            "SELECT session_id,
                    MIN(user_message) AS first_message,
                    COUNT(*)          AS turn_count,
                    MIN(created_at)   AS started_at
             FROM turns
             GROUP BY session_id
             ORDER BY started_at DESC",
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB prepare: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SessionSummary {
                session_id:    row.get(0)?,
                first_message: row.get(1)?,
                turn_count:    row.get(2)?,
                started_at:    row.get(3)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB query: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB row: {e}")))?;

    Ok(Json(rows))
}

async fn api_session_turns(
    State(state): State<Arc<AppState>>,
    session: Session,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<Vec<TurnRow>>, (StatusCode, String)> {
    require_auth(&session).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let db = state.db.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lock: {e}"))
    })?;
    let mut stmt = db
        .prepare(
            "SELECT turn, user_message, assistant_response, created_at
             FROM turns
             WHERE session_id = ?
             ORDER BY turn ASC",
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB prepare: {e}")))?;

    let rows = stmt
        .query_map([&session_id], |row| {
            Ok(TurnRow {
                turn:               row.get(0)?,
                user_message:       row.get(1)?,
                assistant_response: row.get(2)?,
                created_at:         row.get(3)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB query: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB row: {e}")))?;

    Ok(Json(rows))
}

async fn chat(
    State(state): State<Arc<AppState>>,
    session: Session,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    require_auth(&session).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let override_hdr = headers
        .get("x-http-method-override")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !override_hdr.eq_ignore_ascii_case("GET") {
        return Err((StatusCode::BAD_REQUEST, "X-HTTP-Method-Override: GET header required".into()));
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
        let text = api_resp.text().await.unwrap_or_default();
        return Err((StatusCode::BAD_GATEWAY, format!("Claude API error {status}: {text}")));
    }

    let claude: ClaudeResponse = api_resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to parse Claude response: {e}")))?;

    let content = claude.content.into_iter().next()
        .map(|c| c.text)
        .unwrap_or_else(|| "(no response)".to_string());

    messages.push(Message { role: "assistant".to_string(), content: content.clone() });

    let turn = (messages.len() / 2) as i32;
    let now = Utc::now().to_rfc3339();
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

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "claudia=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let api_key  = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");
    let model    = std::env::var("CLAUDE_MODEL").unwrap_or_else(|_| "claude-opus-4-5".to_string());
    let password = std::env::var("AUTH_PASSWORD").expect("AUTH_PASSWORD must be set");
    let smtp_host = std::env::var("SMTP_HOST").expect("SMTP_HOST must be set");
    let smtp_port = std::env::var("SMTP_PORT").unwrap_or_else(|_| "587".to_string())
        .parse::<u16>().expect("SMTP_PORT must be a number");
    let smtp_user  = std::env::var("SMTP_USER").expect("SMTP_USER must be set");
    let smtp_pass  = std::env::var("SMTP_PASS").expect("SMTP_PASS must be set");
    let auth_email = std::env::var("AUTH_EMAIL").expect("AUTH_EMAIL must be set");
    let app_url    = std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let base_path  = std::env::var("BASE_PATH").unwrap_or_else(|_| String::new());
    let base_path  = base_path.trim_end_matches('/').to_string();
    if !base_path.is_empty() { info!("Base path prefix: {base_path}"); }

    info!("Using model: {model}");
    info!("OTP codes will be sent to: {auth_email}");

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "claudia.duckdb".to_string());
    let db = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("Failed to open DuckDB at {db_path}: {e}"));
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS turns (
            session_id         VARCHAR NOT NULL,
            turn               INTEGER NOT NULL,
            user_message       TEXT    NOT NULL,
            assistant_response TEXT    NOT NULL,
            created_at         VARCHAR NOT NULL
        );",
    )
    .expect("Failed to create turns table");
    info!("DuckDB open at {db_path}");

    let state = Arc::new(AppState {
        api_key, model, password,
        smtp_host, smtp_port, smtp_user, smtp_pass, auth_email, app_url, base_path: base_path.clone(),
        http: reqwest::Client::new(),
        db: Mutex::new(db),
    });

    // Session store (in-memory; survives restarts only as long as the process lives)
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)  // set true if behind HTTPS
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let inner = Router::new()
        // ── auth (public) ──────────────────────────────
        .route("/login",  get(get_login).post(post_login))
        .route("/verify", get(get_verify).post(post_verify))
        .route("/logout", post(post_logout))
        // ── app (protected) ───────────────────────────
        .route("/", get(index))
        .route("/api/sessions", get(api_sessions))
        .route("/api/sessions/{id}", get(api_session_turns))
        .route("/chat", post(chat))
        .with_state(state)
        .layer(session_layer);

    let app = if base_path.is_empty() {
        inner
    } else {
        Router::new().nest(&base_path, inner)
    };

    let addr = "0.0.0.0:3033";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
