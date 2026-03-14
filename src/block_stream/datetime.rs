use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use chrono::{Datelike, Offset, TimeZone, Timelike, Utc};
use chrono_tz::OffsetName;
use futures::{stream, StreamExt};

use super::{BlockStream, BlockStreamConfig};
use crate::config::Precision;
use crate::RENDERER;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    timestamp: i64,
    year: i32,
    month: u32,
    month_name: String,
    day: u32,
    hour: u32,
    hour_12: u32,
    minute: u32,
    second: u32,
    am_pm: String,
    weekday: u32,
    weekday_name: String,
    utc_offset: i32,
    timezone_abbreviation: String,
}

impl BlockData {
    fn now(timezone: Option<chrono_tz::Tz>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp = now.as_secs() as i64;

        let tz = timezone.unwrap_or_else(|| {
            let name = iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string());
            name.parse()
                .unwrap_or_else(|_| "UTC".parse().expect("Invalid fallback timezone"))
        });

        let dt = Utc.timestamp_opt(timestamp, 0).unwrap().with_timezone(&tz);
        let offset = dt.offset();
        let utc_offset = offset.fix().local_minus_utc();
        let timezone_abbreviation = offset
            .abbreviation()
            .map(|s| s.to_string())
            .unwrap_or_default();

        BlockData {
            timestamp,
            year: dt.year(),
            month: dt.month(),
            month_name: dt.format("%B").to_string(),
            day: dt.day(),
            hour: dt.hour(),
            hour_12: dt.format("%I").to_string().parse().unwrap(),
            minute: dt.minute(),
            second: dt.second(),
            am_pm: dt.format("%p").to_string(),
            weekday: dt.weekday().num_days_from_sunday(),
            weekday_name: dt.format("%A").to_string(),
            utc_offset,
            timezone_abbreviation,
        }
    }
}

#[derive(Debug, Clone)]
struct Block {
    name: String,
    precision: Precision,
    timezone: Option<chrono_tz::Tz>,
}

impl Block {
    async fn wait_for_output(&self) -> Option<Result<String>> {
        let sleep_duration = next_boundary(self.precision);
        tokio::time::sleep(sleep_duration).await;
        Some(self.render())
    }

    fn render(&self) -> Result<String> {
        let data = BlockData::now(self.timezone);
        RENDERER.render(&self.name, data)
    }
}

impl BlockStreamConfig for crate::config::DateTimeConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self
            .template
            .unwrap_or_else(|| "{{hour}}:{{minute}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let block = Block {
            name: name.clone(),
            precision: self.precision,
            timezone: self.timezone,
        };

        let first_block = block.clone();
        let first_run = stream::once(async move {
            let output = first_block.render();
            (name, output)
        });

        let stream = stream::unfold(block, |block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}

fn next_boundary(precision: Precision) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let now_secs = now.as_secs();
    let now_nanos = now.subsec_nanos();

    let interval = match precision {
        Precision::Second => 1,
        Precision::Minute => 60,
        Precision::Hour => 3600,
        Precision::Day => 86400,
    };

    let elapsed_within_interval = now_secs % interval;
    let elapsed_nanos = elapsed_within_interval * 1_000_000_000 + now_nanos as u64;
    let interval_nanos = interval * 1_000_000_000;
    let remaining_nanos = interval_nanos - elapsed_nanos;

    Duration::from_nanos(remaining_nanos % interval_nanos)
}
