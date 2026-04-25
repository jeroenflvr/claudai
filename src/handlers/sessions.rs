use crate::{
    auth::require_auth,
    models::{SessionSummary, TurnRow},
    state::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;
use tower_sessions::Session;

pub async fn api_sessions(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> Result<Json<Vec<SessionSummary>>, (StatusCode, String)> {
    require_auth(&session, &state.base_path).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let db = state.db.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lock: {e}")))?;

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

pub async fn api_session_turns(
    State(state): State<Arc<AppState>>,
    session: Session,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<Vec<TurnRow>>, (StatusCode, String)> {
    require_auth(&session, &state.base_path).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Not authenticated".into()))?;

    let db = state.db.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lock: {e}")))?;

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
