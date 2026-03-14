use std::collections::BTreeMap;

use anyhow::Result;
use futures::stream;
use sensors::{FeatureType, Sensors, SubfeatureType};

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

#[derive(serde::Serialize, Clone)]
struct SensorData {
    label: String,
    value: f64,
    chip: String,
}

struct Block {
    name: String,
    interval: u64,
}

impl Block {
    fn read_sensors(&self) -> BTreeMap<String, Vec<SensorData>> {
        let sensors = Sensors::new();

        let mut chips: BTreeMap<String, Vec<SensorData>> = BTreeMap::new();

        for chip in sensors {
            let chip_name = match chip.get_name() {
                Ok(name) => name,
                Err(_) => continue,
            };

            let chip_prefix = chip.prefix().to_string();

            let mut chip_sensors = Vec::new();

            for feature in chip {
                if *feature.feature_type() != FeatureType::SENSORS_FEATURE_TEMP {
                    continue;
                }

                let label = feature
                    .get_label()
                    .unwrap_or_else(|_err| feature.name().to_string());

                for subfeature in feature.into_iter() {
                    if *subfeature.subfeature_type()
                        == SubfeatureType::SENSORS_SUBFEATURE_TEMP_INPUT
                    {
                        if let Ok(value) = subfeature.get_value() {
                            chip_sensors.push(SensorData {
                                label,
                                value,
                                chip: chip_prefix.clone(),
                            });
                            break;
                        }
                    }
                }
            }

            if !chip_sensors.is_empty() {
                chips.insert(chip_name, chip_sensors);
            }
        }

        chips
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        tokio::time::sleep(std::time::Duration::from_secs(self.interval)).await;

        let chips = self.read_sensors();

        let rendered = match RENDERER.render(&self.name, serde_json::json!({ "chips": chips })) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        Some(Ok(rendered))
    }
}

impl BlockStreamConfig for crate::config::TemperatureConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| {
            "{{ chips | last | get(key=1) | first | get(attribute=\"value\")}}".to_string()
        });
        RENDERER.add_template(&name, &template)?;

        let block = Block {
            name,
            interval: self.interval,
        };
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}
