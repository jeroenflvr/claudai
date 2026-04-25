use crate::state::AppState;
use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message as EmailMessage, Tokio1Executor,
};
use rand::Rng;

pub fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..=999999))
}

pub async fn send_otp_email(state: &AppState, otp: &str) -> Result<(), String> {
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
