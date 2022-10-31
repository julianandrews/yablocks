mod block_stream;
mod config;
mod renderer;

use std::collections::BTreeMap;

use anyhow::Result;
use clap::{AppSettings, Parser};
use futures::stream::select_all::select_all;
use futures::{FutureExt, StreamExt};

use block_stream::BlockStreamConfig;

static DEBOUNCE_TIME: std::time::Duration = std::time::Duration::from_millis(10);

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config::Config {
        template,
        blocks: block_configs,
    } = config::load_config(args.configfile)?;
    let renderer = renderer::build(template)?;
    let block_streams: Vec<_> = block_configs
        .into_iter()
        .map(|(name, config)| config.to_stream(name, renderer.clone()))
        .collect::<Result<_>>()?;
    let mut stream = select_all(block_streams.into_iter());

    let mut outputs: BTreeMap<String, String> = BTreeMap::new();
    while let Some((name, value)) = stream.next().await {
        outputs.insert(name, value);
        std::thread::sleep(DEBOUNCE_TIME);
        while let Some((name, value)) = stream.next().now_or_never().flatten() {
            outputs.insert(name, value);
        }
        println!("{}", renderer.lock().unwrap().render("", &outputs)?);
    }
    Err(anyhow::anyhow!("Input stream ended"))
}

#[derive(Parser, Debug, Clone)]
#[clap(version, setting=AppSettings::DeriveDisplayOrder)]
pub struct Args {
    #[clap(short, long)]
    pub configfile: Option<std::path::PathBuf>,
}
