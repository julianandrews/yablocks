mod command;
mod inotify;
mod interval;
mod network;
mod pulse_volume;
mod signal;
mod stdin;
mod util;

use anyhow::Result;

use crate::config;
use crate::renderer::Renderer;

type BlockStream = futures::stream::BoxStream<'static, (String, Result<String>)>;

pub trait BlockStreamConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream>;
}

impl BlockStreamConfig for config::BlockConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        match self {
            config::BlockConfig::Command(config) => config.to_stream(name, renderer),
            config::BlockConfig::Interval(config) => config.to_stream(name, renderer),
            config::BlockConfig::Inotify(config) => config.to_stream(name, renderer),
            config::BlockConfig::Network(config) => config.to_stream(name, renderer),
            config::BlockConfig::PulseVolume(config) => config.to_stream(name, renderer),
            config::BlockConfig::Signal(config) => config.to_stream(name, renderer),
            config::BlockConfig::Stdin(config) => config.to_stream(name, renderer),
        }
    }
}
