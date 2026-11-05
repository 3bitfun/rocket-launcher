//! Windows Credential Manager storage, scoped to Rocket Launcher account IDs.
use crate::errors::{AppError, Result};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr};
use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_NOT_FOUND},
    Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    },
};

fn target(id: &str) -> Result<Vec<u16>> {
    let id = uuid::Uuid::parse_str(id)
        .map_err(|_| AppError::new("AUTH_STORAGE", "Invalid account identifier."))?;
    Ok(OsStr::new(&format!("RocketLauncher/Minecraft/{id}"))
        .encode_wide()
        .chain(Some(0))
        .collect())
}

fn storage_error(action: &str) -> AppError {
    AppError::new("AUTH_STORAGE", format!("Could not {action} in Windows Credential Manager. Check your Windows account and try again."))
}

pub fn write(id: &str, secret: &[u8]) -> Result<()> {
    if secret.is_empty() || secret.len() > 2560 {
        return Err(AppError::new("AUTH_STORAGE", "Credential size is invalid."));
    }
    let mut name = target(id)?;
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: name.as_mut_ptr(),
        CredentialBlobSize: secret.len() as u32,
        CredentialBlob: secret.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    // Windows copies the provided bytes before the function returns.
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        return Err(storage_error("save the account credential"));
    }
    Ok(())
}

pub fn read(id: &str) -> Result<Option<Vec<u8>>> {
    let name = target(id)?;
    let mut credential: *mut CREDENTIALW = ptr::null_mut();
    if unsafe { CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
        if unsafe { GetLastError() } == ERROR_NOT_FOUND {
            return Ok(None);
        }
        return Err(storage_error("read the account credential"));
    }
    // CredReadW returns an allocated credential whose lifetime ends at CredFree.
    let result = unsafe {
        let credential_ref = &*credential;
        let size = credential_ref.CredentialBlobSize as usize;
        let bytes = if size > 0 && size <= 2560 && !credential_ref.CredentialBlob.is_null() {
            Some(std::slice::from_raw_parts(credential_ref.CredentialBlob, size).to_vec())
        } else {
            None
        };
        CredFree(credential.cast());
        bytes
    };
    result
        .map(Some)
        .ok_or_else(|| storage_error("read the account credential"))
}

pub fn delete(id: &str) -> Result<()> {
    let name = target(id)?;
    if unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0
        && unsafe { GetLastError() } != ERROR_NOT_FOUND
    {
        return Err(storage_error("remove the account credential"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_identifiers_cannot_address_arbitrary_credentials() {
        assert!(target("another/application").is_err());
        assert!(write("invalid", b"test").is_err());
    }
    #[test]
    #[ignore = "writes and deletes a temporary Windows generic credential"]
    fn windows_credential_round_trip() {
        let id = uuid::Uuid::new_v4().to_string();
        assert!(read(&id).unwrap().is_none());
        write(&id, b"temporary test secret").unwrap();
        assert_eq!(
            read(&id).unwrap().as_deref(),
            Some(&b"temporary test secret"[..])
        );
        delete(&id).unwrap();
        assert!(read(&id).unwrap().is_none());
    }
}
