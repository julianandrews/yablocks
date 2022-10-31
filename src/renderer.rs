use std::sync::{Arc, Mutex};

use anyhow::Result;
use handlebars::Handlebars;

pub type Renderer = std::sync::Arc<std::sync::Mutex<handlebars::Handlebars<'static>>>;

pub fn build(template: String) -> Result<Renderer> {
    let mut handlebars = Handlebars::new();
    // Use the empty string for the root template to avoid conflicts with any block templates.
    handlebars.register_template_string("", template)?;
    Ok(Arc::new(Mutex::new(handlebars)))
}
