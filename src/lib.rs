mod errors;
mod logging;

use errors::ConfigError;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use slog::{debug, warn, Logger};
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


    pub async fn listen_for_updates(&self) -> Result<ServerConfig, ConfigError> {
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
                    // Keep logging all received SSE data for transparency
                    info!(self.logger, "Received SSE data"; "data" => &text);

                    // Check specifically for data messages
                    if text.starts_with("data: ") {
                        // Trim "data: " prefix and any leading/trailing whitespace
                        let json_part = text.trim_start_matches("data: ").trim();

                        match serde_json::from_str::<ServerConfig>(json_part) {
                            Ok(config) => {
                                let mut config_lock = self.current_config.lock().unwrap();
                                *config_lock = Some(config.clone());
                                // Log successful configuration update
                                info!(self.logger, "Updated configuration"; "config" => format!("{:?}", config));
                                return Ok(config); // Return the updated configuration
                            },
                            Err(e) => {
                                // Log failure to parse as a configuration update
                                error!(self.logger, "Failed to parse configuration data"; "error" => e.to_string());
                            }
                        }
                    } else if text.trim().is_empty() || text.starts_with(":") {
                        // Log or ignore keep-alive and other non-data messages
                        // For example, you might log these at a debug level if you want to keep track of them
                        debug!(self.logger, "Non-data message received"; "message" => &text);
                    } else {
                        // Handle or log unexpected message formats
                        warn!(self.logger, "Unexpected SSE message format"; "message" => &text);
                    }
                }
                Err(e) => {
                    error!(self.logger, "Error processing SSE data"; "error" => format!("{:?}", e));
                    return Err(e.into());
                },
            }
        }

        // If the loop ends without returning the config, return an error indicating no configuration was received
        Err(ConfigError::NoConfigReceived)
    }


    // Fetch the current configuration if available
    pub fn get_current_config(&self) -> Option<ServerConfig> {
        let config_lock = self.current_config.lock().unwrap();
        match *config_lock {
            Some(ref config) => {
                info!(self.logger, "Retrieving current configuration"; "config" => format!("{:?}", config));
                Some(config.clone())
            },
            None => {
                warn!(self.logger, "No current configuration available");
                None
            },
        }
    }

}