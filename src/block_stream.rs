mod inotify;
mod network;
mod pulse_volume;

use anyhow::Result;

use crate::config;
use crate::renderer::Renderer;

type BlockStream = futures::stream::BoxStream<'static, (String, String)>;

pub trait BlockStreamConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream>;
}

impl BlockStreamConfig for config::BlockConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        match self {
            config::BlockConfig::Inotify(config) => config.to_stream(name, renderer),
            config::BlockConfig::Network(config) => config.to_stream(name, renderer),
            config::BlockConfig::PulseVolume(config) => config.to_stream(name, renderer),
        }
    }
}
