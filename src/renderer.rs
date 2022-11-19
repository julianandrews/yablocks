use std::sync::{Arc, Mutex};

use anyhow::Result;
use once_cell::sync::Lazy;

pub static RENDERER: Lazy<Renderer> = Lazy::new(Renderer::new);

#[derive(Debug, Clone)]
pub struct Renderer {
    tera: Arc<Mutex<tera::Tera>>,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            tera: Arc::new(Mutex::new(tera::Tera::default())),
        }
    }

    pub fn add_template(&self, name: &str, template: &str) -> Result<()> {
        self.tera.lock().unwrap().add_raw_template(name, template)?;

        Ok(())
    }

    pub fn render(&self, name: &str, data: impl serde::Serialize) -> Result<String> {
        let context = tera::Context::from_serialize(&data)?;
        let rendered = self.tera.lock().unwrap().render(name, &context)?;

        Ok(rendered)
    }
}
