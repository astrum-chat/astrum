use secrecy::SecretString;

const SERVICE: &str = "chat.astrum.astrum";

fn get_raw_secret(key: impl AsRef<str>) -> keyring::Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, key.as_ref())?)
}

pub fn get_secret(key: impl AsRef<str>) -> keyring::Result<SecretString> {
    let entry = get_raw_secret(key)?;
    Ok(SecretString::from(entry.get_password()?))
}

pub fn set_secret(key: impl AsRef<str>, value: impl AsRef<str>) -> keyring::Result<()> {
    let key = key.as_ref();

    remove_secret(key)?;

    let entry = keyring::Entry::new(SERVICE, key)?;

    entry.set_password(value.as_ref())?;

    Ok(())
}

pub fn remove_secret(key: impl AsRef<str>) -> keyring::Result<()> {
    if let Ok(secret) = get_raw_secret(key) {
        secret.delete_credential()?;
    }

    Ok(())
}
