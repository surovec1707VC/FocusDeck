//! SecretStore: API keys and tokens live in the OS credential store
//! (Windows Credential Manager), never in files or SQLite.

const SERVICE: &str = "com.focusdeck.app";

#[derive(Debug, thiserror::Error)]
#[error("secret store error: {0}")]
pub struct SecretError(String);

impl From<keyring::Error> for SecretError {
    fn from(e: keyring::Error) -> Self {
        Self(e.to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SecretStore;

impl SecretStore {
    fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
        Ok(keyring::Entry::new(SERVICE, key)?)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        Self::entry(key)?.set_password(value)?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        match Self::entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn has(&self, key: &str) -> Result<bool, SecretError> {
        Ok(self.get(key)?.is_some())
    }

    pub fn delete(&self, key: &str) -> Result<(), SecretError> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
