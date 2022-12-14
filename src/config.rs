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
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub template: String,
    pub header: Option<String>,
    pub stdin_handler: Option<StdinHandler>,
    #[serde(default)]
    pub blocks: BTreeMap<String, BlockConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
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
    Interval(IntervalConfig),
    Inotify(InotifyConfig),
    Network(NetworkConfig),
    PulseVolume(PulseVolumeConfig),
    Signal(SignalConfig),
    Stdin(StdinConfig),
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
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub json: bool,
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
    #[serde(default)]
    pub args: Vec<String>,
    pub interval: u64,
    #[serde(default)]
    pub json: bool,
}

/// Template Values:
///   - file
///   - contents
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct InotifyConfig {
    pub template: Option<String>,
    pub file: std::path::PathBuf,
    #[serde(default)]
    pub json: bool,
}

/// Template Values:
///   - device
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
    pub sink_name: Option<String>,
}

/// Template Values:
///   - command
///   - args
///   - signal
///   - output
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
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
            Err(format!("Invalid signal (not between {} and {})", min, max))
        } else {
            Ok(RTSigNum(value))
        }
    }
}

/// Template Values:
///   - output
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct StdinConfig {
    pub template: Option<String>,
    #[serde(default)]
    pub json: bool,
}

/// Template Values:
///   - interval
///   - cpu_times
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct CpuConfig {
    pub template: Option<String>,
    pub interval: u64,
}
