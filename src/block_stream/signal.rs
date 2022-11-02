use anyhow::{Context, Result};
use futures::{stream, StreamExt};
use tokio::signal::unix::{signal, Signal, SignalKind};

use super::{BlockStream, BlockStreamConfig, Renderer};
use crate::config::RTSigNum;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    command: String,
    args: Vec<String>,
    signal: i32,
    status: i32,
    output: String,
}

struct Block {
    name: String,
    command: String,
    args: Vec<String>,
    num: RTSigNum,
    signal: Signal,
    renderer: Renderer,
}

impl Block {
    fn new(
        name: String,
        template: String,
        command: String,
        args: Vec<String>,
        num: RTSigNum,
        renderer: Renderer,
    ) -> Result<Self> {
        renderer
            .lock()
            .unwrap()
            .register_template_string(&name, template)?;
        let signal = signal(SignalKind::from_raw(num.0))?;
        Ok(Self {
            name,
            command,
            args,
            num,
            signal,
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
            signal: self.num.0,
            status,
            output,
        })
    }

    async fn get_output(&self) -> Result<String> {
        let data = self.get_data().await?;
        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
        Ok(output)
    }

    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        self.signal.recv().await;
        let output = self.get_output().await?;
        Ok(Some(output))
    }
}

impl BlockStreamConfig for crate::config::SignalConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        let block = Block::new(
            name.clone(),
            template,
            self.command,
            self.args,
            self.signal,
            renderer,
        )?;
        let initial_output = futures::executor::block_on(block.get_output())?;
        let first_run = stream::once(async { Ok((name, initial_output)) });
        let stream = stream::unfold(block, move |mut block| async {
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
