use gpui::{App, Task};
use secrecy::SecretString;

const SERVICE: &str = "chat.astrum.astrum";

pub fn get_secret(cx: &App, key: impl AsRef<str>) -> Task<anyhow::Result<SecretString>> {
    let key = key.as_ref().to_string();
    cx.background_executor().spawn(async move {
        let entry = keyring::Entry::new(SERVICE, &key)?;
        Ok(SecretString::from(entry.get_password()?))
    })
}

pub fn set_secret(cx: &App, key: impl AsRef<str>, value: impl AsRef<str>) -> Task<anyhow::Result<()>> {
    let key = key.as_ref().to_string();
    let value = value.as_ref().to_string();
    cx.background_executor().spawn(async move {
        // Remove first to avoid stale entries
        let _ = remove_entry(&key);
        let entry = keyring::Entry::new(SERVICE, &key)?;
        entry.set_password(&value)?;
        Ok(())
    })
}

pub fn remove_secret(cx: &App, key: impl AsRef<str>) -> Task<anyhow::Result<()>> {
    let key = key.as_ref().to_string();
    cx.background_executor().spawn(async move {
        remove_entry(&key)
    })
}

fn remove_entry(key: &str) -> anyhow::Result<()> {
    match keyring::Entry::new(SERVICE, key) {
        Ok(entry) => match entry.delete_credential() {
            Err(keyring::Error::NoEntry) => Ok(()),
            other => other.map_err(Into::into),
        },
        Err(e) => Err(e.into()),
    }
}
