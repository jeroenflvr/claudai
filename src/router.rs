use crate::{auth, handlers, state::AppState};
use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tower_sessions::{MemoryStore, SessionManagerLayer};

pub fn build(state: Arc<AppState>) -> Router {
    let p = state.base_path.clone();

    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_secure(false)
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    Router::new()
        //  auth (public) 
        .route(&format!("{p}/login"),              get(auth::get_login).post(auth::post_login))
        .route(&format!("{p}/verify"),             get(auth::get_verify).post(auth::post_verify))
        .route(&format!("{p}/logout"),             post(auth::post_logout))
        //  app (protected) 
        .route(&format!("{p}/"),                   get(handlers::index::index))
        .route(&format!("{p}/api/sessions"),       get(handlers::sessions::api_sessions))
        .route(&format!("{p}/api/sessions/{{id}}"),get(handlers::sessions::api_session_turns))
        .route(&format!("{p}/chat"),               post(handlers::chat::chat))
        //  static assets (embedded) 
        .route(&format!("{p}/static/css/app.css"),    get(handlers::static_files::app_css))
        .route(&format!("{p}/static/css/auth.css"),   get(handlers::static_files::auth_css))
        .route(&format!("{p}/static/js/app.js"),      get(handlers::static_files::app_js))
        .with_state(state)
        .layer(session_layer)
}
