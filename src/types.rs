use serde::{ Deserialize, Serialize };
use std::collections::HashMap;

/// WebSocket message envelope
#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    /// Event type
    pub t: String,
    /// Data payload (varies by event type)
    pub d: serde_json::Value,
    /// Timestamp (ISO 8601)
    pub s: Option<String>,
}

/// Aircraft data from the API
/// Note: This is a HashMap with callsign as key
pub type AircraftDataMap = HashMap<String, AircraftInfo>;

/// Individual aircraft information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AircraftInfo {
    /// Heading in degrees (0-360, 360 = North)
    pub heading: f64,

    /// Roblox username of the pilot
    #[serde(rename = "playerName")]
    pub player_name: String,

    /// Altitude in feet
    pub altitude: f64,

    /// Aircraft type (e.g., "Airbus A380")
    #[serde(rename = "aircraftType")]
    pub aircraft_type: String,

    /// Position in studs
    pub position: Position,

    /// Indicated airspeed in knots (game knots, not real)
    pub speed: f64,

    /// Wind direction/speed (e.g., "357/15")
    pub wind: String,

    /// Whether aircraft is on ground (taxiing mode)
    /// Note: Not present for helicopters
    #[serde(rename = "isOnGround")]
    pub is_on_ground: Option<bool>,

    /// Ground speed in knots (affected by altitude damping)
    #[serde(rename = "groundSpeed")]
    pub ground_speed: f64,

    /// Emergency status
    #[serde(rename = "isEmergencyOccuring")]
    pub is_emergency_occuring: bool,
}

/// Position in studs (Roblox coordinate system)
/// Note: -y is North, -x is West
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Flight plan information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlightPlan {
    #[serde(rename = "robloxName")]
    pub roblox_name: String,

    pub callsign: String,

    #[serde(rename = "realcallsign")]
    pub real_callsign: String,

    pub aircraft: String,

    #[serde(rename = "flightrules")]
    pub flight_rules: String, // "IFR" or "VFR"

    pub departing: String, // Airport ICAO

    pub arriving: String, // Airport ICAO

    pub route: String,

    #[serde(rename = "flightlevel")]
    pub flight_level: String,
}

/// ATC Controller position information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ControllerPosition {
    /// Roblox username of position holder (null if unclaimed)
    pub holder: Option<String>,

    /// Unix timestamp (ms) when position was claimed
    #[serde(rename = "heldSince")]
    pub held_since: Option<u64>,

    /// Whether position can be claimed
    pub claimable: bool,

    /// Airport ICAO or Area Control Centre
    pub airport: String,

    /// Position type: GND, TWR, CTR
    pub position: String,

    /// Queue of usernames waiting for position
    pub queue: Vec<String>,
}

/// ATIS information for an airport
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Atis {
    /// Airport ICAO code
    pub airport: String,

    /// ATIS designator letter
    pub letter: String,

    /// Full ATIS content (with \n)
    pub content: String,

    /// ATIS split into lines
    pub lines: Vec<String>,

    /// Roblox username of last editor (can be null)
    pub editor: Option<String>,
}

/// Internal state for a tracked aircraft with history
#[derive(Debug, Clone)]
pub struct TrackedAircraft {
    /// Callsign
    pub callsign: String,

    /// Current aircraft info
    pub info: AircraftInfo,

    /// Associated flight plan (if any)
    pub flight_plan: Option<FlightPlan>,

    /// History trail positions (for drawing "comet tail")
    /// Stores (x, y, timestamp) tuples
    pub history: Vec<(f64, f64, i64)>,

    /// Last update timestamp
    pub last_update: i64,

    /// Emergency flash state (for animation)
    pub emergency_flash: bool,
}

impl TrackedAircraft {
    pub fn new(callsign: String, info: AircraftInfo) -> Self {
        Self {
            callsign,
            info,
            flight_plan: None,
            history: Vec::new(),
            last_update: chrono::Utc::now().timestamp_millis(),
            emergency_flash: false,
        }
    }

    /// Update aircraft info and add to history trail
    pub fn update(&mut self, info: AircraftInfo, max_history: usize) {
        let now = chrono::Utc::now().timestamp_millis();

        // Add current position to history if it's different enough
        if self.should_add_history(&info) {
            self.history.push((self.info.position.x, self.info.position.y, self.last_update));

            // Trim history to max size
            if self.history.len() > max_history {
                self.history.remove(0);
            }
        }

        self.info = info;
        self.last_update = now;
    }

    /// Determine if we should add a new history point
    fn should_add_history(&self, new_info: &AircraftInfo) -> bool {
        let dx = new_info.position.x - self.info.position.x;
        let dy = new_info.position.y - self.info.position.y;
        let distance = (dx * dx + dy * dy).sqrt();

        // Add point if moved more than 100 studs (~54 meters)
        distance > 100.0
    }
}
