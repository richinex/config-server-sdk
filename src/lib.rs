mod errors;
mod logging;
mod appstate;

use std::collections::BTreeMap;

use errors::ConfigError;
use futures::StreamExt;
use std::sync::{Arc, Mutex};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use slog::{debug, warn, Logger};
use serde_json::Value;
use slog::{error, info};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub settings: BTreeMap<String, Value>,
}
pub struct ConfigSdk {
    config_endpoint: String,
    logger: Logger,
    current_config: Arc<Mutex<Option<ServerConfig>>>, // Store the latest config here
}

impl ConfigSdk {
    pub fn new(config_endpoint: &str) -> Self {
        Self {
            config_endpoint: config_endpoint.to_string(),
            logger: logging::configure_logging(),
            current_config: Arc::new(Mutex::new(None)),
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
                        let json_part = text.trim_start_matches("data: ").trim();
                        match serde_json::from_str::<ServerConfig>(json_part) {
                            Ok(config) => {
                                // Update the current configuration
                                let mut config_lock = self.current_config.lock().unwrap();
                                *config_lock = Some(config.clone());
                                info!(self.logger, "Updated configuration"; "config" => format!("{:?}", config));
                            },
                            Err(e) => {
                                error!(self.logger, "Failed to parse configuration data"; "error" => e.to_string());
                            }
                        }
                    } else if text.trim().is_empty() || text.starts_with(":") {
                        debug!(self.logger, "Non-data message received"; "message" => &text);
                    } else {
                        warn!(self.logger, "Unexpected SSE message format"; "message" => &text);
                    }
                },
                Err(e) => {
                    error!(self.logger, "Error processing SSE data"; "error" => format!("{:?}", e));
                    return Err(e.into());
                },
            }
        }

        Ok(())
    }

    // Public method to fetch the current configuration
    pub fn get_current_config(&self) -> Option<ServerConfig> {
        let config_lock = self.current_config.lock().unwrap();
        (*config_lock).clone()
    }
}