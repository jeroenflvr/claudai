use tracing::info;

pub struct Config {
    pub api_key:    String,
    pub model:      String,
    pub password:   String,
    pub smtp_host:  String,
    pub smtp_port:  u16,
    pub smtp_user:  String,
    pub smtp_pass:  String,
    pub auth_email: String,
    pub base_path:  String,
    pub db_path:    String,
    pub port:       u16,
}

impl Config {
    pub fn from_env() -> Self {
        let base_path = std::env::var("BASE_PATH")
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string();
        if !base_path.is_empty() {
            info!("Base path prefix: {base_path}");
        }

        Config {
            api_key: std::env::var("ANTHROPIC_API_KEY")
                .expect("ANTHROPIC_API_KEY must be set"),
            model: std::env::var("CLAUDE_MODEL")
                .unwrap_or_else(|_| "claude-opus-4-5".to_string()),
            password: std::env::var("AUTH_PASSWORD")
                .expect("AUTH_PASSWORD must be set"),
            smtp_host: std::env::var("SMTP_HOST")
                .expect("SMTP_HOST must be set"),
            smtp_port: std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse::<u16>()
                .expect("SMTP_PORT must be a number"),
            smtp_user: std::env::var("SMTP_USER")
                .expect("SMTP_USER must be set"),
            smtp_pass: std::env::var("SMTP_PASS")
                .expect("SMTP_PASS must be set"),
            auth_email: std::env::var("AUTH_EMAIL")
                .expect("AUTH_EMAIL must be set"),
            base_path,
            db_path: std::env::var("DB_PATH")
                .unwrap_or_else(|_| "claudia.duckdb".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse::<u16>()
                .expect("PORT must be a number"),
        }
    }
}
