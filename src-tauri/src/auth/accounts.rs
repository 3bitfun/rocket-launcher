use super::{browser, credential_store, microsoft, services, verified_cache::VerifiedCache};
use crate::{
    errors::{AppError, Result},
    minecraft::launch::ValidatedSession,
    models::now,
    storage::atomic_json,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const DEFAULT_MICROSOFT_CLIENT_ID: &str = "49a85b9b-5cd0-46be-a44a-6bd45ac70db4";

#[derive(Default)]
pub struct AccountJobs(pub tokio::sync::Mutex<()>);

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub account_type: AccountType,
    client_id: String,
    #[serde(default)]
    appearance: Appearance,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub skin: Option<String>,
    pub cape: Option<String>,
    pub slim: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    #[default]
    Microsoft,
    LocalTest,
    Offline,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accounts {
    pub profiles: Vec<Account>,
    pub active_id: Option<String>,
    pub offline: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsSummary {
    pub profiles: Vec<AccountSummaryProfile>,
    pub active_id: Option<String>,
    pub offline: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummaryProfile {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    pub appearance: Appearance,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccountMode {
    Microsoft { offline: bool },
    LocalTest,
    Offline,
}

impl Accounts {
    pub fn read(data: &Path) -> Result<Self> {
        match fs::read(data.join("accounts.json")) {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => {
                serde_json::from_slice(&bytes).map_err(|_| storage())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            _ => Err(storage()),
        }
    }

    fn save(&self, data: &Path) -> Result<()> {
        if serde_json::to_vec_pretty(self).map_err(|_| storage())?.len() >= 1024 * 1024 {
            return Err(AppError::new("AUTH_STORAGE", "Saved profiles are full. Remove an unused skin before saving."));
        }
        atomic_json(&data.join("accounts.json"), self)
    }

    fn active(&self) -> Result<&Account> {
        self.profiles
            .iter()
            .find(|profile| {
                Some(&profile.id) == self.active_id.as_ref()
                    && (cfg!(debug_assertions) || profile.account_type != AccountType::LocalTest)
            })
            .ok_or_else(crate::minecraft::launch::authentication_required)
    }

    pub fn summary(&self) -> AccountsSummary {
        let profiles = self
            .profiles
            .iter()
            .filter(|profile| {
                cfg!(debug_assertions) || profile.account_type != AccountType::LocalTest
            })
            .map(|profile| AccountSummaryProfile {
                id: profile.id.clone(),
                name: profile.name.clone(),
                account_type: profile.account_type,
                appearance: profile.appearance.clone(),
            })
            .collect::<Vec<_>>();
        let active_id = self
            .active_id
            .as_ref()
            .filter(|id| profiles.iter().any(|profile| &profile.id == *id))
            .cloned();
        AccountsSummary {
            profiles,
            active_id,
            offline: self.offline,
        }
    }
}

fn storage() -> AppError {
    AppError::new("AUTH_STORAGE", "Could not read saved launcher accounts.")
}
pub fn busy() -> AppError {
    AppError::new("AUTH_BUSY", "Another account operation is in progress.")
}
pub fn offline_error() -> AppError {
    AppError::new(
        "OFFLINE_MODE",
        "Turn off Offline Mode in Settings to use this feature.",
    )
}
fn refresh_key(id: &str) -> String {
    format!("refresh:{id}")
}

pub fn select(data: &Path, id: String) -> Result<Accounts> {
    let mut accounts = Accounts::read(data)?;
    let profile = accounts
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(crate::minecraft::launch::authentication_required)?;
    if profile.account_type == AccountType::LocalTest {
        if !cfg!(debug_assertions) {
            return Err(crate::minecraft::launch::authentication_required());
        }
        accounts.offline = false;
    } else if profile.account_type == AccountType::Microsoft && VerifiedCache::load(&id)?.is_none() {
        return Err(crate::minecraft::launch::authentication_required());
    }
    accounts.active_id = Some(id);
    accounts.save(data)?;
    Ok(accounts)
}

fn offline_uuid(name: &str) -> Result<String> {
    use md5::{Digest, Md5};
    let name = name.trim();
    if !(3..=16).contains(&name.len())
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(AppError::new("VALIDATION", "Username must be 3–16 letters, numbers, or underscores."));
    }
    let mut digest: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    Ok(uuid::Uuid::from_bytes(digest).simple().to_string())
}

pub fn create_offline_account(data: &Path, name: String) -> Result<Accounts> {
    let name = name.trim();
    let id = offline_uuid(name)?;
    let mut accounts = Accounts::read(data)?;
    if let Some(existing) = accounts.profiles.iter().find(|profile| profile.id == id) {
        if existing.account_type != AccountType::Offline {
            return Err(AppError::new("VALIDATION", "An account with that identity already exists."));
        }
    } else {
        accounts.profiles.push(Account { id: id.clone(), name: name.into(), account_type: AccountType::Offline, client_id: String::new(), appearance: Appearance::default() });
    }
    accounts.active_id = Some(id);
    accounts.save(data)?;
    Ok(accounts)
}

pub fn edit_offline(data: &Path, id: String, name: String, skin: Option<String>, slim: bool) -> Result<Accounts> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let name = name.trim();
    let new_id = offline_uuid(name)?;
    if let Some(skin) = &skin {
        let invalid = || AppError::new("VALIDATION", "Choose a 64×64 or 64×32 PNG skin under 64 KiB.");
        let encoded = skin.strip_prefix("data:image/png;base64,").ok_or_else(invalid)?;
        if encoded.len() > 87_384 { return Err(invalid()); }
        let bytes = STANDARD.decode(encoded).map_err(|_| invalid())?;
        if bytes.len() < 33 || bytes.len() > 65_536
            || &bytes[..16] != b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR"
            || bytes[16..20] != [0, 0, 0, 64]
            || !(bytes[20..24] == [0, 0, 0, 64] || bytes[20..24] == [0, 0, 0, 32])
        { return Err(invalid()); }
    }
    let mut accounts = Accounts::read(data)?;
    if new_id != id && accounts.profiles.iter().any(|p| p.id == new_id) {
        return Err(AppError::new("VALIDATION", "An account with that username already exists."));
    }
    let profile = accounts.profiles.iter_mut().find(|p| p.id == id && p.account_type == AccountType::Offline)
        .ok_or_else(crate::minecraft::launch::authentication_required)?;
    profile.name = name.into();
    profile.id = new_id.clone();
    profile.appearance = Appearance { skin, slim, cape: None };
    if accounts.active_id.as_deref() == Some(id.as_str()) {
        accounts.active_id = Some(new_id);
    }
    accounts.save(data)?;
    Ok(accounts)
}

pub async fn profile_appearance(data: &Path, id: &str) -> Result<Appearance> {
    let accounts = Accounts::read(data)?;
    let profile = accounts.profiles.iter().find(|profile| profile.id == id && profile.account_type == AccountType::Microsoft)
        .ok_or_else(crate::minecraft::launch::authentication_required)?;
    if accounts.offline { return Ok(profile.appearance.clone()); }
    let Some(cache) = VerifiedCache::load(id)? else { return Ok(profile.appearance.clone()); };
    if cache.minecraft_expires_at <= now() { return Ok(profile.appearance.clone()); }
    services::profile_appearance(&cache.minecraft_token, id).await.or_else(|_| Ok(profile.appearance.clone()))
}

#[cfg(debug_assertions)]
pub fn create_test_account(data: &Path) -> Result<Accounts> {
    let mut accounts = Accounts::read(data)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    accounts.profiles.push(Account {
        id: id.clone(),
        name: "RocketTester".into(),
        account_type: AccountType::LocalTest,
        client_id: String::new(),
        appearance: Appearance::default(),
    });
    accounts.active_id = Some(id);
    accounts.offline = false;
    accounts.save(data)?;
    Ok(accounts)
}

pub fn set_offline(data: &Path, offline: bool) -> Result<Accounts> {
    let mut accounts = Accounts::read(data)?;
    if offline {
        let active = accounts.active()?;
        if active.account_type != AccountType::Offline && VerifiedCache::load(&active.id)?.is_none() {
            return Err(crate::minecraft::launch::authentication_required());
        }
    }
    accounts.offline = offline;
    accounts.save(data)?;
    Ok(accounts)
}

pub fn sign_out(data: &Path, id: String) -> Result<Accounts> {
    let mut accounts = Accounts::read(data)?;
    if !accounts.profiles.iter().any(|p| p.id == id) {
        return Err(crate::minecraft::launch::authentication_required());
    }
    if accounts.profiles.iter().any(|p| p.id == id && p.account_type == AccountType::Microsoft) {
        credential_store::delete(&id)?;
        credential_store::delete(&refresh_key(&id))?;
    }
    accounts.profiles.retain(|p| p.id != id);
    if accounts.active_id.as_ref() == Some(&id) {
        accounts.active_id = accounts.profiles.first().map(|p| p.id.clone());
        accounts.offline = false;
    }
    accounts.save(data)?;
    Ok(accounts)
}

pub async fn sign_in(data: &Path) -> Result<Accounts> {
    let mut accounts = Accounts::read(data)?;
    if accounts.offline {
        return Err(offline_error());
    }
    let client_id = std::env::var("ROCKET_MICROSOFT_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("ROCKET_MICROSOFT_CLIENT_ID").map(str::to_owned))
        .unwrap_or_else(|| DEFAULT_MICROSOFT_CLIENT_ID.to_owned());
    let client_id = microsoft::client_id(client_id.trim())?;
    let tokens = browser::sign_in(client_id).await?;
    let verified = services::verify(&tokens.access_token).await?;
    let appearance = verified.appearance.clone();
    let cache = VerifiedCache::from_verified(verified, now())?;
    let id = cache.uuid.clone();
    credential_store::write(&refresh_key(&id), tokens.refresh_token.as_bytes())?;
    cache.store()?;
    accounts.profiles.retain(|p| p.id != id);
    accounts.profiles.push(Account {
        id: id.clone(),
        name: cache.name,
        account_type: AccountType::Microsoft,
        client_id: client_id.into(),
        appearance,
    });
    accounts.active_id = Some(id);
    accounts.save(data)?;
    Ok(accounts)
}

pub fn readiness(data: &Path) -> Result<AccountMode> {
    let accounts = Accounts::read(data)?;
    let active = accounts.active()?;
    if active.account_type == AccountType::Offline {
        return Ok(AccountMode::Offline);
    }
    if active.account_type == AccountType::LocalTest {
        return if cfg!(debug_assertions) {
            Ok(AccountMode::LocalTest)
        } else {
            Err(crate::minecraft::launch::authentication_required())
        };
    }
    if VerifiedCache::load(&active.id)?.is_none() {
        return Err(crate::minecraft::launch::authentication_required());
    }
    Ok(AccountMode::Microsoft {
        offline: accounts.offline,
    })
}

pub fn offline_identity(data: &Path) -> Result<(String, String)> {
    let accounts = Accounts::read(data)?;
    let active = accounts.active()?;
    if active.account_type != AccountType::Offline {
        return Err(crate::minecraft::launch::authentication_required());
    }
    Ok((active.name.clone(), active.id.clone()))
}

#[cfg(debug_assertions)]
pub fn local_test_identity(data: &Path) -> Result<Option<(String, String)>> {
    let accounts = Accounts::read(data)?;
    let active = accounts.active()?;
    if active.account_type != AccountType::LocalTest {
        return Ok(None);
    }
    if !cfg!(debug_assertions) {
        return Err(crate::minecraft::launch::authentication_required());
    }
    Ok(Some((active.name.clone(), active.id.clone())))
}

fn session(cache: VerifiedCache, client_id: String, offline: bool) -> ValidatedSession {
    ValidatedSession {
        player: cache.name,
        uuid: cache.uuid,
        access_token: if offline {
            String::new()
        } else {
            cache.minecraft_token
        },
        // The Xbox user hash is not an XUID. Do not pass it in that argument.
        xuid: String::new(),
        client_id: if offline { String::new() } else { client_id },
        expires_at: cache.minecraft_expires_at,
        offline,
    }
}

pub async fn launch_session(data: &Path) -> Result<ValidatedSession> {
    let mut accounts = Accounts::read(data)?;
    let active = accounts.active()?.clone();
    if active.account_type != AccountType::Microsoft {
        return Err(crate::minecraft::launch::authentication_required());
    }
    let cache = VerifiedCache::load(&active.id)?
        .ok_or_else(crate::minecraft::launch::authentication_required)?;
    if accounts.offline {
        return Ok(session(cache, active.client_id, true));
    }
    let refreshed = async {
        if cache.minecraft_expires_at > now().saturating_add(30) {
            match services::check_profile(&cache.minecraft_token, &cache.uuid).await {
                Ok(()) => return Ok(None),
                Err(error) if error.code == "AUTH_EXPIRED" => {}
                Err(error) => return Err(error),
            }
        }
        let refresh = credential_store::read(&refresh_key(&active.id))?
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or_else(crate::minecraft::launch::authentication_required)?;
        let tokens = microsoft::refresh(&active.client_id, &refresh).await?;
        let verified = services::verify(&tokens.access_token).await?;
        if !verified.uuid.eq_ignore_ascii_case(&active.id) {
            return Err(AppError::new(
                "AUTH_REJECTED",
                "The refreshed Minecraft account does not match the selected account.",
            ));
        }
        let cache = VerifiedCache::from_verified(verified, now())?;
        credential_store::write(&refresh_key(&active.id), tokens.refresh_token.as_bytes())?;
        cache.store()?;
        if let Some(profile) = accounts
            .profiles
            .iter_mut()
            .find(|profile| profile.id == active.id)
        {
            profile.name.clone_from(&cache.name);
        }
        accounts.save(data)?;
        Ok(Some(cache))
    }
    .await;
    match refreshed {
        Ok(new_cache) => Ok(session(new_cache.unwrap_or(cache), active.client_id, false)),
        Err(error) if error.code == "AUTH_NETWORK" => Ok(session(cache, active.client_id, true)),
        Err(error) if error.code == "NO_ENTITLEMENT" => {
            credential_store::delete(&active.id)?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    #[test]
    fn offline_profiles_have_stable_minecraft_identity_and_need_no_login() {
        let directory = tempfile::tempdir().unwrap();
        let accounts = create_offline_account(directory.path(), "Notch".into()).unwrap();
        let profile = &accounts.profiles[0];
        assert_eq!(profile.id, "b50ad385829d3141a2167e7d7539ba7f");
        assert_eq!(offline_identity(directory.path()).unwrap(), ("Notch".into(), profile.id.clone()));
        assert!(matches!(readiness(directory.path()), Ok(AccountMode::Offline)));
        assert_eq!(create_offline_account(directory.path(), "Notch".into()).unwrap().profiles.len(), 1);
        assert!(set_offline(directory.path(), true).unwrap().offline);
        assert!(create_offline_account(directory.path(), "Alex".into()).unwrap().offline);
        assert!(select(directory.path(), profile.id.clone()).unwrap().offline);
        assert!(create_offline_account(directory.path(), "bad name".into()).is_err());
        assert!(matches!(readiness(directory.path()), Ok(AccountMode::Offline)));
    }

    #[test]
    fn local_test_accounts_are_persisted_unverified_and_selectable() {
        let directory = tempfile::tempdir().unwrap();
        let accounts = create_test_account(directory.path()).unwrap();
        let profile = &accounts.profiles[0];
        let id = profile.id.clone();

        assert!(uuid::Uuid::parse_str(&id).is_ok());
        assert_eq!(profile.name, "RocketTester");
        assert_eq!(profile.account_type, AccountType::LocalTest);
        assert!(profile.client_id.is_empty());
        assert!(VerifiedCache::load(&id).unwrap().is_none());
        assert!(matches!(
            readiness(directory.path()),
            Ok(AccountMode::LocalTest)
        ));

        let reloaded = Accounts::read(directory.path()).unwrap();
        assert_eq!(reloaded.profiles[0].account_type, AccountType::LocalTest);
        let selected = select(directory.path(), id.clone()).unwrap();
        assert_eq!(selected.active_id.as_deref(), Some(id.as_str()));
    }
}
