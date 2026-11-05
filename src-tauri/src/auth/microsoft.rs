//! Microsoft public-client OAuth primitives. Tokens remain in native memory.
use super::pkce::AuthorizationAttempt;
use crate::errors::{AppError, Result};
use serde::Deserialize;
use std::time::Duration;

const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPES: &str = "XboxLive.signin offline_access";

#[derive(Deserialize)]
pub struct MicrosoftTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

pub fn client_id(value: &str) -> Result<&str> {
    uuid::Uuid::parse_str(value).map_err(|_| {
        AppError::new(
            "AUTH_CONFIGURATION",
            "Configure a valid Microsoft application client ID.",
        )
    })?;
    Ok(value)
}

fn callback(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AppError::new("AUTH_CONFIGURATION", "Invalid local redirect URI."))?;
    if url.scheme() != "http"
        || url.host_str() != Some("localhost")
        || url.port().is_none()
        || url.path() != "/callback"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::new(
            "AUTH_CONFIGURATION",
            "The redirect must be a localhost callback with an explicit port.",
        ));
    }
    Ok(url)
}

pub fn authorization_url(
    id: &str,
    redirect: &str,
    attempt: &AuthorizationAttempt,
) -> Result<reqwest::Url> {
    let id = client_id(id)?;
    callback(redirect)?;
    let mut url =
        reqwest::Url::parse(&format!("{AUTHORITY}/authorize")).expect("fixed authority URL");
    url.query_pairs_mut()
        .append_pair("client_id", id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPES)
        .append_pair("prompt", "select_account")
        .append_pair("state", attempt.state())
        .append_pair("code_challenge", &attempt.challenge())
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| {
            AppError::new(
                "AUTH_NETWORK",
                "Could not initialize the Microsoft authentication client.",
            )
        })
}

async fn token_request(form: &[(&str, &str)]) -> Result<MicrosoftTokens> {
    let mut response = client()?
        .post(format!("{AUTHORITY}/token"))
        .form(form)
        .send()
        .await
        .map_err(|_| {
            AppError::new(
                "AUTH_NETWORK",
                "Could not contact Microsoft. Check your connection and try again.",
            )
        })?;
    if !response.status().is_success() {
        // Never expose response bodies: they can contain credentials or personal data.
        return Err(AppError::new("AUTH_REJECTED", format!("Microsoft rejected authentication (HTTP {}). Sign in again or check the app registration.", response.status())));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        AppError::new(
            "AUTH_NETWORK",
            "Microsoft's authentication response was interrupted.",
        )
    })? {
        if body.len() + chunk.len() > 64 * 1024 {
            return Err(AppError::new(
                "AUTH_RESPONSE",
                "Microsoft's authentication response was too large.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let tokens: MicrosoftTokens = serde_json::from_slice(&body).map_err(|_| {
        AppError::new(
            "AUTH_RESPONSE",
            "Microsoft returned an incomplete authentication response.",
        )
    })?;
    if tokens.access_token.is_empty() || tokens.refresh_token.is_empty() || tokens.expires_in == 0 {
        return Err(AppError::new(
            "AUTH_RESPONSE",
            "Microsoft returned an incomplete authentication response.",
        ));
    }
    Ok(tokens)
}

pub async fn redeem(
    id: &str,
    redirect: &str,
    code: &str,
    attempt: &AuthorizationAttempt,
) -> Result<MicrosoftTokens> {
    let id = client_id(id)?;
    callback(redirect)?;
    if code.is_empty() || code.len() > 8192 {
        return Err(AppError::new(
            "AUTH_RESPONSE",
            "Invalid authorization code.",
        ));
    }
    token_request(&[
        ("client_id", id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect),
        ("code_verifier", attempt.verifier()),
        ("scope", SCOPES),
    ])
    .await
}

pub async fn refresh(id: &str, refresh_token: &str) -> Result<MicrosoftTokens> {
    let id = client_id(id)?;
    if refresh_token.is_empty() {
        return Err(AppError::new(
            "AUTH_REQUIRED",
            "Sign in to Microsoft again.",
        ));
    }
    token_request(&[
        ("client_id", id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", SCOPES),
    ])
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorization_uses_pkce_and_rejects_untrusted_redirects() {
        let attempt = AuthorizationAttempt::new();
        let id = "12345678-1234-4234-8234-123456789abc";
        let url = authorization_url(id, "http://localhost:4123/callback", &attempt).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["code_challenge"], attempt.challenge());
        assert_eq!(query["state"], attempt.state());
        assert!(!url.as_str().contains(attempt.verifier()));
        assert!(authorization_url(id, "http://evil.example/callback", &attempt).is_err());
        assert!(authorization_url("invalid", "http://localhost:4123/callback", &attempt).is_err());
    }
}
