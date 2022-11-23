use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::channel::mpsc::{Receiver, Sender};
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{util::send_or_eprint, BlockStream, BlockStreamConfig};
use crate::RENDERER;

static READER: Lazy<StdinReader> = Lazy::new(StdinReader::spawn);

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    output: String,
}

struct Block {
    name: String,
    rx: Receiver<Result<BlockData>>,
}

impl Block {
    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        let data = match self.rx.next().await? {
            Ok(data) => data,
            Err(e) => return Some(Err(e)),
        };
        Some(RENDERER.render(&self.name, data))
    }
}

impl BlockStreamConfig for crate::config::StdinConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
        RENDERER.add_template(&name, &template)?;
        let rx = READER.subscribe();

        let block = Block { name, rx };
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
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
