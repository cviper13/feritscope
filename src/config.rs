use anyhow::{ Context, Result };
use notify::{ Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher };
use serde::{ Deserialize, Serialize };
use std::path::{ Path, PathBuf };
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::state::RadarState;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RadarConfig {
    #[serde(default)]
    pub display: DisplayConfig,

    #[serde(default)]
    pub colors: ColorConfig,

    #[serde(default)]
    pub data_tags: DataTagConfig,

    #[serde(default)]
    pub performance: PerformanceConfig,

    #[serde(default)]
    pub network: NetworkConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Target symbol scale multiplier
    #[serde(default = "default_target_scale")]
    pub target_scale: f32,

    /// Target symbol stroke width
    #[serde(default = "default_target_stroke")]
    pub target_stroke: f32,

    /// Font size for data tags
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// Number of history dots to show (trail length)
    #[serde(default = "default_history_length")]
    pub history_length: usize,

    /// History dot size in pixels
    #[serde(default = "default_history_dot_size")]
    pub history_dot_size: f32,

    /// Predictive vector duration in minutes
    #[serde(default = "default_vector_minutes")]
    pub vector_minutes: f32,

    /// Show predictive vectors
    #[serde(default = "default_true")]
    pub show_vectors: bool,

    /// Show history trails
    #[serde(default = "default_true")]
    pub show_history: bool,

    /// Show data tags
    #[serde(default = "default_true")]
    pub show_tags: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorConfig {
    /// Background color (hex)
    #[serde(default = "default_bg_color")]
    pub background: String,

    /// Normal aircraft target color
    #[serde(default = "default_target_color")]
    pub target: String,

    /// Selected aircraft color
    #[serde(default = "default_selected_color")]
    pub target_selected: String,

    /// Emergency aircraft color
    #[serde(default = "default_emergency_color")]
    pub target_emergency: String,

    /// Data tag text color
    #[serde(default = "default_tag_color")]
    pub tag_text: String,

    /// History trail color
    #[serde(default = "default_history_color")]
    pub history: String,

    /// Predictive vector color
    #[serde(default = "default_vector_color")]
    pub vector: String,

    /// Ground track color
    #[serde(default = "default_ground_color")]
    pub ground: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataTagConfig {
    /// Offset from target symbol (x, y)
    #[serde(default = "default_tag_offset")]
    pub offset: (f32, f32),

    /// Line spacing between tag lines
    #[serde(default = "default_line_spacing")]
    pub line_spacing: f32,

    /// Template for line 1
    /// Available variables: {callsign}, {altitude}, {speed}, {gs}, {heading}, {type}
    #[serde(default = "default_line1")]
    pub line1: String,

    /// Template for line 2
    #[serde(default = "default_line2")]
    pub line2: String,

    /// Template for line 3 (optional)
    #[serde(default)]
    pub line3: Option<String>,

    /// Template for line 4 (optional)
    #[serde(default)]
    pub line4: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// Target FPS
    #[serde(default = "default_fps")]
    pub target_fps: u32,

    /// Max aircraft to render
    #[serde(default = "default_max_aircraft")]
    pub max_aircraft: usize,

    /// Enable anti-aliasing
    #[serde(default = "default_true")]
    pub anti_aliasing: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    /// WebSocket URL
    #[serde(default = "default_ws_url")]
    pub websocket_url: String,

    /// REST API base URL
    #[serde(default = "default_api_url")]
    pub api_base_url: String,

    /// Reconnection delay in seconds
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay_secs: u64,

    /// Enable main server data
    #[serde(default = "default_true")]
    pub enable_main_server: bool,

    /// Enable event server data
    #[serde(default = "default_false")]
    pub enable_event_server: bool,
}

// Default value functions
fn default_target_scale() -> f32 {
    1.0
}
fn default_target_stroke() -> f32 {
    2.0
}
fn default_font_size() -> f32 {
    12.0
}
fn default_history_length() -> usize {
    20
}
fn default_history_dot_size() -> f32 {
    2.0
}
fn default_vector_minutes() -> f32 {
    3.0
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

fn default_bg_color() -> String {
    "#0A0E1A".to_string()
}
fn default_target_color() -> String {
    "#00FF00".to_string()
}
fn default_selected_color() -> String {
    "#FFD700".to_string()
}
fn default_emergency_color() -> String {
    "#FF0000".to_string()
}
fn default_tag_color() -> String {
    "#00FF00".to_string()
}
fn default_history_color() -> String {
    "#00AA00".to_string()
}
fn default_vector_color() -> String {
    "#0088FF".to_string()
}
fn default_ground_color() -> String {
    "#888888".to_string()
}

fn default_tag_offset() -> (f32, f32) {
    (15.0, -10.0)
}
fn default_line_spacing() -> f32 {
    14.0
}
fn default_line1() -> String {
    "{callsign}".to_string()
}
fn default_line2() -> String {
    "F{altitude:03} {gs:03}KT".to_string()
}

fn default_fps() -> u32 {
    60
}
fn default_max_aircraft() -> usize {
    500
}

fn default_ws_url() -> String {
    "wss://24data.ptfs.app/wss".to_string()
}
fn default_api_url() -> String {
    "https://24data.ptfs.app".to_string()
}
fn default_reconnect_delay() -> u64 {
    5
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            target_scale: default_target_scale(),
            target_stroke: default_target_stroke(),
            font_size: default_font_size(),
            history_length: default_history_length(),
            history_dot_size: default_history_dot_size(),
            vector_minutes: default_vector_minutes(),
            show_vectors: default_true(),
            show_history: default_true(),
            show_tags: default_true(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            background: default_bg_color(),
            target: default_target_color(),
            target_selected: default_selected_color(),
            target_emergency: default_emergency_color(),
            tag_text: default_tag_color(),
            history: default_history_color(),
            vector: default_vector_color(),
            ground: default_ground_color(),
        }
    }
}

impl Default for DataTagConfig {
    fn default() -> Self {
        Self {
            offset: default_tag_offset(),
            line_spacing: default_line_spacing(),
            line1: default_line1(),
            line2: default_line2(),
            line3: None,
            line4: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            target_fps: default_fps(),
            max_aircraft: default_max_aircraft(),
            anti_aliasing: default_true(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            websocket_url: default_ws_url(),
            api_base_url: default_api_url(),
            reconnect_delay_secs: default_reconnect_delay(),
            enable_main_server: default_true(),
            enable_event_server: default_false(),
        }
    }
}

impl Default for RadarConfig {
    fn default() -> Self {
        Self {
            display: DisplayConfig::default(),
            colors: ColorConfig::default(),
            data_tags: DataTagConfig::default(),
            performance: PerformanceConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

/// Get the config file path
pub fn config_path() -> PathBuf {
    PathBuf::from("config.toml")
}

/// Load configuration from file or create default
pub fn load_config() -> Result<RadarConfig> {
    let path = config_path();

    if path.exists() {
        let contents = std::fs::read_to_string(&path).context("Failed to read config.toml")?;

        toml::from_str(&contents).context("Failed to parse config.toml")
    } else {
        tracing::warn!("config.toml not found, creating default configuration");
        let config = RadarConfig::default();
        save_config(&config)?;
        Ok(config)
    }
}

/// Save configuration to file
pub fn save_config(config: &RadarConfig) -> Result<()> {
    let contents = toml::to_string_pretty(config).context("Failed to serialize config")?;

    std::fs::write(config_path(), contents).context("Failed to write config.toml")?;

    Ok(())
}

/// Configuration file watcher with hot-reload
pub struct ConfigWatcher {
    state: Arc<RadarState>,
}

impl ConfigWatcher {
    pub fn new(state: Arc<RadarState>) -> Self {
        Self { state }
    }

    /// Start watching the config file for changes
    pub async fn watch(self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(1);

        let mut watcher = RecommendedWatcher::new(move |res| {
            let _ = tx.blocking_send(res);
        }, NotifyConfig::default())?;

        watcher.watch(config_path().as_ref(), RecursiveMode::NonRecursive)?;

        tracing::info!("Config file watcher started");

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => {
                    tracing::debug!("Config file event: {:?}", event);

                    // Reload configuration
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    match load_config() {
                        Ok(config) => {
                            self.state.update_config(config);
                            tracing::info!("Configuration reloaded successfully");
                        }
                        Err(e) => {
                            tracing::error!("Failed to reload config: {}", e);
                        }
                    }
                }
                Err(e) => tracing::error!("Watch error: {:?}", e),
            }
        }

        Ok(())
    }
}
