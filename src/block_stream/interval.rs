use anyhow::Result;
use futures::{stream, StreamExt};

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    command: String,
    args: Vec<String>,
    interval: u64,
    status: i32,
    output: String,
}

struct Block {
    name: String,
    command: String,
    args: Vec<String>,
    interval: u64,
}

impl Block {
    async fn get_data(&self) -> Result<BlockData> {
        let process_output = tokio::process::Command::new(&self.command)
            .args(&self.args)
            .output()
            .await?;
        let status = process_output.status.code().unwrap_or(0);
        let output = String::from_utf8_lossy(&process_output.stdout)
            .trim()
            .to_string();
        Ok(BlockData {
            command: self.command.clone(),
            args: self.args.clone(),
            interval: self.interval,
            status,
            output,
        })
    }

    async fn get_output(&self) -> Result<String> {
        let data = self.get_data().await?;
        let output = RENDERER.render(&self.name, data)?;
        Ok(output)
    }

    async fn wait_for_output(&self) -> Option<Result<String>> {
        tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;
        Some(self.get_output().await)
    }
}

impl BlockStreamConfig for crate::config::IntervalConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block {
            name: name.clone(),
            command: self.command,
            args: self.args,
            interval: self.interval,
        };
        let initial_output = futures::executor::block_on(block.get_output())?;
        let first_run = stream::once(async { (name, Ok(initial_output)) });
        let stream = stream::unfold(block, move |block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
