use anyhow::Result;
use futures::{stream, StreamExt};
use tokio::process::Command;

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData<'a> {
    command: &'a str,
    args: &'a Vec<String>,
    interval: u64,
    status: i32,
    output: serde_json::Value,
}

#[derive(Debug, Clone)]
struct Block {
    name: String,
    command: String,
    args: Vec<String>,
    interval: u64,
    json: bool,
}

impl Block {
    async fn wait_for_output(&self) -> Option<Result<String>> {
        tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;
        Some(
            render_output(
                &self.name,
                &self.command,
                &self.args,
                self.interval,
                self.json,
            )
            .await,
        )
    }
}

impl BlockStreamConfig for crate::config::IntervalConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block {
            name: name.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            interval: self.interval,
            json: self.json,
        };
        let first_run = stream::once(async move {
            let output =
                render_output(&name, &self.command, &self.args, self.interval, self.json).await;
            (name, output)
        });
        let stream = stream::unfold(block, move |block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}

async fn render_output(
    name: &str,
    command: &str,
    args: &Vec<String>,
    interval: u64,
    json: bool,
) -> Result<String> {
    let process_output = Command::new(command).args(args).output().await?;
    let status = process_output.status.code().unwrap_or(0);
    let output = if json {
        serde_json::from_slice(&process_output.stdout)?
    } else {
        serde_json::json!(String::from_utf8_lossy(&process_output.stdout).trim())
    };
    let data = BlockData {
        command,
        args,
        interval,
        status,
        output,
    };
    RENDERER.render(name, data)
}
