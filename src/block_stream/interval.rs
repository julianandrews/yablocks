use anyhow::{Context, Result};
use futures::{stream, StreamExt};

use super::{BlockStream, BlockStreamConfig, Renderer};

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
    renderer: Renderer,
}

impl Block {
    fn new(
        name: String,
        template: String,
        command: String,
        args: Vec<String>,
        interval: u64,
        renderer: Renderer,
    ) -> Result<Self> {
        renderer
            .lock()
            .unwrap()
            .register_template_string(&name, template)?;
        Ok(Self {
            name,
            command,
            args,
            interval,
            renderer,
        })
    }

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
        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
        Ok(output)
    }

    async fn wait_for_output(&self) -> Result<Option<String>> {
        tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;
        let output = self.get_output().await?;
        Ok(Some(output))
    }
}

impl BlockStreamConfig for crate::config::IntervalConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        let block = Block::new(
            name.clone(),
            template,
            self.command,
            self.args,
            self.interval,
            renderer,
        )?;
        let initial_output = futures::executor::block_on(block.get_output())?;
        let first_run = stream::once(async { Ok((name, initial_output)) });
        let stream = stream::unfold(block, move |block| async {
            let result = block.wait_for_output().await;
            let tagged_result = match result {
                Ok(output) => Ok((block.name.clone(), output?)),
                Err(error) => Err(error).with_context(|| format!("Error from {}", block.name)),
            };
            Some((tagged_result, block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
