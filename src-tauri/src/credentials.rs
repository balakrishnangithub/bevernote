//! Platform-native persistence for the pCloud access token.
use bevernote_core::error::{BeverError, Result};

const KEYRING_SERVICE: &str = "app.bevernote.desktop";
const KEYRING_ACCOUNT: &str = "pcloud";

#[cfg(not(target_os = "android"))]
mod platform {
    use super::*;

    fn entry() -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(storage_error)
    }

    fn storage_error(error: keyring::Error) -> BeverError {
        BeverError::Invalid(format!("secure credential storage: {error}"))
    }

    pub fn save_access_token(token: &str) -> Result<()> {
        entry()?.set_password(token).map_err(storage_error)
    }

    pub fn read_access_token() -> Result<Option<String>> {
        match entry()?.get_password() {
            Ok(token) if !token.trim().is_empty() => Ok(Some(token)),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(storage_error(error)),
        }
    }

    pub fn delete_access_token() -> Result<()> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(storage_error(error)),
        }
    }
}

#[cfg(target_os = "android")]
mod platform {
    use std::collections::HashMap;

    use android_native_keyring_store::Store;
    use keyring_core::api::CredentialStoreApi;

    use super::*;

    fn entry() -> Result<keyring_core::Entry> {
        let store = Store::new_with_configuration(&HashMap::from([("name", KEYRING_SERVICE)]))
            .map_err(storage_error)?;
        store
            .build(KEYRING_SERVICE, KEYRING_ACCOUNT, None)
            .map_err(storage_error)
    }

    fn storage_error(error: keyring_core::Error) -> BeverError {
        BeverError::Invalid(format!("Android Keystore credential storage: {error}"))
    }

    pub fn save_access_token(token: &str) -> Result<()> {
        entry()?.set_password(token).map_err(storage_error)
    }

    pub fn read_access_token() -> Result<Option<String>> {
        match entry()?.get_password() {
            Ok(token) if !token.trim().is_empty() => Ok(Some(token)),
            Ok(_) | Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(storage_error(error)),
        }
    }

    pub fn delete_access_token() -> Result<()> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(storage_error(error)),
        }
    }
}

pub use platform::{delete_access_token, read_access_token, save_access_token};
