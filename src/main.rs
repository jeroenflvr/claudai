mod auth;
mod config;
mod db;
mod handlers;
mod models;
mod router;
mod state;
mod templates;

use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "claudia=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let config = config::Config::from_env();
    info!("Using model: {}", config.model);
    info!("OTP codes will be sent to: {}", config.auth_email);

    let port  = config.port;
    let db    = db::init(&config.db_path);
    let state = Arc::new(state::AppState::new(config, db));
    let app   = router::build(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("Listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
