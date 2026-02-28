use gpui::{App, Task};
use secrecy::SecretString;

pub fn get_secret(cx: &App, key: impl AsRef<str>) -> Task<anyhow::Result<SecretString>> {
    let task = cx.read_credentials(key.as_ref());
    cx.foreground_executor().spawn(async move {
        let result = task.await?;
        match result {
            Some((_username, password)) => {
                let s = String::from_utf8(password)?;
                Ok(SecretString::from(s))
            }
            None => Err(anyhow::anyhow!("No credential found")),
        }
    })
}

pub fn set_secret(cx: &App, key: impl AsRef<str>, value: impl AsRef<str>) -> Task<anyhow::Result<()>> {
    cx.write_credentials(key.as_ref(), "api_key", value.as_ref().as_bytes())
}

pub fn remove_secret(cx: &App, key: impl AsRef<str>) -> Task<anyhow::Result<()>> {
    cx.delete_credentials(key.as_ref())
}
