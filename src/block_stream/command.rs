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
        let child = tokio::process::Command::new(&command)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let stdout = child
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

    async fn get_data(&mut self) -> Option<BlockData> {
        let output = match self.lines.next_line().await {
            Ok(Some(output)) => output,
            Ok(None) => return None,
            Err(e) => {
                eprintln!("Error reading input: {:?}", e);
                "Error".to_string()
            }
        };
        Some(BlockData {
            command: self.command.clone(),
            args: self.args.clone(),
            output,
        })
    }

    fn render(&self, data: &BlockData) -> Result<String> {
        let output = self.renderer.lock().unwrap().render(&self.name, data)?;
        Ok(output)
    }
}

impl BlockStreamConfig for crate::config::CommandConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        let block = Block::new(name, template, self.command, self.args, renderer)?;
        let stream = stream::unfold(block, move |mut block| async {
            let data = match block.get_data().await {
                Some(data) => data,
                None => return None,
            };
            let output = match block.render(&data) {
                Ok(output) => output,
                Err(error) => {
                    eprintln!("Error rendering template: {:?}", error);
                    "Error".to_string()
                }
            };
            Some(((block.name.clone(), output), block))
        });

        Ok(Box::pin(stream))
    }
}
