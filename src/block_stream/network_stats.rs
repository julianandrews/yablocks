use std::path::PathBuf;

use anyhow::Result;
use futures::stream;

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    rx_bytes_per_sec: f64,
    tx_bytes_per_sec: f64,
    rx_packets_per_sec: f64,
    tx_packets_per_sec: f64,
}

struct Block {
    name: String,
    device: String,
    interval: u64,
    prev_stats: Option<NetworkStats>,
}

#[derive(Debug, Clone)]
struct NetworkStats {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
}

impl Block {
    fn new(name: String, device: String, interval: u64) -> Self {
        Self {
            name,
            device,
            interval,
            prev_stats: None,
        }
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;

        let new_stats = match NetworkStats::read(&self.device).await {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };

        let data = match &self.prev_stats {
            Some(prev) => {
                let interval = self.interval as f64;
                let rx_bytes_diff = new_stats.rx_bytes.wrapping_sub(prev.rx_bytes);
                let tx_bytes_diff = new_stats.tx_bytes.wrapping_sub(prev.tx_bytes);
                let rx_packets_diff = new_stats.rx_packets.wrapping_sub(prev.rx_packets);
                let tx_packets_diff = new_stats.tx_packets.wrapping_sub(prev.tx_packets);

                BlockData {
                    rx_bytes_per_sec: rx_bytes_diff as f64 / interval,
                    tx_bytes_per_sec: tx_bytes_diff as f64 / interval,
                    rx_packets_per_sec: rx_packets_diff as f64 / interval,
                    tx_packets_per_sec: tx_packets_diff as f64 / interval,
                }
            }
            None => BlockData {
                rx_bytes_per_sec: 0.0,
                tx_bytes_per_sec: 0.0,
                rx_packets_per_sec: 0.0,
                tx_packets_per_sec: 0.0,
            },
        };

        let rendered = RENDERER.render(&self.name, data);
        self.prev_stats = Some(new_stats);
        Some(rendered)
    }
}

impl NetworkStats {
    async fn read(device: &str) -> Result<Self> {
        let base: PathBuf = ["/sys/class/net", device, "statistics"].iter().collect();

        let rx_bytes = tokio::fs::read_to_string(base.join("rx_bytes"))
            .await?
            .trim()
            .parse::<u64>()?;
        let tx_bytes = tokio::fs::read_to_string(base.join("tx_bytes"))
            .await?
            .trim()
            .parse::<u64>()?;
        let rx_packets = tokio::fs::read_to_string(base.join("rx_packets"))
            .await?
            .trim()
            .parse::<u64>()?;
        let tx_packets = tokio::fs::read_to_string(base.join("tx_packets"))
            .await?
            .trim()
            .parse::<u64>()?;

        Ok(Self {
            rx_bytes,
            tx_bytes,
            rx_packets,
            tx_packets,
        })
    }
}

impl BlockStreamConfig for crate::config::NetworkStatsConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self
            .template
            .unwrap_or_else(|| "{{rx_bytes_per_sec}} ↓ {{tx_bytes_per_sec}} ↑".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block::new(name.clone(), self.device.clone(), self.interval);
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}
