mod errors;
mod logging;

use errors::ConfigError;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use slog::Logger;
use serde_json::Value;
use slog::{error, info};

use std::{collections::BTreeMap, sync::{Arc, Mutex}};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub settings: BTreeMap<String, Value>,
}
pub struct ConfigSdk {
    config_endpoint: String,
    current_config: Arc<Mutex<Option<ServerConfig>>>,
    logger: Logger,
}

impl ConfigSdk {
    pub fn new(config_endpoint: &str) -> Self {
        Self {
            config_endpoint: config_endpoint.to_string(),
            current_config: Arc::new(Mutex::new(None)),
            logger: logging::configure_logging(),
        }
    }

    pub async fn listen_for_updates(&self) -> Result<(), ConfigError> {
        let client = Client::new();
        let response = client
            .get(&self.config_endpoint)
            .header("Accept", "text/event-stream")
            .send()
            .await?;

        let mut lines = response.bytes_stream();

        while let Some(item) = lines.next().await {
            match item {
                Ok(bytes) => {
                    let text = String::from_utf8(bytes.to_vec())?;
                    info!(self.logger, "Received SSE data"; "data" => &text);
                    if text.starts_with("data: ") {
                        let json_part = text.trim_start_matches("data: ");
                        if let Ok(config) = serde_json::from_str::<ServerConfig>(json_part) {
                            let mut config_lock = self.current_config.lock().unwrap();
                            *config_lock = Some(config.clone());
                            info!(self.logger, "Updated configuration"; "config" => format!("{:?}", config));
                        } else {
                            error!(self.logger, "Failed to parse configuration data");
                        }
                    }
                }
                Err(e) => {
                    error!(self.logger, "Error processing SSE data"; "error" => format!("{:?}", e));
                    return Err(e.into());
                },
            }
        }

        Ok(())
    }

    // Fetch the current configuration if available
    pub fn get_current_config(&self) -> Option<ServerConfig> {
        let config_lock = self.current_config.lock().unwrap();
        config_lock.clone()
    }
}