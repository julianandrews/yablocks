use std::collections::BTreeMap;

use anyhow::Result;
use serde::Deserialize;

pub fn load_config(file: Option<std::path::PathBuf>) -> Result<Config> {
    let file = match file {
        Some(file) => file,
        None => xdg::BaseDirectories::with_prefix(env!("CARGO_BIN_NAME"))?
            .find_config_file("config.toml")
            .ok_or_else(|| anyhow::anyhow!("Failed to find config"))?,
    };

    let config: Config = toml::from_str(&std::fs::read_to_string(file)?)?;
    Ok(config)
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub template: String,
    pub blocks: BTreeMap<String, BlockConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BlockConfig {
    Inotify(InotifyConfig),
}

/// Template Values:
///   - contents
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct InotifyConfig {
    pub template: Option<String>,
    pub file: std::path::PathBuf,
}
