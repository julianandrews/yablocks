use anyhow::Result;
use futures::stream;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{BlockStream, BlockStreamConfig, Renderer};

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    command: String,
    args: Vec<String>,
    output: String,
}

struct Block {
    name: String,
    command: String,
    args: Vec<String>,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
    renderer: Renderer,
}

impl Block {
    fn new(
        name: String,
        template: String,
        command: String,
        args: Vec<String>,
        renderer: Renderer,
    ) -> Result<Self> {
        renderer
            .lock()
            .unwrap()
            .register_template_string(&name, template)?;
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
            renderer,
        })
    }

    async fn get_data(&mut self) -> Result<Option<BlockData>> {
        let output = match self.lines.next_line().await? {
            Some(output) => output,
            None => return Ok(None),
        };
        Ok(Some(BlockData {
            command: self.command.clone(),
            args: self.args.clone(),
            output,
        }))
    }

    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        let data = self.get_data().await?;
        match data {
            Some(data) => {
                let result = self.renderer.lock().unwrap().render(&self.name, &data)?;
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }
}

impl BlockStreamConfig for crate::config::CommandConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        let block = Block::new(name, template, self.command, self.args, renderer)?;
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await.transpose()?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}
