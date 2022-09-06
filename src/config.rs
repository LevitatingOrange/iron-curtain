use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use time::UtcOffset;
use tokio::fs::read_to_string;
use url::Url;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub enum Secret {
    Env,
    Plain(String),
    File(PathBuf),
}

impl Secret {
    pub async fn extract(&self, env_name: &'static str) -> eyre::Result<String> {
        let s = match self {
            Secret::Env => std::env::var(env_name)?,
            Secret::Plain(s) => s.to_owned(),
            Secret::File(f) => read_to_string(f).await?,
        };
        Ok(s)
    }
}

impl Default for Secret {
    fn default() -> Self {
        Self::Env
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct Config {
    pub scrape_urls: Vec<Url>,
    pub utc_offset: UtcOffset,
    pub search_duration_in_days: u64,
    pub team_regex: String,
    pub pushover: PushoverConfig,
}

impl Config {
    pub async fn load(config_path: impl AsRef<Path>) -> Result<Self> {
        let path = config_path.as_ref();
        let contents = read_to_string(path)
            .await
            .wrap_err_with(|| format!("failed to read config from {}", path.display()))?;
        let config = toml::from_str(&contents)
            .wrap_err_with(|| format!("failed to parse config from {}", path.display()))?;
        Ok(config)
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct PushoverConfig {
    pub token: Secret,
    pub user_key: Secret,
    #[serde(default = "default::notification_sound")]
    pub notification_sound: String,
    pub notification_title: String,
    pub notification_message: String,
}

mod default {
    pub fn notification_sound() -> String {
        "intermission".to_owned()
    }
}
