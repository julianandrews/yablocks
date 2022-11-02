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
    Command(CommandConfig),
    Interval(IntervalConfig),
    Inotify(InotifyConfig),
    Network(NetworkConfig),
    PulseVolume(PulseVolumeConfig),
}

/// Template Values:
///   - command
///   - args
///   - output
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct CommandConfig {
    pub template: Option<String>,
    pub command: String,
    pub args: Vec<String>,
}

/// Template Values:
///   - command
///   - args
///   - interval
///   - output
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct IntervalConfig {
    pub template: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub interval: u64,
}

/// Template Values:
///   - contents
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct InotifyConfig {
    pub template: Option<String>,
    pub file: std::path::PathBuf,
}

/// Template Values:
///   - operstate
///   - wireless
///   - essid
///   - quality
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct NetworkConfig {
    pub template: Option<String>,
    pub device: String,
}

/// Template Values:
///   - sink_name
///   - volume
///   - muted
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PulseVolumeConfig {
    pub template: Option<String>,
    pub sink_name: String,
}
