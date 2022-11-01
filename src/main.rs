mod block_stream;
mod config;
mod renderer;

use std::collections::BTreeMap;

use anyhow::{Context, Result};
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
    } = config::load_config(args.configfile).context("Failed to load config")?;
    let renderer = renderer::build(template).context("Failed to build template renderer")?;
    let block_streams: Vec<_> = block_configs
        .into_iter()
        .map(|(name, config)| {
            config
                .to_stream(name.clone(), renderer.clone())
                .context(format!("Failed to initialize block '{}'", name))
        })
        .collect::<Result<_>>()?;
    let mut stream = select_all(block_streams.into_iter());

    let mut outputs: BTreeMap<String, String> = BTreeMap::new();
    while let Some((name, value)) = stream.next().await {
        outputs.insert(name, value);
        tokio::time::sleep(DEBOUNCE_TIME).await;
        while let Some((name, value)) = stream.next().now_or_never().flatten() {
            outputs.insert(name, value);
        }
        let output = renderer
            .lock()
            .unwrap()
            .render("", &outputs)
            .context("Failed to render template")?;
        println!("{}", output);
    }
    Err(anyhow::anyhow!("Input stream ended"))
}

#[derive(Parser, Debug, Clone)]
#[clap(version, setting=AppSettings::DeriveDisplayOrder)]
pub struct Args {
    #[clap(short, long)]
    pub configfile: Option<std::path::PathBuf>,
}
