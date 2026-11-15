use std::sync::Arc;

use type4we_core::secrets::{credential_target, SecretError, SecretStore};
use windows::core::PCWSTR;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

pub struct WindowsSecretStore;

pub fn windows_secret_store() -> Arc<dyn SecretStore> {
    Arc::new(WindowsSecretStore)
}

impl SecretStore for WindowsSecretStore {
    fn get(&self, account: &str, field: &str) -> Result<Option<String>, SecretError> {
        match read_secret(&credential_target(account, field)) {
            Ok(value) => Ok(value),
            Err(err) if not_found(&err) => Ok(None),
            Err(err) => Err(SecretError::Message(err)),
        }
    }

    fn set(&self, account: &str, field: &str, value: &str) -> Result<(), SecretError> {
        write_secret(&credential_target(account, field), value).map_err(SecretError::Message)
    }

    fn delete(&self, account: &str, field: &str) -> Result<(), SecretError> {
        match delete_secret(&credential_target(account, field)) {
            Ok(()) => Ok(()),
            Err(err) if not_found(&err) => Ok(()),
            Err(err) => Err(SecretError::Message(err)),
        }
    }
}

fn write_secret(target: &str, secret: &str) -> Result<(), String> {
    let mut target_wide = wide(target);
    let mut user_wide = wide("type4we");
    let mut blob = secret.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Flags: windows::Win32::Security::Credentials::CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: windows::core::PWSTR(target_wide.as_mut_ptr()),
        Comment: windows::core::PWSTR::null(),
        LastWritten: FILETIME::default(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: windows::core::PWSTR::null(),
        UserName: windows::core::PWSTR(user_wide.as_mut_ptr()),
    };
    unsafe { CredWriteW(&credential, 0) }.map_err(|err| err.to_string())
}

fn read_secret(target: &str) -> Result<Option<String>, String> {
    let target_wide = wide(target);
    let mut raw = std::ptr::null_mut();
    unsafe {
        CredReadW(
            PCWSTR(target_wide.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut raw,
        )
    }
    .map_err(|err| err.to_string())?;
    if raw.is_null() {
        return Ok(None);
    }
    let secret = unsafe {
        let credential = &*raw;
        let len = credential.CredentialBlobSize as usize;
        let bytes = if credential.CredentialBlob.is_null() || len == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(credential.CredentialBlob, len)
        };
        let text = String::from_utf8_lossy(bytes).into_owned();
        CredFree(raw.cast());
        text
    };
    if secret.is_empty() {
        Ok(None)
    } else {
        Ok(Some(secret))
    }
}

fn delete_secret(target: &str) -> Result<(), String> {
    let target_wide = wide(target);
    unsafe { CredDeleteW(PCWSTR(target_wide.as_ptr()), CRED_TYPE_GENERIC, None) }
        .map_err(|err| err.to_string())
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn not_found(err: &str) -> bool {
    err.contains("1168")
        || err.contains("0x80070490")
        || err.to_ascii_lowercase().contains("not found")
}
