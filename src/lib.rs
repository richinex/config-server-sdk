mod errors;
mod logging;


use std::collections::BTreeMap;

use errors::ConfigError;
use futures::StreamExt;
use std::sync::{Arc, Mutex};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use slog::{debug, warn, Logger};
use serde_json::Value;
use slog::{error, info};
use tokio::time::{self, Duration};
use tokio::sync::Notify;



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub settings: BTreeMap<String, Value>,
}
pub struct ConfigSdk {
    config_endpoint: String,
    logger: Logger,
    current_config: Arc<Mutex<Option<ServerConfig>>>, // Store the latest config here
    notify: Arc<Notify>, // Notification system for config updates
}

impl ConfigSdk {
    pub fn new(config_endpoint: &str) -> Self {
        Self {
            config_endpoint: config_endpoint.to_string(),
            logger: logging::configure_logging(),
            current_config: Arc::new(Mutex::new(None)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn listen_for_updates(&self) -> Result<(), ConfigError> {
        let mut retry_count = 0;
        const MAX_RETRIES: usize = 5;
        let client = Client::new();

        loop {
            if retry_count >= MAX_RETRIES {
                error!(self.logger, "Failed to connect after {} retries.", MAX_RETRIES);
                return Err(ConfigError::GenericError("Failed to connect".into()));
            }

            let response = match client.get(&self.config_endpoint).header("Accept", "text/event-stream").send().await {
                Ok(response) => response,
                Err(e) => {
                    error!(self.logger, "Failed to connect to SSE server: {}", e);
                    retry_count += 1;
                    time::sleep(Duration::from_secs(10)).await;
                    continue;
                },
            };

            let mut lines = response.bytes_stream();

            while let Some(item) = lines.next().await {
                match item {
                    Ok(bytes) => {
                        let text = String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| "".to_string());
                        info!(self.logger, "Received SSE data"; "data" => &text);

                        if text.starts_with("data: ") {
                            let json_part = text.trim_start_matches("data: ").trim();
                            if let Ok(config) = serde_json::from_str::<ServerConfig>(json_part) {
                                let mut config_lock = self.current_config.lock().unwrap();
                                *config_lock = Some(config.clone());
                                self.notify.notify_waiters(); // Notify all waiting tasks
                                info!(self.logger, "Configuration updated"; "config" => format!("{:?}", config));
                                retry_count = 0;
                            } else {
                                error!(self.logger, "Failed to parse configuration data");
                            }
                        } else if text.trim().is_empty() || text.starts_with(":") {
                            debug!(self.logger, "Non-data message received"; "message" => &text);
                        } else {
                            warn!(self.logger, "Unexpected SSE message format"; "message" => &text);
                        }
                    },
                    Err(e) => {
                        error!(self.logger, "Error processing SSE data: {}", e);
                        break;
                    },
                }
            }

            retry_count += 1;
            warn!(self.logger, "Attempting to reconnect... Retry count: {}", retry_count);
            time::sleep(Duration::from_secs(10)).await;
        }
    }

    // Method to fetch the current configuration safely
    pub fn get_current_config(&self) -> Option<ServerConfig> {
        let config_lock = self.current_config.lock().unwrap();
        config_lock.clone()
    }
}