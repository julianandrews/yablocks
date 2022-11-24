use anyhow::Result;
use futures::stream;

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;
use procfs::{CpuTime, KernelStats};

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    interval: u64,
    cpu_times: NormalizedCpuTimes,
}

struct Block {
    name: String,
    interval: u64,
    cpu_time: Option<CpuTime>,
}

impl Block {
    fn new(name: String, interval: u64) -> Self {
        Self {
            name,
            interval,
            cpu_time: KernelStats::new().map(|stats| stats.total).ok(),
        }
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;
        let old_cpu_time = match &self.cpu_time {
            Some(cpu_time) => cpu_time,
            None => {
                self.cpu_time = match KernelStats::new() {
                    Ok(stats) => Some(stats.total),
                    Err(e) => return Some(Err(anyhow::Error::from(e))),
                };
                // Wait another interval so that we get a good sample of CPU usage data
                tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;
                self.cpu_time.as_ref().unwrap()
            }
        };
        let new_cpu_time = match KernelStats::new() {
            Ok(stats) => stats.total,
            Err(e) => return Some(Err(anyhow::Error::from(e))),
        };
        let cpu_times = NormalizedCpuTimes::from_diff(old_cpu_time, &new_cpu_time);
        let data = BlockData {
            interval: self.interval,
            cpu_times,
        };

        let rendered = RENDERER.render(&self.name, data);
        self.cpu_time = Some(new_cpu_time);
        Some(rendered)
    }
}

impl BlockStreamConfig for crate::config::CpuConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self
            .template
            .unwrap_or_else(|| "{{cpu_times.non_idle | round(precision=1)}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block::new(name, self.interval);
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct NormalizedCpuTimes {
    non_idle: f64,
    user: f64,
    nice: f64,
    system: f64,
    idle: f64,
    iowait: f64,
    irq: f64,
    softirq: f64,
    steal: f64,
    guest: f64,
    guest_nice: f64,
}

impl NormalizedCpuTimes {
    fn from_diff(old: &CpuTime, new: &CpuTime) -> Self {
        // All of these variables are in ticks.
        let user = (new.user - old.user) as f64;
        let nice = (new.nice - old.nice) as f64;
        let system = (new.system - old.system) as f64;
        let idle = (new.idle - old.idle) as f64;
        let iowait = (new.iowait.unwrap_or(0) - old.iowait.unwrap_or(0)) as f64;
        let irq = (new.irq.unwrap_or(0) - old.irq.unwrap_or(0)) as f64;
        let softirq = (new.softirq.unwrap_or(0) - old.softirq.unwrap_or(0)) as f64;
        let steal = (new.steal.unwrap_or(0) - old.steal.unwrap_or(0)) as f64;
        let guest = (new.guest.unwrap_or(0) - old.guest.unwrap_or(0)) as f64;
        let guest_nice = (new.guest_nice.unwrap_or(0) - old.guest_nice.unwrap_or(0)) as f64;
        let non_idle = user + nice + system + irq + softirq + steal + guest + guest_nice;

        // Normalize so that the sum of all ticks gives 100
        let scale = 100.0 / (non_idle + idle + iowait);

        Self {
            non_idle: non_idle * scale,
            user: user * scale,
            nice: nice * scale,
            system: system * scale,
            idle: idle * scale,
            iowait: iowait * scale,
            irq: irq * scale,
            softirq: softirq * scale,
            steal: steal * scale,
            guest: guest * scale,
            guest_nice: guest_nice * scale,
        }
    }
}
