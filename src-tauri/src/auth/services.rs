//! Xbox and Minecraft Services exchange. Secrets never cross the IPC boundary.
use crate::errors::{AppError, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Deserialize)]
struct XboxToken {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    claims: XboxClaims,
}
#[derive(Deserialize)]
struct XboxClaims {
    xui: Vec<XboxUser>,
}
#[derive(Deserialize)]
struct XboxUser {
    uhs: String,
}
#[derive(Deserialize)]
struct MinecraftToken {
    access_token: String,
    expires_in: u64,
}
#[derive(Deserialize)]
struct Entitlements {
    items: Vec<Entitlement>,
}
#[derive(Deserialize)]
struct Entitlement {
    name: String,
}
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

pub struct VerifiedAccount {
    pub uuid: String,
    pub name: String,
    pub access_token: String,
    pub expires_at: u64,
    pub user_hash: String,
}

fn malformed() -> AppError {
    AppError::new(
        "AUTH_RESPONSE",
        "An authentication service returned an invalid response.",
    )
}
fn xbox_user(response: &XboxToken) -> Result<&str> {
    if response.token.is_empty()
        || response.claims.xui.len() != 1
        || response.claims.xui[0].uhs.is_empty()
    {
        return Err(malformed());
    }
    Ok(&response.claims.xui[0].uhs)
}
fn owned(entitlements: &Entitlements) -> bool {
    entitlements
        .items
        .iter()
        .any(|item| matches!(item.name.as_str(), "game_minecraft" | "product_minecraft"))
}
fn valid_profile(profile: &Profile) -> bool {
    profile.id.len() == 32
        && profile.id.bytes().all(|b| b.is_ascii_hexdigit())
        && (1..=16).contains(&profile.name.len())
        && profile
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
async fn parse<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
    service: &str,
) -> Result<T> {
    if !response.status().is_success() {
        return Err(AppError::new("AUTH_REJECTED", format!("{service} rejected authentication (HTTP {}). Verify the account and app registration.", response.status())));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        AppError::new(
            "AUTH_NETWORK",
            format!("{service} response was interrupted."),
        )
    })? {
        if body.len() + chunk.len() > 256 * 1024 {
            return Err(malformed());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| malformed())
}
async fn post<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &'static str,
    body: Value,
    service: &str,
) -> Result<T> {
    let response = client.post(url).json(&body).send().await.map_err(|_| {
        AppError::new(
            "AUTH_NETWORK",
            format!("Could not contact {service}. Check your connection."),
        )
    })?;
    parse(response, service).await
}
async fn get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &'static str,
    token: &str,
) -> Result<T> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| {
            AppError::new(
                "AUTH_NETWORK",
                "Could not contact Minecraft Services. Check your connection.",
            )
        })?;
    parse(response, "Minecraft Services").await
}

/// Returns an account only after Xbox authentication, Minecraft ownership and profile checks.
pub async fn verify(microsoft_access_token: &str) -> Result<VerifiedAccount> {
    if microsoft_access_token.is_empty() {
        return Err(malformed());
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| malformed())?;
    let xbox: XboxToken = post(&client, "https://user.auth.xboxlive.com/user/authenticate", json!({
        "Properties": {"AuthMethod":"RPS", "SiteName":"user.auth.xboxlive.com", "RpsTicket": format!("d={microsoft_access_token}")},
        "RelyingParty":"http://auth.xboxlive.com", "TokenType":"JWT"
    }), "Xbox Live").await?;
    let expected_hash = xbox_user(&xbox)?.to_owned();
    let xsts: XboxToken = post(
        &client,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "Properties": {"SandboxId":"RETAIL", "UserTokens":[xbox.token]},
            "RelyingParty":"rp://api.minecraftservices.com/", "TokenType":"JWT"
        }),
        "Xbox XSTS",
    )
    .await?;
    let user_hash = xbox_user(&xsts)?.to_owned();
    if user_hash != expected_hash {
        return Err(malformed());
    }
    let minecraft: MinecraftToken = post(
        &client,
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        json!({
            "identityToken": format!("XBL3.0 x={user_hash};{}", xsts.token)
        }),
        "Minecraft Services",
    )
    .await?;
    if minecraft.access_token.is_empty() || minecraft.expires_in == 0 {
        return Err(malformed());
    }
    let entitlements: Entitlements = get(
        &client,
        "https://api.minecraftservices.com/entitlements/mcstore",
        &minecraft.access_token,
    )
    .await?;
    if !owned(&entitlements) {
        return Err(AppError::new(
            "NO_ENTITLEMENT",
            "This Microsoft account does not have a verified Minecraft Java Edition entitlement.",
        ));
    }
    let profile: Profile = get(
        &client,
        "https://api.minecraftservices.com/minecraft/profile",
        &minecraft.access_token,
    )
    .await?;
    if !valid_profile(&profile) {
        return Err(malformed());
    }
    Ok(VerifiedAccount {
        uuid: profile.id,
        name: profile.name,
        access_token: minecraft.access_token,
        expires_at: crate::models::now().saturating_add(minecraft.expires_in.saturating_sub(60)),
        user_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ownership_and_profile_require_real_minecraft_fields() {
        let none: Entitlements =
            serde_json::from_str(r#"{"items":[{"name":"game_other"}]}"#).unwrap();
        let yes: Entitlements =
            serde_json::from_str(r#"{"items":[{"name":"game_minecraft"}]}"#).unwrap();
        assert!(!owned(&none));
        assert!(owned(&yes));
        assert!(valid_profile(&Profile {
            id: "0123456789abcdef0123456789abcdef".into(),
            name: "Player_1".into()
        }));
        assert!(!valid_profile(&Profile {
            id: "invalid".into(),
            name: "Player_1".into()
        }));
        assert!(!valid_profile(&Profile {
            id: "0123456789abcdef0123456789abcdef".into(),
            name: "bad name".into()
        }));
    }
    #[test]
    fn xbox_response_must_contain_exactly_one_identity() {
        let empty: XboxToken =
            serde_json::from_str(r#"{"Token":"token","DisplayClaims":{"xui":[]}}"#).unwrap();
        assert!(xbox_user(&empty).is_err());
        let multiple: XboxToken = serde_json::from_str(
            r#"{"Token":"token","DisplayClaims":{"xui":[{"uhs":"one"},{"uhs":"two"}]}}"#,
        )
        .unwrap();
        assert!(xbox_user(&multiple).is_err());
    }
}
