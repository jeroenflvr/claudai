#[derive(askama::Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error:     String,
    #[allow(dead_code)]
    pub base_path: String,
    #[allow(dead_code)]
    pub version:   &'static str,
}

#[derive(askama::Template)]
#[template(path = "verify.html")]
pub struct VerifyTemplate {
    pub error:     String,
    #[allow(dead_code)]
    pub base_path: String,
    #[allow(dead_code)]
    pub version:   &'static str,
}

#[derive(askama::Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub base_path: String,
    #[allow(dead_code)]
    pub version:   &'static str,
}

#[derive(askama::Template)]
#[template(path = "response.html")]
pub struct ResponseTemplate {
    pub content:      String,
    pub history_json: String,
}
