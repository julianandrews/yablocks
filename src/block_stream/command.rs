use anyhow::Result;
use futures::stream;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    command: String,
    args: Vec<String>,
    output: serde_json::Value,
}

struct Block {
    name: String,
    command: String,
    args: Vec<String>,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
    json: bool,
}

impl Block {
    fn new(name: String, command: String, args: Vec<String>, json: bool) -> Result<Self> {
        let stdout = tokio::process::Command::new(&command)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .spawn()?
            .stdout
            .ok_or_else(|| anyhow::anyhow!(format!("Failed to open stdout for {}", name)))?;
        let lines = BufReader::new(stdout).lines();
        Ok(Self {
            name,
            command,
            args,
            lines,
            json,
        })
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        let output = match self.lines.next_line().await.transpose()? {
            Ok(output) => output,
            Err(e) => return Some(Err(anyhow::Error::from(e))),
        };
        let output = if self.json {
            match serde_json::from_str(&output) {
                Ok(value) => value,
                Err(e) => return Some(Err(anyhow::Error::from(e))),
            }
        } else {
            serde_json::json!(output)
        };
        let data = BlockData {
            command: self.command.clone(),
            args: self.args.clone(),
            output,
        };
        Some(RENDERER.render(&self.name, data))
    }
}

impl BlockStreamConfig for crate::config::CommandConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block::new(name, self.command, self.args, self.json)?;
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}
