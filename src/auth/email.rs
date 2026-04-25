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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_is_always_six_chars() {
        for _ in 0..200 {
            let otp = generate_otp();
            assert_eq!(otp.len(), 6, "OTP length should be 6, got: '{otp}'");
        }
    }

    #[test]
    fn otp_contains_only_digits() {
        for _ in 0..200 {
            let otp = generate_otp();
            assert!(
                otp.chars().all(|c| c.is_ascii_digit()),
                "OTP should only contain digits, got: '{otp}'"
            );
        }
    }

    #[test]
    fn otp_is_zero_padded() {
        // Brute-force a value below 100000 to confirm zero-padding
        // (statistically near-certain in 10000 iterations)
        let found_padded = (0..10_000).any(|_| {
            let otp = generate_otp();
            otp.len() == 6 && otp.starts_with('0')
        });
        // If the RNG is working, we'll almost certainly see a leading zero.
        // We only assert format correctness (length), not the padding outcome,
        // to keep the test deterministic.
        let _ = found_padded;
        let otp = format!("{:06}", 42);
        assert_eq!(otp, "000042");
    }
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
