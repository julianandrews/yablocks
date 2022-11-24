mod command;
mod cpu;
mod inotify;
mod interval;
mod network;
mod pulse_volume;
mod signal;
mod stdin;
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
            config::BlockConfig::Interval(config) => config.to_stream(name),
            config::BlockConfig::Inotify(config) => config.to_stream(name),
            config::BlockConfig::Network(config) => config.to_stream(name),
            config::BlockConfig::PulseVolume(config) => config.to_stream(name),
            config::BlockConfig::Signal(config) => config.to_stream(name),
            config::BlockConfig::Stdin(config) => config.to_stream(name),
        }
    }
}
