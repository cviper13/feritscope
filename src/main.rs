mod config;
mod network;
mod radar;
mod state;
mod types;
mod ui;

use anyhow::Result;
use eframe::egui;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing_subscriber::{ layer::SubscriberExt, util::SubscriberInitExt };

use crate::config::ConfigWatcher;
use crate::network::NetworkManager;
use crate::state::RadarState;
use crate::ui::RadarApp;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber
        ::registry()
        .with(
            tracing_subscriber::EnvFilter
                ::try_from_default_env()
                .unwrap_or_else(|_| "atc24_radar=debug,info".into())
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting ATC 24 Radar Client");

    // Create Tokio runtime for async operations
    let runtime = Arc::new(Runtime::new()?);

    // Initialize shared state
    let radar_state = Arc::new(RadarState::new());

    // Load initial configuration
    let config = config::load_config()?;
    tracing::info!("Configuration loaded successfully");

    // Start config file watcher
    let config_watcher = ConfigWatcher::new(radar_state.clone());
    runtime.spawn(async move {
        if let Err(e) = config_watcher.watch().await {
            tracing::error!("Config watcher error: {}", e);
        }
    });

    // Start network manager for WebSocket and REST API
    let network_manager = NetworkManager::new(radar_state.clone());
    runtime.spawn(async move {
        network_manager.run().await;
    });

    // Configure and run the GUI
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder
            ::default()
            .with_inner_size([1920.0, 1080.0])
            .with_min_inner_size([1280.0, 720.0])
            .with_title("ATC 24 Radar - TopSky Interface")
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe
        ::run_native(
            "ATC 24 Radar",
            native_options,
            Box::new(|cc| {
                // Configure fonts
                configure_fonts(&cc.egui_ctx);

                Ok(Box::new(RadarApp::new(cc, radar_state, config, runtime)))
            })
        )
        .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

/// Configure custom fonts for the radar display
fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Use built-in proportional font for monospace family
    // This ensures compatibility across all platforms
    fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "Hack".to_owned());

    ctx.set_fonts(fonts);
}

/// Load application icon
fn load_icon() -> egui::IconData {
    // Placeholder - replace with actual icon
    egui::IconData {
        rgba: vec![255; 32 * 32 * 4],
        width: 32,
        height: 32,
    }
}
