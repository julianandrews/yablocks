use std::sync::{Arc, Mutex};

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Renderer {
    tera: Arc<Mutex<tera::Tera>>,
}

impl Renderer {
    pub fn new(template: &str) -> Result<Self> {
        let mut tera = tera::Tera::default();
        // Use the empty string for the root template to avoid conflicts with any block templates.
        tera.add_raw_template("", template)?;

        Ok(Self {
            tera: Arc::new(Mutex::new(tera)),
        })
    }

    pub fn add_template(&mut self, name: &str, template: &str) -> Result<()> {
        self.tera.lock().unwrap().add_raw_template(name, template)?;

        Ok(())
    }

    pub fn render(&self, name: &str, data: impl serde::Serialize) -> Result<String> {
        let context = tera::Context::from_serialize(&data)?;
        let rendered = self.tera.lock().unwrap().render(name, &context)?;

        Ok(rendered)
    }
}
