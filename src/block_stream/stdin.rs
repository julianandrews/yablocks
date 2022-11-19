use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::channel::mpsc::{Receiver, Sender};
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{util::send_or_eprint, BlockStream, BlockStreamConfig, Renderer};

static READER: Lazy<StdinReader> = Lazy::new(StdinReader::spawn);

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    output: String,
}

struct Block {
    name: String,
    rx: Receiver<Result<BlockData>>,
    renderer: Renderer,
}

impl Block {
    fn new(name: String, template: String, mut renderer: Renderer) -> Result<Self> {
        let rx = READER.subscribe();
        renderer.add_template(&name, &template)?;
        Ok(Self { name, rx, renderer })
    }

    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        let data = match self.rx.next().await {
            Some(data) => data?,
            None => return Ok(None),
        };
        let output = self.renderer.render(&self.name, data)?;
        Ok(Some(output))
    }
}

impl BlockStreamConfig for crate::config::StdinConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        let block = Block::new(name, template, renderer)?;

        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await.transpose()?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}

struct StdinReader {
    senders: Arc<Mutex<Vec<Sender<Result<BlockData>>>>>,
}

impl StdinReader {
    fn spawn() -> Self {
        let senders = Arc::new(Mutex::new(vec![]));
        let senders_clone = senders.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            while let Some(result) = lines.next_line().await.transpose() {
                for tx in &mut *senders_clone.lock().unwrap() {
                    let result = match &result {
                        Ok(s) => Ok(BlockData {
                            output: s.to_string(),
                        }),
                        Err(e) => Err(anyhow::anyhow!("Failed to read from stdin: {}", e.kind())),
                    };
                    send_or_eprint(result, tx);
                }
            }
        });
        Self { senders }
    }

    fn subscribe(&self) -> Receiver<Result<BlockData>> {
        let (tx, rx) = futures::channel::mpsc::channel::<Result<BlockData>>(1);
        self.senders.lock().unwrap().push(tx);
        rx
    }
}
