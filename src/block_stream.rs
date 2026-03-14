mod command;
mod cpu;
mod datetime;
mod inotify;
mod interval;
mod network;
mod network_stats;
mod noop;
mod pulse_volume;
mod signal;
mod stdin;
mod temperature;
mod util;

use anyhow::Result;

use crate::config;

type BlockStream = futures::stream::BoxStream<'static, (String, Result<String>)>;

pub trait BlockStreamConfig {
    fn to_stream(self, name: String) -> Result<BlockStream>;
}

impl BlockStreamConfig for config::BlockConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        match self {
            config::BlockConfig::Command(config) => config.to_stream(name),
            config::BlockConfig::Cpu(config) => config.to_stream(name),
            config::BlockConfig::DateTime(config) => config.to_stream(name),
            config::BlockConfig::Interval(config) => config.to_stream(name),
            config::BlockConfig::Inotify(config) => config.to_stream(name),
            config::BlockConfig::Network(config) => config.to_stream(name),
            config::BlockConfig::NetworkStats(config) => config.to_stream(name),
            config::BlockConfig::Noop(config) => config.to_stream(name),
            config::BlockConfig::PulseVolume(config) => config.to_stream(name),
            config::BlockConfig::Signal(config) => config.to_stream(name),
            config::BlockConfig::Stdin(config) => config.to_stream(name),
            config::BlockConfig::Temperature(config) => config.to_stream(name),
        }
    }
}
