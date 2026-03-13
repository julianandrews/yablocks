use std::time::Duration;

use anyhow::Result;
use futures::{stream, StreamExt};

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

impl BlockStreamConfig for crate::config::NoopConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        if let Some(template) = &self.template {
            RENDERER.add_template(&name, template)?;
        }

        let name_clone = name.clone();
        let first_run = stream::once({
            let name_clone = name_clone.clone();
            async move {
                let output = RENDERER.render(&name_clone, ()).unwrap_or_default();
                (name_clone, Ok(output))
            }
        });

        let stream = stream::repeat(()).then(move |_| {
            let name = name_clone.clone();
            async move {
                tokio::time::sleep(Duration::MAX).await;
                let output = RENDERER.render(&name, ()).unwrap_or_default();
                (name, Ok(output))
            }
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
