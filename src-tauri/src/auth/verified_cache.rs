//! Offline eligibility is derived from an authenticated, entitlement-checked record.
//! The record is intended for OS credential storage, never plain configuration.
use crate::{
    auth::services::VerifiedAccount,
    errors::{AppError, Result},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VerifiedCache {
    schema: u32,
    pub uuid: String,
    pub name: String,
    pub minecraft_token: String,
    pub minecraft_expires_at: u64,
    pub user_hash: String,
    pub verified_at: u64,
}

impl VerifiedCache {
    pub fn from_verified(account: VerifiedAccount, verified_at: u64) -> Result<Self> {
        let cache = Self {
            schema: 1,
            uuid: account.uuid,
            name: account.name,
            minecraft_token: account.access_token,
            minecraft_expires_at: account.expires_at,
            user_hash: account.user_hash,
            verified_at,
        };
        cache.validate()?;
        Ok(cache)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1
            || self.verified_at == 0
            || self.minecraft_expires_at == 0
            || self.uuid.len() != 32
            || !self.uuid.bytes().all(|b| b.is_ascii_hexdigit())
            || !(1..=16).contains(&self.name.len())
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self.minecraft_token.is_empty()
            || self.user_hash.is_empty()
        {
            return Err(AppError::new(
                "AUTH_STORAGE",
                "The verified account record is invalid. Sign in again.",
            ));
        }
        Ok(())
    }
    #[cfg(windows)]
    pub fn store(&self) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|_| AppError::new("AUTH_STORAGE", "Could not encode the account record."))?;
        super::credential_store::write(&self.uuid, &bytes)
    }
    #[cfg(windows)]
    pub fn load(id: &str) -> Result<Option<Self>> {
        let Some(bytes) = super::credential_store::read(id)? else {
            return Ok(None);
        };
        let cache: Self = serde_json::from_slice(&bytes).map_err(|_| {
            AppError::new(
                "AUTH_STORAGE",
                "The account record is damaged. Sign in again.",
            )
        })?;
        cache.validate()?;
        let expected = uuid::Uuid::parse_str(id)
            .map_err(|_| AppError::new("AUTH_STORAGE", "Invalid account identifier."))?;
        let actual = uuid::Uuid::parse_str(&cache.uuid)
            .map_err(|_| AppError::new("AUTH_STORAGE", "Invalid account identifier."))?;
        if expected != actual {
            return Err(AppError::new(
                "AUTH_STORAGE",
                "Account identity mismatch. Sign in again.",
            ));
        }
        Ok(Some(cache))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unverified_or_damaged_records() {
        let mut account = VerifiedCache {
            schema: 1,
            uuid: "0123456789abcdef0123456789abcdef".into(),
            name: "Player".into(),
            minecraft_token: "previously-issued-token".into(),
            minecraft_expires_at: 123,
            user_hash: "hash".into(),
            verified_at: 1,
        };
        assert!(account.validate().is_ok());
        account.verified_at = 0;
        assert!(account.validate().is_err());
        account.verified_at = 1;
        account.minecraft_token.clear();
        assert!(account.validate().is_err());
    }
}
