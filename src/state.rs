use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::RadarConfig;
use crate::types::{ Atis, ControllerPosition, FlightPlan, TrackedAircraft };

/// Thread-safe radar state shared between GUI and network threads
pub struct RadarState {
    /// Aircraft currently being tracked
    aircraft: RwLock<HashMap<String, TrackedAircraft>>,

    /// Active ATC controller positions
    controllers: RwLock<Vec<ControllerPosition>>,

    /// ATIS information by airport
    atis: RwLock<HashMap<String, Atis>>,

    /// Configuration (hot-reloadable)
    config: RwLock<RadarConfig>,

    /// Connection status
    connection_status: RwLock<ConnectionStatus>,
}

#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub websocket_connected: bool,
    pub last_data_received: Option<i64>,
    pub aircraft_count: usize,
    pub event_aircraft_count: usize,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self {
            websocket_connected: false,
            last_data_received: None,
            aircraft_count: 0,
            event_aircraft_count: 0,
        }
    }
}

impl RadarState {
    pub fn new() -> Self {
        Self {
            aircraft: RwLock::new(HashMap::new()),
            controllers: RwLock::new(Vec::new()),
            atis: RwLock::new(HashMap::new()),
            config: RwLock::new(RadarConfig::default()),
            connection_status: RwLock::new(ConnectionStatus::default()),
        }
    }

    // Aircraft management

    /// Update aircraft data from API
    pub fn update_aircraft_batch(&self, aircraft_map: HashMap<String, crate::types::AircraftInfo>) {
        let mut aircraft = self.aircraft.write();
        let config = self.config.read();
        let max_history = config.display.history_length;

        // Update existing and add new aircraft
        for (callsign, info) in aircraft_map {
            aircraft
                .entry(callsign.clone())
                .and_modify(|tracked| tracked.update(info.clone(), max_history))
                .or_insert_with(|| TrackedAircraft::new(callsign, info));
        }

        // Update connection status
        let mut status = self.connection_status.write();
        status.aircraft_count = aircraft.len();
        status.last_data_received = Some(chrono::Utc::now().timestamp_millis());
    }

    /// Get all tracked aircraft (read-only)
    pub fn get_aircraft(&self) -> HashMap<String, TrackedAircraft> {
        self.aircraft.read().clone()
    }

    /// Get specific aircraft by callsign
    pub fn get_aircraft_by_callsign(&self, callsign: &str) -> Option<TrackedAircraft> {
        self.aircraft.read().get(callsign).cloned()
    }

    /// Clear stale aircraft (not updated in last N seconds)
    pub fn clear_stale_aircraft(&self, max_age_secs: i64) {
        let mut aircraft = self.aircraft.write();
        let now = chrono::Utc::now().timestamp_millis();

        aircraft.retain(|_, tracked| { now - tracked.last_update < max_age_secs * 1000 });
    }

    /// Associate flight plan with aircraft
    pub fn update_flight_plan(&self, flight_plan: FlightPlan) {
        let mut aircraft = self.aircraft.write();

        if let Some(tracked) = aircraft.get_mut(&flight_plan.callsign) {
            tracked.flight_plan = Some(flight_plan);
        }
    }

    // Controller management

    /// Update controller positions
    pub fn update_controllers(&self, positions: Vec<ControllerPosition>) {
        *self.controllers.write() = positions;
    }

    /// Get all controller positions
    pub fn get_controllers(&self) -> Vec<ControllerPosition> {
        self.controllers.read().clone()
    }

    // ATIS management

    /// Update ATIS for an airport
    pub fn update_atis(&self, atis: Atis) {
        self.atis.write().insert(atis.airport.clone(), atis);
    }

    /// Get ATIS for specific airport
    pub fn get_atis(&self, airport: &str) -> Option<Atis> {
        self.atis.read().get(airport).cloned()
    }

    /// Get all ATIS
    pub fn get_all_atis(&self) -> HashMap<String, Atis> {
        self.atis.read().clone()
    }

    // Configuration management

    /// Update configuration (hot-reload)
    pub fn update_config(&self, config: RadarConfig) {
        *self.config.write() = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> RadarConfig {
        self.config.read().clone()
    }

    // Connection status

    /// Update WebSocket connection status
    pub fn set_websocket_connected(&self, connected: bool) {
        self.connection_status.write().websocket_connected = connected;
    }

    /// Get connection status
    pub fn get_connection_status(&self) -> ConnectionStatus {
        self.connection_status.read().clone()
    }
}

impl Default for RadarState {
    fn default() -> Self {
        Self::new()
    }
}
