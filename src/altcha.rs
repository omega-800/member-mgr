//! Altcha anti-bot challenge: issues challenges at `/challenge` and verifies
//! the `altchaToken` field on `/submit`.

use std::{collections::HashMap, error::Error};

use altcha::{
    CreateChallengeOptions, Payload, VerifySolutionOptions, create_challenge, verify_solution,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use tiny_http::{Request, Response};

use crate::{Config, SubmitErr};

/// Responds to `/challenge` with a new Altcha challenge as JSON.
pub fn get_challenge(request: Request, config: &Config) -> Result<(), Box<dyn Error>> {
    let challenge = create_challenge(CreateChallengeOptions {
        algorithm: "PBKDF2/SHA-256".to_string(),
        cost: 5_000,
        counter: Some(rand::random_range(5_000..=10_000)),
        expires_at: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 600,
        ),
        hmac_signature_secret: Some((config.hmac_secret).clone()),
        hmac_key_signature_secret: Some((config.hmac_key_secret).clone()),
        ..Default::default()
    })?;

    request.respond(
        Response::from_string(serde_json::to_string(&challenge)?).with_header(
            tiny_http::Header::from_bytes(b"Content-Type", b"application/json")
                .map_err(|_| "invalid HTTP header")?,
        ),
    )?;

    Ok(())
}

struct AltchaResult {
    verified: bool,
    expired: bool,
    invalid_signature: Option<bool>,
}

/// Verifies and removes the `altchaToken` field from the submitted form.
///
/// # Errors
///
/// Returns a [`SubmitErr`] with a 400 if no token was provided, or a 500 if the
/// token cannot be decoded or its solution fails verification.
pub fn post_submit(
    config: &Config,
    fields: &mut HashMap<String, String>,
) -> Result<(), SubmitErr> {
    let secret = (config.hmac_secret).as_str();

    let Some(token) = fields.remove("altchaToken") else {
        return Err(SubmitErr::new(
            400,
            "Invalid form data: No altchaToken provided".to_string(),
            "Invalid form data".to_string(),
        ));
    };

    let Ok(bytes) = BASE64.decode(token) else {
        return Err(SubmitErr::new(
            500,
            "Invalid token: base64 decode failed".to_string(),
            "Invalid token".to_string(),
        ));
    };

    let altcha_result = match serde_json::from_slice::<Payload>(&bytes) {
        Ok(payload) => {
            match verify_solution(VerifySolutionOptions {
                hmac_key_signature_secret: Some((config.hmac_key_secret).clone()),
                ..VerifySolutionOptions::new(&payload.challenge, &payload.solution, secret)
            }) {
                Ok(r) => AltchaResult {
                    verified: r.verified,
                    expired: r.expired,
                    invalid_signature: r.invalid_signature,
                },
                Err(err) => {
                    return Err(SubmitErr::new(
                        500,
                        format!("Invalid token: {err}"),
                        "Invalid token".to_string(),
                    ));
                }
            }
        }
        Err(_) => {
            return Err(SubmitErr::new(
                500,
                "Altcha: unrecognised payload format".to_string(),
                "Unrecognised payload format".to_string(),
            ));
        }
    };

    if !altcha_result.verified {
        let reason = if altcha_result.expired {
            "challenge has expired"
        } else if altcha_result.invalid_signature == Some(true) {
            "invalid signature"
        } else {
            "invalid solution"
        };
        return Err(SubmitErr::new(
            500,
            format!("Altcha: {reason}"),
            format!("Altcha: {reason}"),
        ));
    }

    Ok(())
}
