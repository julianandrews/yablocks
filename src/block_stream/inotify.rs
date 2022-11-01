use anyhow::Result;
use futures::channel::mpsc::Receiver;
use futures::stream;
use futures::{FutureExt, SinkExt, StreamExt};
use notify::Watcher;

use super::{BlockStream, BlockStreamConfig, Renderer};

static DEBOUNCE_TIME: std::time::Duration = std::time::Duration::from_millis(10);

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
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
        renderer: Renderer,
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
        renderer
            .lock()
            .unwrap()
            .register_template_string(&name, template)?;
        Ok(Self {
            name,
            file,
            rx,
            renderer,
            _watcher: watcher,
        })
    }

    fn render(&self) -> Result<String> {
        let contents = match std::fs::read_to_string(&self.file) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "".to_string(),
            Err(error) => Err(error)?,
        };
        let data = BlockData { contents };
        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
        Ok(output)
    }

    async fn wait_for_update(&mut self) {
        loop {
            let mut results = vec![self.rx.next().await.expect("inotify event stream ended")];
            tokio::time::sleep(DEBOUNCE_TIME).await;
            while let Some(result) = self.rx.next().now_or_never() {
                results.push(result.expect("inotify event stream ended"));
            }
            for result in results {
                match result {
                    Ok(event) => {
                        for path in event.paths {
                            if path == self.file {
                                return;
                            }
                        }
                    }
                    Err(error) => eprintln!("Error watching files: {:?}", error),
                }
            }
        }
    }
}

impl BlockStreamConfig for crate::config::InotifyConfig {
    fn to_stream<'a>(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{contents}}".to_string());
        let state = Block::new(name.clone(), template, self.file, renderer)?;
        let initial_contents: String = state.render()?;
        let first_run = stream::once(async { (name, initial_contents) });
        let stream = stream::unfold(state, move |mut state| async {
            state.wait_for_update().await;
            let output = match state.render() {
                Ok(output) => output,
                Err(error) => {
                    eprintln!("Error rendering template: {:?}", error);
                    "Error".to_string()
                }
            };
            Some(((state.name.clone(), output), state))
        });
        Ok(Box::pin(first_run.chain(stream)))
    }
}
