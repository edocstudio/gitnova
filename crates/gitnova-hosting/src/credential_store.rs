//! OS-managed secrets only. Preserve the existing macOS service/account keys.
use crate::err;
use gitnova_domain::*;

pub(crate) fn read(service: &str, account: &str) -> Result<Option<Vec<u8>>> {
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::{generic_password, PasswordOptions};
        match generic_password(PasswordOptions::new_generic_password(service, account)) {
            Ok(value) => Ok(Some(value)),
            Err(e) if e.code() == -25300 => Ok(None),
            Err(_) => Err(err("Could not access the system credential store. Unlock it and reconnect the account.")),
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        match entry(service, account)?.get_secret() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(err("Could not access the system credential store. Unlock it and reconnect the account.")),
        }
    }
}
pub(crate) fn write(service: &str, account: &str, value: &[u8]) -> Result<()> {
    #[cfg(target_os = "macos")]
    let result = security_framework::passwords::set_generic_password(service, account, value);
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let result = entry(service, account)?.set_secret(value);
    result.map_err(|_| err("Could not save the account in the system credential store."))
}
pub(crate) fn delete(service: &str, account: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        match security_framework::passwords::delete_generic_password(service, account) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(_) => Err(err(
                "Could not remove the account from the system credential store.",
            )),
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        match entry(service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(err(
                "Could not remove the account from the system credential store.",
            )),
        }
    }
}
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn entry(service: &str, account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(service, account).map_err(|_| {
        err("Could not access the system credential store. Unlock it and reconnect the account.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Explicit native acceptance test: synthetic entries only, never host accounts.
    #[test]
    #[ignore = "requires an unlocked isolated OS credential store"]
    fn platform_store_round_trip() {
        let account = format!(
            "fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let service = "org.gitnova.tests.credential-store";
        struct Cleanup<'a>(&'a str, &'a str);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = delete(self.0, self.1);
            }
        }
        let _cleanup = Cleanup(service, &account);
        assert!(read(service, &account).unwrap().is_none());
        write(service, &account, b"synthetic-credential-fixture").unwrap();
        assert_eq!(
            read(service, &account).unwrap().as_deref(),
            Some(b"synthetic-credential-fixture".as_slice())
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            read_background(service, &account).unwrap().as_deref(),
            Some(b"synthetic-credential-fixture".as_slice())
        );
        // Different account/service keys must not reuse this secret.
        assert!(read(service, &format!("{account}-other"))
            .unwrap()
            .is_none());
        assert!(read("org.gitnova.tests.other-store", &account)
            .unwrap()
            .is_none());
        delete(service, &account).unwrap();
        assert!(read(service, &account).unwrap().is_none());
        delete(service, &account).unwrap();
    }
}

/// Read only an already-unlocked, unique Secret Service item. Never unlock or
/// create a collection in the background; the zero prompt timeout is defensive.
#[cfg(target_os = "linux")]
pub(crate) fn read_background(service: &str, account: &str) -> Result<Option<Vec<u8>>> {
    use dbus_secret_service::{EncryptionType, SecretService};
    use std::collections::HashMap;
    let fail = || {
        err("Could not access the system credential store. Unlock it and reconnect the account.")
    };
    let ss = SecretService::connect_with_max_prompt_timeout(EncryptionType::Dh, 0)
        .map_err(|_| fail())?;
    let found = ss
        .search_items(HashMap::from([
            ("service", service),
            ("username", account),
            ("target", "default"),
        ]))
        .map_err(|_| fail())?;
    match (found.unlocked.len(), found.locked.len()) {
        (0, 0) => Ok(None),
        (1, 0) => found.unlocked[0].get_secret().map(Some).map_err(|_| fail()),
        _ => Err(fail()),
    }
}
