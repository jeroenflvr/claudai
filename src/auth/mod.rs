pub mod email;

use crate::{
    state::AppState,
    templates::{LoginTemplate, VerifyTemplate},
};
use askama_axum::IntoResponse;
use axum::{extract::State, response::Redirect, Form};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
use tower_sessions::Session;
use tracing::{info, warn};

pub const SESSION_AUTH_STEP: &str = "auth_step"; // "password_ok" | "authenticated"
pub const SESSION_OTP:       &str = "otp";       // 6-digit code
pub const SESSION_OTP_EXP:   &str = "otp_exp";  // unix timestamp expiry

/// Guard — returns Err(redirect) when the session is not fully authenticated.
pub async fn require_auth(session: &Session, base_path: &str) -> Result<(), Redirect> {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    if step.as_deref() == Some("authenticated") {
        Ok(())
    } else {
        Err(Redirect::to(&format!("{base_path}/login")))
    }
}

// ---------------------------------------------------------------------------
// Form types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginForm {
    pub password: String,
}

#[derive(Deserialize)]
pub struct VerifyForm {
    pub otp: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn get_login(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    if step.as_deref() == Some("authenticated") {
        return Redirect::to(&format!("{}/", state.base_path)).into_response();
    }
    LoginTemplate { error: String::new(), base_path: state.base_path.clone(), version: env!("CARGO_PKG_VERSION") }.into_response()
}

pub async fn post_login(
    State(state): State<Arc<AppState>>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    if form.password != state.password {
        warn!("Failed login attempt");
        return LoginTemplate {
            error: "Incorrect password.".into(),
            base_path: state.base_path.clone(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .into_response();
    }

    let otp = email::generate_otp();
    let exp = Utc::now().timestamp() + 300; // 5 minutes

    session.insert(SESSION_OTP, &otp).await.ok();
    session.insert(SESSION_OTP_EXP, exp).await.ok();
    session.insert(SESSION_AUTH_STEP, "password_ok").await.ok();

    match email::send_otp_email(&state, &otp).await {
        Ok(()) => {
            info!("OTP sent to {}", state.auth_email);
            Redirect::to(&format!("{}/verify", state.base_path)).into_response()
        }
        Err(e) => {
            warn!("Failed to send OTP email: {e}");
            session.delete().await.ok();
            LoginTemplate {
                error: format!("Could not send verification email: {e}"),
                base_path: state.base_path.clone(),
                version: env!("CARGO_PKG_VERSION"),
            }
            .into_response()
        }
    }
}

pub async fn get_verify(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    match step.as_deref() {
        Some("authenticated") => {
            Redirect::to(&format!("{}/", state.base_path)).into_response()
        }
        Some("password_ok") => {
            VerifyTemplate { error: String::new(), base_path: state.base_path.clone(), version: env!("CARGO_PKG_VERSION") }
                .into_response()
        }
        _ => Redirect::to(&format!("{}/login", state.base_path)).into_response(),
    }
}

pub async fn post_verify(
    State(state): State<Arc<AppState>>,
    session: Session,
    Form(form): Form<VerifyForm>,
) -> impl IntoResponse {
    let step: Option<String> = session.get(SESSION_AUTH_STEP).await.unwrap_or(None);
    if step.as_deref() != Some("password_ok") {
        return Redirect::to(&format!("{}/login", state.base_path)).into_response();
    }

    let stored_otp: Option<String> = session.get(SESSION_OTP).await.unwrap_or(None);
    let exp: Option<i64>           = session.get(SESSION_OTP_EXP).await.unwrap_or(None);

    let now     = Utc::now().timestamp();
    let expired = exp.map(|e| now > e).unwrap_or(true);

    if expired {
        session.delete().await.ok();
        return Redirect::to(&format!("{}/login?expired=1", state.base_path)).into_response();
    }

    if stored_otp.as_deref() != Some(form.otp.trim()) {
        warn!("Incorrect OTP attempt");
        return VerifyTemplate {
            error: "Incorrect code. Please try again.".into(),
            base_path: state.base_path.clone(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .into_response();
    }

    session.remove::<String>(SESSION_OTP).await.ok();
    session.remove::<i64>(SESSION_OTP_EXP).await.ok();
    session.insert(SESSION_AUTH_STEP, "authenticated").await.ok();
    info!("User authenticated successfully");

    Redirect::to(&format!("{}/", state.base_path)).into_response()
}

pub async fn post_logout(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    session.delete().await.ok();
    Redirect::to(&format!("{}/login", state.base_path))
}
