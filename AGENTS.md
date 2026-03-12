# AGENTS.md - Guidelines for Agentic Coding in yablocks

## Project Overview

yablocks is a Rust-based status bar generator that listens to various data sources and outputs formatted text for status bars like dzen2, xmobar, i3bar, and lemonbar. It uses the Tera templating engine for rendering.

## Build/Lint/Test Commands

```bash
# Build the project
cargo build
cargo build --release

# Run all tests
cargo test

# Run a single test by name
cargo test test_name

# Check code (compiles without running)
cargo check

# Format code
cargo fmt --all

# Check formatting (CI style)
cargo fmt --all -- --check

# Run clippy lints
cargo clippy -- -D warnings

# Run all CI checks locally
cargo check
cargo fmt --all -- --check
cargo clippy -- -D warnings
cargo test
```

## Code Style Guidelines

### General Conventions

- **Rust Edition**: 2021
- **Minimum Rust Version**: Stable (use recent stable)
- **Dependencies**: See `Cargo.toml` - avoid adding new dependencies without good reason

### Imports

```rust
// Standard library imports first
use std::collections::BTreeMap;

// Then external crate imports (alphabetically within group)
use anyhow::{Context, Result};
use futures::stream::select_all::select_all;
use futures::{FutureExt, StreamExt};

// Then local imports
use block_stream::BlockStreamConfig;
pub use renderer::RENDERER;
```

### Types and Naming

- **Types**: PascalCase (e.g., `BlockConfig`, `CpuConfig`, `RTSigNum`)
- **Functions/Variables**: snake_case (e.g., `to_stream`, `load_config`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEBOUNCE_TIME`)
- **Enums**: PascalCase variants (e.g., `BlockConfig::Command`)
- **Config Structs**: Use `#[serde(rename_all = "kebab-case")]` for toml compatibility

### Error Handling

- Use `anyhow::Result<T>` for fallible operations
- Use `anyhow::Context` for adding context to errors:
  ```rust
  config::load_config(args.configfile).context("Failed to load config")?
  ```
- Use `anyhow::bail!("error message")` for early returns with errors
- Print errors to stderr: `eprintln!("Error from {name}: {error:?}")`

### Async Patterns

- Use `tokio` for async runtime with `#[tokio::main]` macro
- Use `futures::StreamExt` for stream manipulation
- Block streams return `BoxStream<'static, (String, Result<String>)>` where the String is the block name

### Block Implementation Pattern

New blocks should follow this pattern:

1. **Config struct** in `src/config.rs`:
   ```rust
   #[derive(Deserialize, Debug, Clone)]
   #[serde(rename_all = "kebab-case")]
   pub struct NewBlockConfig {
       pub template: Option<String>,
       pub some_field: String,
   }
   ```

2. **Add variant** to `BlockConfig` enum in `src/config.rs`:
   ```rust
   #[derive(Deserialize, Debug, Clone)]
   #[serde(tag = "kind", rename_all = "kebab-case")]
   pub enum BlockConfig {
       // ... existing variants
       NewBlock(NewBlockConfig),
   }
   ```

3. **Implement BlockStreamConfig** in a new file under `src/block_stream/`:
   ```rust
   use anyhow::Result;
   use futures::stream;
   
   use super::{BlockStream, BlockStreamConfig};
   use crate::RENDERER;
   
   impl BlockStreamConfig for crate::config::NewBlockConfig {
       fn to_stream(self, name: String) -> Result<BlockStream> {
           let template = self.template.unwrap_or_else(|| "{{output}}".to_string());
           RENDERER.add_template(&name, &template)?;
           
           // Create block and stream
           let stream = stream::unfold(block, move |mut block| async {
               let result = block.wait_for_output().await?;
               Some(((block.name.clone(), result), block))
           });
           
           Ok(Box::pin(stream))
       }
   }
   ```

4. **Update match** in `src/block_stream.rs` to handle new variant

### Serialization

- Use `#[derive(serde::Serialize, Deserialize)]` for data structures
- Use `#[serde(rename_all = "kebab-case")]` for config structs
- Template data structs should derive `Serialize`

### Testing

- Place tests in the same file using `#[cfg(test)]` module
- Use `#[tokio::test]` for async tests
- Follow existing test patterns in the codebase

### Configuration File Format

- Config files use TOML format
- Block configurations use a `kind` discriminator field:
  ```toml
  [[blocks]]
  name = "cpu"
  kind = "cpu"
  interval = 5
  template = "{{cpu_times.non_idle | round(precision=1)}}%"
  ```

### Common Dependencies Used

- `anyhow` - Error handling
- `tokio` - Async runtime
- `futures` - Stream utilities
- `serde`/`toml`/`serde_json` - Serialization
- `clap` - CLI argument parsing
- `tera` - Templating

### CI Checks

The CI pipeline runs:
1. `cargo check` - Compilation check
2. `cargo fmt --all -- --check` - Formatting check
3. `cargo clippy -- -D warnings` - Linting (warnings become errors)

Ensure all CI checks pass before submitting code.
