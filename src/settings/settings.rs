use anyhow::{Result, anyhow};
use config::{Config, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub auth: Auth,
    pub captcha: Captcha,
    pub chat: Chat,
    pub http: Http,
    pub log: Log,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct Auth {
    pub backend: String, // "fake" or "real"
}

#[derive(Debug, Deserialize)]
pub struct Captcha {
    pub backend: String, // "fake" or "real"
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub backend: String, // "fake" or "real"
}

#[derive(Debug, Deserialize)]
pub struct Http {
    pub cert_path: String,
    pub key_path: String,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct Log {
    pub filter: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub backend: String, // "fake" or "real"
}

#[cfg(debug_assertions)]
const SETTINGS_PATH: &str = "settings/dev.toml";
#[cfg(not(debug_assertions))]
const SETTINGS_PATH: &str = "settings/release.toml";

pub fn parse_settings(path: Option<&str>) -> Result<Settings> {
    let path = path.unwrap_or(SETTINGS_PATH);

    let settings: Settings = Config::builder()
        .add_source(File::with_name(path))
        .build()
        .map_err(|e| anyhow!(e))?
        .try_deserialize()
        .map_err(|e| anyhow!(e))?;

    Ok(settings)
}
