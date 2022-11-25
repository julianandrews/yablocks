use anyhow::Result;
use futures::channel::mpsc::Receiver;
use futures::stream;
use futures::{FutureExt, SinkExt, StreamExt};
use notify::Watcher;

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

static DEBOUNCE_TIME: std::time::Duration = std::time::Duration::from_millis(10);

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    file: String,
    contents: serde_json::Value,
}

struct Block {
    name: String,
    file: std::path::PathBuf,
    rx: Receiver<notify::Result<notify::Event>>,
    json: bool,
    _watcher: notify::RecommendedWatcher,
}

impl Block {
    fn new(name: String, file: std::path::PathBuf, json: bool) -> Result<Self> {
        let (mut tx, rx) = futures::channel::mpsc::channel(1);
        let mut watcher = notify::RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            notify::Config::default(),
        )?;
        let watch_dir = file.parent().unwrap_or_else(|| std::path::Path::new("/"));
        watcher.watch(watch_dir, notify::RecursiveMode::NonRecursive)?;
        Ok(Self {
            name,
            file,
            rx,
            json,
            _watcher: watcher,
        })
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        loop {
            let mut results = vec![self.rx.next().await?];
            tokio::time::sleep(DEBOUNCE_TIME).await;
            while let Some(result) = self.rx.next().now_or_never().flatten() {
                results.push(result);
            }
            for result in results {
                let event = match result {
                    Ok(event) => event,
                    Err(e) => return Some(Err(anyhow::Error::from(e))),
                };
                for path in event.paths {
                    if path == self.file {
                        return Some(render_file(&self.name, &self.file, self.json).await);
                    }
                }
            }
        }
    }
}

impl BlockStreamConfig for crate::config::InotifyConfig {
    fn to_stream<'a>(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{contents}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block::new(name.clone(), self.file.clone(), self.json)?;
        let first_run = stream::once(async move {
            let result = render_file(&name, &self.file, self.json).await;
            (name, result)
        });
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}

async fn render_file(name: &str, file: &std::path::Path, json: bool) -> Result<String> {
    let contents = match tokio::fs::read_to_string(file).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "".to_string(),
        Err(error) => Err(error)?,
    };
    let contents = if json {
        serde_json::from_str(&contents)?
    } else {
        serde_json::json!(contents)
    };
    let data = BlockData {
        file: file.to_string_lossy().into_owned(),
        contents,
    };
    RENDERER.render(name, data)
}
