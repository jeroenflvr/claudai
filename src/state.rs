use crate::config::Config;
use duckdb::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub api_key:    String,
    pub model:      String,
    pub http:       reqwest::Client,
    pub db:         Mutex<Connection>,
    pub password:   String,
    pub smtp_host:  String,
    pub smtp_port:  u16,
    pub smtp_user:  String,
    pub smtp_pass:  String,
    pub auth_email: String,
    pub base_path:  String,
}

impl AppState {
    pub fn new(config: Config, db: Connection) -> Self {
        Self {
            http:       reqwest::Client::new(),
            db:         Mutex::new(db),
            api_key:    config.api_key,
            model:      config.model,
            password:   config.password,
            smtp_host:  config.smtp_host,
            smtp_port:  config.smtp_port,
            smtp_user:  config.smtp_user,
            smtp_pass:  config.smtp_pass,
            auth_email: config.auth_email,
            base_path:  config.base_path,
        }
    }
}
