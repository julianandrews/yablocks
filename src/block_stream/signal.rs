use anyhow::Result;
use futures::{stream, StreamExt};
use tokio::process::Command;
use tokio::signal::unix::{signal, Signal, SignalKind};

use super::{BlockStream, BlockStreamConfig};
use crate::{config::RTSigNum, RENDERER};

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData<'a> {
    command: &'a str,
    args: &'a Vec<String>,
    signal: i32,
    status: i32,
    output: serde_json::Value,
}

struct Block {
    name: String,
    command: String,
    args: Vec<String>,
    num: RTSigNum,
    signal: Signal,
    json: bool,
}

impl Block {
    fn new(
        name: String,
        command: String,
        args: Vec<String>,
        num: RTSigNum,
        json: bool,
    ) -> Result<Self> {
        let signal = signal(SignalKind::from_raw(num.0))?;
        Ok(Self {
            name,
            command,
            args,
            num,
            signal,
            json,
        })
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        self.signal.recv().await;
        Some(render_output(&self.name, &self.command, &self.args, self.num.0, self.json).await)
    }
}

impl BlockStreamConfig for crate::config::SignalConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block::new(
            name.clone(),
            self.command.clone(),
            self.args.clone(),
            self.signal,
            self.json,
        )?;
        let first_run = stream::once(async move {
            let output =
                render_output(&name, &self.command, &self.args, self.signal.0, self.json).await;
            (name, output)
        });
        let stream = stream::unfold(block, move |mut block| async {
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
    signal: i32,
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
        signal,
        status,
        output,
    };
    RENDERER.render(name, data)
}
