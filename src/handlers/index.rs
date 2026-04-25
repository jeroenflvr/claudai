use crate::{auth::require_auth, state::AppState, templates::IndexTemplate};
use askama_axum::IntoResponse;
use axum::extract::State;
use std::sync::Arc;
use tower_sessions::Session;

pub async fn index(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    if let Err(r) = require_auth(&session, &state.base_path).await {
        return r.into_response();
    }
    IndexTemplate { base_path: state.base_path.clone(), version: env!("CARGO_PKG_VERSION") }.into_response()
}
