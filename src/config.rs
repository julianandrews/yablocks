use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

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
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub template: String,
    pub header: Option<String>,
    pub stdin_handler: Option<StdinHandler>,
    #[serde(default)]
    pub blocks: BTreeMap<String, BlockConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct StdinHandler {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BlockConfig {
    Command(CommandConfig),
    Cpu(CpuConfig),
    DateTime(DateTimeConfig),
    Interval(IntervalConfig),
    Inotify(InotifyConfig),
    Network(NetworkConfig),
    NetworkStats(NetworkStatsConfig),
    Noop(NoopConfig),
    PulseVolume(PulseVolumeConfig),
    Signal(SignalConfig),
    Stdin(StdinConfig),
    Temperature(TemperatureConfig),
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CommandConfig {
    pub template: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub json: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IntervalConfig {
    pub template: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub interval: u64,
    #[serde(default)]
    pub json: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct InotifyConfig {
    pub template: Option<String>,
    pub file: std::path::PathBuf,
    #[serde(default)]
    pub json: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NetworkConfig {
    pub template: Option<String>,
    pub device: String,
}

const fn default_interval() -> u64 {
    1
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NetworkStatsConfig {
    pub template: Option<String>,
    pub device: String,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PulseVolumeConfig {
    pub template: Option<String>,
    pub sink_name: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SignalConfig {
    pub template: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub signal: RTSigNum,
    #[serde(default)]
    pub json: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(try_from = "i32")]
pub struct RTSigNum(pub i32);

impl TryFrom<i32> for RTSigNum {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        let min = libc::SIGRTMIN();
        let max = libc::SIGRTMAX();
        if value < min || value > max {
            Err(format!("Invalid signal (not between {min} and {max})"))
        } else {
            Ok(RTSigNum(value))
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct StdinConfig {
    pub template: Option<String>,
    #[serde(default)]
    pub json: bool,
}

/// A no-op block that does nothing. Used when no other blocks are configured.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NoopConfig {
    pub template: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CpuConfig {
    pub template: Option<String>,
    pub interval: u64,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Precision {
    Second,
    Minute,
    Hour,
    Day,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DateTimeConfig {
    pub template: Option<String>,
    pub precision: Precision,
    #[serde(default)]
    pub timezone: Option<chrono_tz::Tz>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TemperatureConfig {
    pub template: Option<String>,
    pub interval: u64,
}
