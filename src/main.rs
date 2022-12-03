mod block_stream;
mod config;
mod renderer;
mod stdin_handler;

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use clap::{AppSettings, Parser};
use futures::stream::select_all::select_all;
use futures::{FutureExt, StreamExt};

use block_stream::BlockStreamConfig;
pub use renderer::RENDERER;

static DEBOUNCE_TIME: std::time::Duration = std::time::Duration::from_millis(10);

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config::Config {
        template,
        header,
        stdin_handler,
        blocks: block_configs,
    } = config::load_config(args.configfile).context("Failed to load config")?;

    // If an stdin_handler is specified, make sure we're not using any stdin blocks, then run it.
    if let Some(handler) = stdin_handler {
        if block_configs
            .iter()
            .any(|(_, config)| matches!(config, config::BlockConfig::Stdin(_)))
        {
            anyhow::bail!("Cannot use stdin block with stdin_handler");
        }
        stdin_handler::spawn_handler(handler)?;
    }

    // Use the empty string for the root template to avoid conflicts with any block templates.
    RENDERER
        .add_template("", &template)
        .context("Failed to build template renderer")?;

    // Initialize the context so we can start rendering immediately
    let mut context = BTreeMap::new();
    for name in block_configs.keys() {
        context.insert(name.clone(), "".to_string());
    }

    let block_streams = block_configs
        .into_iter()
        .map(|(name, config)| {
            config
                .to_stream(name.clone())
                .with_context(|| format!("Failed to initialize block '{}'", name))
        })
        .filter_map(|result| match result {
            Ok(block_stream) => Some(block_stream),
            Err(error) => {
                eprintln!("{:?}", error);
                None
            }
        });
    let mut stream = select_all(block_streams);

    if let Some(header) = header {
        println!("{}", header);
    }
    while let Some((name, result)) = stream.next().await {
        match result {
            Ok(value) => context.insert(name, value),
            Err(error) => {
                eprintln!("Error from {}: {:?}", name, error);
                continue;
            }
        };
        tokio::time::sleep(DEBOUNCE_TIME).await;
        while let Some((name, result)) = stream.next().now_or_never().flatten() {
            match result {
                Ok(value) => {
                    context.insert(name, value);
                }
                Err(error) => eprintln!("Error from {}: {:?}", name, error),
            };
        }
        let output = RENDERER
            .render("", &context)
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
