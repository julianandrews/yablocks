use anyhow::Result;
use futures::channel::mpsc::Receiver;
use futures::stream;
use futures::{FutureExt, SinkExt, StreamExt};
use notify::Watcher;

use super::{BlockStream, BlockStreamConfig, Renderer};

static DEBOUNCE_TIME: std::time::Duration = std::time::Duration::from_millis(10);

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    file: String,
    contents: String,
}

struct Block {
    name: String,
    file: std::path::PathBuf,
    rx: Receiver<notify::Result<notify::Event>>,
    renderer: Renderer,
    _watcher: notify::RecommendedWatcher,
}

impl Block {
    fn new(
        name: String,
        template: String,
        file: std::path::PathBuf,
        mut renderer: Renderer,
    ) -> Result<Self> {
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
        renderer.add_template(&name, &template)?;
        Ok(Self {
            name,
            file,
            rx,
            renderer,
            _watcher: watcher,
        })
    }

    async fn get_data(&self) -> Result<BlockData> {
        let contents = match tokio::fs::read_to_string(&self.file).await {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "".to_string(),
            Err(error) => Err(error)?,
        };
        Ok(BlockData {
            file: self.file.to_string_lossy().into_owned(),
            contents,
        })
    }

    async fn get_output(&mut self) -> Result<String> {
        let data = self.get_data().await?;
        let output = self.renderer.render(&self.name, data)?;
        Ok(output)
    }

    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        loop {
            let mut results = match self.rx.next().await {
                Some(result) => vec![result],
                None => return Ok(None),
            };
            tokio::time::sleep(DEBOUNCE_TIME).await;
            while let Some(result) = self.rx.next().now_or_never().flatten() {
                results.push(result);
            }
            for result in results {
                let event = result?;
                for path in event.paths {
                    if path == self.file {
                        return self.get_output().await.map(Some);
                    }
                }
            }
        }
    }
}

impl BlockStreamConfig for crate::config::InotifyConfig {
    fn to_stream<'a>(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{contents}}".to_string());
        let mut block = Block::new(name.clone(), template, self.file, renderer)?;
        let initial_output = futures::executor::block_on(block.get_output())?;
        let first_run = stream::once(async { (name, Ok(initial_output)) });
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await.transpose()?;
            Some(((block.name.clone(), result), block))
        });
        Ok(Box::pin(first_run.chain(stream)))
    }
}
