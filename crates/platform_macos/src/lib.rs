//! macOS platform integrations that do not belong in the desktop renderer.

use std::process::Command;

use app_core::{SecretKey, SecretStore, SecretStoreError};

const KEYCHAIN_SERVICE: &str = "com.gitronimo.github";

#[derive(Clone, Debug, Default)]
pub struct MacKeychainStore;

impl MacKeychainStore {
    #[must_use]
    pub fn github_key(account: &str) -> SecretKey {
        SecretKey {
            service: KEYCHAIN_SERVICE.into(),
            account: account.into(),
        }
    }
}

impl SecretStore for MacKeychainStore {
    fn read(&self, key: &SecretKey) -> Result<Option<String>, SecretStoreError> {
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                &key.service,
                "-a",
                &key.account,
                "-w",
            ])
            .output()
            .map_err(|_| SecretStoreError::Unavailable)?;
        if output.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&output.stdout)
                    .trim_end()
                    .to_owned(),
            ));
        }
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("could not be found") || error.contains("SecKeychainSearchCopyNext") {
            Ok(None)
        } else {
            Err(SecretStoreError::CommandFailed)
        }
    }

    fn write(&self, key: &SecretKey, value: &str) -> Result<(), SecretStoreError> {
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                &key.service,
                "-a",
                &key.account,
                "-w",
                value,
                "-U",
            ])
            .output()
            .map_err(|_| SecretStoreError::Unavailable)?;
        output
            .status
            .success()
            .then_some(())
            .ok_or(SecretStoreError::CommandFailed)
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        let output = Command::new("security")
            .args([
                "delete-generic-password",
                "-s",
                &key.service,
                "-a",
                &key.account,
            ])
            .output()
            .map_err(|_| SecretStoreError::Unavailable)?;
        if output.status.success() {
            Ok(())
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            if error.contains("could not be found") {
                Ok(())
            } else {
                Err(SecretStoreError::CommandFailed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MacKeychainStore;

    #[test]
    fn github_key_is_provider_scoped_without_a_secret() {
        let key = MacKeychainStore::github_key("octocat");
        assert_eq!(key.service, "com.gitronimo.github");
        assert_eq!(key.account, "octocat");
    }
}
