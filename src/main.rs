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
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::ConfigWatcher;
use crate::network::NetworkManager;
use crate::state::RadarState;
use crate::ui::RadarApp;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "atc24_radar=debug,info".into()),
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
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.0])
            .with_min_inner_size([1280.0, 720.0])
            .with_title("ATC 24 Radar - TopSky Interface")
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "ATC 24 Radar",
        native_options,
        Box::new(move |cc| {
            // Configure fonts with user's config
            configure_fonts(&cc.egui_ctx, &config);
            
            Ok(Box::new(RadarApp::new(
                cc,
                radar_state,
                config,
                runtime,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

/// Configure custom fonts for the radar display
pub fn configure_fonts(ctx: &egui::Context, config: &config::RadarConfig) {
    use std::fs;
    
    let mut fonts = egui::FontDefinitions::default();
    let mut fonts_loaded = false;
    
    // Try to load custom radar font
    if let Some(radar_font_path) = &config.fonts.radar_font {
        match fs::read(radar_font_path) {
            Ok(font_data) => {
                tracing::info!("Loaded custom radar font from: {}", radar_font_path);
                fonts.font_data.insert(
                    "radar_custom".to_owned(),
                    egui::FontData::from_owned(font_data),
                );
                
                // Set as primary monospace font
                fonts.families.get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .insert(0, "radar_custom".to_owned());
                
                fonts_loaded = true;
            }
            Err(e) => {
                tracing::warn!("Failed to load radar font '{}': {}", radar_font_path, e);
            }
        }
    }
    
    // Try to load custom UI font
    if let Some(ui_font_path) = &config.fonts.ui_font {
        match fs::read(ui_font_path) {
            Ok(font_data) => {
                tracing::info!("Loaded custom UI font from: {}", ui_font_path);
                fonts.font_data.insert(
                    "ui_custom".to_owned(),
                    egui::FontData::from_owned(font_data),
                );
                
                // Set as primary proportional font
                fonts.families.get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "ui_custom".to_owned());
                
                fonts_loaded = true;
            }
            Err(e) => {
                tracing::warn!("Failed to load UI font '{}': {}", ui_font_path, e);
            }
        }
    }
    
    // Apply font configuration
    ctx.set_fonts(fonts);
    
    // Set font sizes
    let mut style = (*ctx.style()).clone();
    
    // Set UI text size
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(config.fonts.ui_font_size),
    );
    
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(config.fonts.ui_font_size),
    );
    
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(config.fonts.ui_font_size * 0.85),
    );
    
    // Set heading size
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(config.fonts.heading_font_size),
    );
    
    // Set monospace size (for radar tags, uses radar_font_size if specified, else display.font_size)
    let radar_font_size = config.fonts.radar_font_size
        .unwrap_or(config.display.font_size);
    
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(radar_font_size),
    );
    
    ctx.set_style(style);
    
    if fonts_loaded {
        tracing::info!("Custom fonts applied successfully");
    } else {
        tracing::info!("Using built-in fonts");
    }
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