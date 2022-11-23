use anyhow::Result;
use futures::{stream, StreamExt};
use tokio::signal::unix::{signal, Signal, SignalKind};

use super::{BlockStream, BlockStreamConfig};
use crate::{config::RTSigNum, RENDERER};

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
}

impl Block {
    fn new(name: String, command: String, args: Vec<String>, num: RTSigNum) -> Result<Self> {
        let signal = signal(SignalKind::from_raw(num.0))?;
        Ok(Self {
            name,
            command,
            args,
            num,
            signal,
        })
    }

    async fn get_output(&self) -> Result<String> {
        let process_output = tokio::process::Command::new(&self.command)
            .args(&self.args)
            .output()
            .await?;
        let status = process_output.status.code().unwrap_or(0);
        let output = String::from_utf8_lossy(&process_output.stdout)
            .trim()
            .to_string();
        let data = BlockData {
            command: self.command.clone(),
            args: self.args.clone(),
            signal: self.num.0,
            status,
            output,
        };
        let output = RENDERER.render(&self.name, data)?;
        Ok(output)
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        self.signal.recv().await;
        Some(self.get_output().await)
    }
}

impl BlockStreamConfig for crate::config::SignalConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block::new(name.clone(), self.command, self.args, self.signal)?;
        let initial_output = futures::executor::block_on(block.get_output())?;
        let first_run = stream::once(async { (name, Ok(initial_output)) });
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
