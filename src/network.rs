use anyhow::{ Context, Result };
use futures_util::{ SinkExt, StreamExt };
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tokio_tungstenite::{ connect_async, tungstenite::Message };

use crate::state::RadarState;
use crate::types::{ AircraftDataMap, Atis, ControllerPosition, FlightPlan, WsMessage };

/// Network manager for WebSocket and REST API communication
pub struct NetworkManager {
    state: Arc<RadarState>,
}

impl NetworkManager {
    pub fn new(state: Arc<RadarState>) -> Self {
        Self { state }
    }

    /// Main run loop - manages WebSocket connection with auto-reconnect
    pub async fn run(self) {
        loop {
            let config = self.state.get_config();

            tracing::info!("Attempting to connect to WebSocket: {}", config.network.websocket_url);

            match self.connect_websocket(&config.network.websocket_url).await {
                Ok(_) => {
                    tracing::info!("WebSocket connection closed normally");
                }
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                }
            }

            self.state.set_websocket_connected(false);

            // Wait before reconnecting
            let delay = config.network.reconnect_delay_secs;
            tracing::info!("Reconnecting in {} seconds...", delay);
            time::sleep(Duration::from_secs(delay)).await;
        }
    }

    /// Connect to WebSocket and handle messages
    async fn connect_websocket(&self, url: &str) -> Result<()> {
        let (ws_stream, _) = connect_async(url).await.context("Failed to connect to WebSocket")?;

        self.state.set_websocket_connected(true);
        tracing::info!("WebSocket connected successfully");

        let (mut write, mut read) = ws_stream.split();

        // Send ping periodically to keep connection alive
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if state.get_connection_status().websocket_connected {
                    if let Err(e) = write.send(Message::Ping(vec![])).await {
                        tracing::error!("Failed to send ping: {}", e);
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.handle_message(&text).await {
                        tracing::error!("Error handling message: {}", e);
                    }
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                    // Handled automatically
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Parse and handle WebSocket message
    async fn handle_message(&self, text: &str) -> Result<()> {
        let msg: WsMessage = serde_json
            ::from_str(text)
            .context("Failed to parse WebSocket message")?;

        match msg.t.as_str() {
            "ACFT_DATA" => {
                let aircraft: AircraftDataMap = serde_json
                    ::from_value(msg.d)
                    .context("Failed to parse aircraft data")?;

                self.state.update_aircraft_batch(aircraft);
            }

            "EVENT_ACFT_DATA" => {
                let config = self.state.get_config();
                if config.network.enable_event_server {
                    let aircraft: AircraftDataMap = serde_json
                        ::from_value(msg.d)
                        .context("Failed to parse event aircraft data")?;

                    self.state.update_aircraft_batch(aircraft);
                }
            }

            "FLIGHT_PLAN" | "EVENT_FLIGHT_PLAN" => {
                let flight_plan: FlightPlan = serde_json
                    ::from_value(msg.d)
                    .context("Failed to parse flight plan")?;

                self.state.update_flight_plan(flight_plan);
            }

            "CONTROLLERS" => {
                let controllers: Vec<ControllerPosition> = serde_json
                    ::from_value(msg.d)
                    .context("Failed to parse controller positions")?;

                self.state.update_controllers(controllers);
            }

            "ATIS" => {
                let atis: Atis = serde_json::from_value(msg.d).context("Failed to parse ATIS")?;

                self.state.update_atis(atis);
            }

            unknown => {
                tracing::warn!("Unknown event type: {}", unknown);
            }
        }

        Ok(())
    }
}

/// REST API client for polling endpoints
pub struct RestClient {
    base_url: String,
    client: reqwest::Client,
}

impl RestClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client
                ::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch aircraft data from REST endpoint
    pub async fn get_aircraft_data(&self) -> Result<AircraftDataMap> {
        let url = format!("{}/acft-data", self.base_url);

        let resp = self.client.get(&url).send().await.context("Failed to fetch aircraft data")?;

        resp.json().await.context("Failed to parse aircraft data")
    }

    /// Fetch controller positions
    pub async fn get_controllers(&self) -> Result<Vec<ControllerPosition>> {
        let url = format!("{}/controllers", self.base_url);

        let resp = self.client.get(&url).send().await.context("Failed to fetch controllers")?;

        resp.json().await.context("Failed to parse controllers")
    }

    /// Fetch ATIS data
    pub async fn get_atis(&self) -> Result<Vec<Atis>> {
        let url = format!("{}/atis", self.base_url);

        let resp = self.client.get(&url).send().await.context("Failed to fetch ATIS")?;

        resp.json().await.context("Failed to parse ATIS")
    }

    /// Check if Discord user is a controller
    pub async fn is_controller(&self, discord_id: &str) -> Result<bool> {
        let url = format!("{}/is-controller/{}", self.base_url, discord_id);

        let resp = self.client.get(&url).send().await.context("Failed to check controller status")?;

        resp.json().await.context("Failed to parse controller status")
    }
}
