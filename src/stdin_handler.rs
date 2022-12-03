use anyhow::Result;

use crate::config::StdinHandler;

pub fn spawn_handler(handler: StdinHandler) -> Result<()> {
    eprintln!("Running {:?}", handler.command);
    tokio::process::Command::new(&handler.command)
        .args(&handler.args)
        .stdin(std::process::Stdio::inherit())
        .spawn()?;
    Ok(())
}
