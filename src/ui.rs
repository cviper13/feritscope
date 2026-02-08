use eframe::egui;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;

use crate::config::RadarConfig;
use crate::radar::{ parse_color, Projection, RadarRenderer };
use crate::state::RadarState;

/// Main radar application
pub struct RadarApp {
    /// Shared radar state
    state: Arc<RadarState>,

    /// Current configuration
    config: RadarConfig,

    /// Coordinate projection
    projection: Projection,

    /// Radar renderer
    renderer: RadarRenderer,

    /// Tokio runtime for async operations
    _runtime: Arc<Runtime>,

    /// UI state
    ui_state: UiState,

    /// Start time for animations
    start_time: Instant,
}

#[derive(Default)]
struct UiState {
    /// Whether sidebar is visible
    show_sidebar: bool,

    /// Search filter for aircraft list
    search_filter: String,

    /// Show settings panel
    show_settings: bool,

    /// Last mouse position for panning
    last_mouse_pos: Option<egui::Pos2>,

    /// Whether we're currently panning
    is_panning: bool,
}

impl RadarApp {
    pub fn new(
        _cc: &eframe::CreationContext,
        state: Arc<RadarState>,
        config: RadarConfig,
        runtime: Arc<Runtime>
    ) -> Self {
        Self {
            state,
            config: config.clone(),
            projection: Projection::new(1920.0, 1080.0),
            renderer: RadarRenderer::new(),
            _runtime: runtime,
            ui_state: UiState {
                show_sidebar: true,
                ..Default::default()
            },
            start_time: Instant::now(),
        }
    }
}

impl eframe::App for RadarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update config from state (hot-reload)
        let new_config = self.state.get_config();

        // Check if fonts changed (requires restart to take effect)
        let fonts_changed =
            self.config.fonts.radar_font != new_config.fonts.radar_font ||
            self.config.fonts.ui_font != new_config.fonts.ui_font;

        self.config = new_config;

        // Note: Font file changes require restart, but font sizes update immediately
        if fonts_changed {
            tracing::warn!("Font configuration changed - restart application to apply new fonts");
        }

        // Request repaint for animations
        ctx.request_repaint();

        // Top panel - status bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.render_top_panel(ui);
        });

        // Side panel - aircraft list
        if self.ui_state.show_sidebar {
            egui::SidePanel
                ::left("sidebar")
                .default_width(300.0)
                .show(ctx, |ui| {
                    self.render_sidebar(ui);
                });
        }

        // Settings panel
        if self.ui_state.show_settings {
            egui::Window
                ::new("Settings")
                .default_width(400.0)
                .show(ctx, |ui| {
                    self.render_settings(ui);
                });
        }

        // Central panel - radar display
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_radar(ui);
        });
    }
}

impl RadarApp {
    /// Render top status bar
    fn render_top_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("ATC 24 Radar - TopSky Interface");

            ui.separator();

            let status = self.state.get_connection_status();

            // Connection indicator
            let (status_text, status_color) = if status.websocket_connected {
                ("● CONNECTED", egui::Color32::GREEN)
            } else {
                ("● DISCONNECTED", egui::Color32::RED)
            };

            ui.colored_label(status_color, status_text);

            ui.separator();

            ui.label(format!("Aircraft: {}", status.aircraft_count));

            ui.separator();

            ui.label(format!("Zoom: {:.0} studs/px", self.projection.studs_per_pixel));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙ Settings").clicked() {
                    self.ui_state.show_settings = !self.ui_state.show_settings;
                }

                if ui.button(if self.ui_state.show_sidebar { "◄" } else { "►" }).clicked() {
                    self.ui_state.show_sidebar = !self.ui_state.show_sidebar;
                }
            });
        });
    }

    /// Render aircraft list sidebar
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Aircraft");

        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.text_edit_singleline(&mut self.ui_state.search_filter);
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let aircraft = self.state.get_aircraft();
            let filter = self.ui_state.search_filter.to_lowercase();

            let mut sorted: Vec<_> = aircraft.values().collect();
            sorted.sort_by(|a, b| a.callsign.cmp(&b.callsign));

            for tracked in sorted {
                if !filter.is_empty() && !tracked.callsign.to_lowercase().contains(&filter) {
                    continue;
                }

                let is_selected =
                    self.renderer.selected_aircraft.as_ref() == Some(&tracked.callsign);

                let response = ui.selectable_label(is_selected, &tracked.callsign);

                if response.clicked() {
                    if is_selected {
                        self.renderer.selected_aircraft = None;
                    } else {
                        self.renderer.selected_aircraft = Some(tracked.callsign.clone());

                        // Center view on selected aircraft
                        self.projection.center = (tracked.info.position.x, tracked.info.position.y);
                    }
                }

                // Show details
                ui.indent(tracked.callsign.clone(), |ui| {
                    ui.small(format!("Type: {}", tracked.info.aircraft_type));
                    ui.small(format!("Alt: {:.0} ft", tracked.info.altitude));
                    ui.small(format!("GS: {:.0} kt", tracked.info.ground_speed));
                    ui.small(format!("Hdg: {:.0}°", tracked.info.heading));

                    if let Some(fp) = &tracked.flight_plan {
                        ui.small(format!("{} → {}", fp.departing, fp.arriving));
                    }

                    if tracked.info.is_emergency_occuring {
                        ui.colored_label(egui::Color32::RED, "⚠ EMERGENCY");
                    }
                });

                ui.separator();
            }
        });

        // ATIS section
        ui.separator();
        ui.heading("ATIS");

        egui::ScrollArea::vertical().show(ui, |ui| {
            let atis_map = self.state.get_all_atis();

            for (airport, atis) in atis_map {
                ui.collapsing(format!("{} - {}", airport, atis.letter), |ui| {
                    for line in &atis.lines {
                        ui.small(line);
                    }
                });
            }
        });
    }

    /// Render settings window
    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Configuration is managed via config.toml");
        ui.label("Edit the file to customize the radar display.");
        ui.separator();

        ui.label(format!("Config file: {}", crate::config::config_path().display()));

        if ui.button("Open Config Folder").clicked() {
            #[cfg(target_os = "windows")]
            std::process::Command::new("explorer").arg(".").spawn().ok();

            #[cfg(target_os = "macos")]
            std::process::Command::new("open").arg(".").spawn().ok();

            #[cfg(target_os = "linux")]
            std::process::Command::new("xdg-open").arg(".").spawn().ok();
        }
    }

    /// Render main radar display
    fn render_radar(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();

        // Update projection screen size
        self.projection.update_screen_size(rect.width(), rect.height());

        // Draw background
        let bg_color = parse_color(&self.config.colors.background);
        ui.painter().rect_filled(rect, 0.0, bg_color);

        // Handle input
        self.handle_radar_input(ui, rect);

        // Get current aircraft
        let aircraft = self.state.get_aircraft();

        // Get current time for animations
        let time_millis = self.start_time.elapsed().as_millis() as i64;

        // Render radar
        self.renderer.render(
            ui.painter(),
            &self.projection,
            &aircraft,
            &self.config.display,
            &self.config.colors,
            &self.config.data_tags,
            time_millis
        );

        // Draw center crosshair
        self.draw_center_crosshair(ui, rect);
    }

    /// Handle mouse/keyboard input for radar
    fn handle_radar_input(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        // Panning with middle mouse button or drag
        if
            response.dragged_by(egui::PointerButton::Middle) ||
            (response.dragged() && ui.input(|i| i.modifiers.shift))
        {
            let delta = response.drag_delta();
            self.projection.pan(delta);
        }

        // Zoom with scroll wheel
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let mouse_pos = response.hover_pos();
            self.projection.zoom(scroll, mouse_pos);
        }

        // Click to select aircraft
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.select_aircraft_at_position(pos);
            }
        }
    }

    /// Select aircraft at screen position
    fn select_aircraft_at_position(&mut self, screen_pos: egui::Pos2) {
        let aircraft = self.state.get_aircraft();
        let threshold = 15.0; // Click radius in pixels

        let mut closest: Option<(&String, f32)> = None;

        for (callsign, tracked) in &aircraft {
            let aircraft_pos = self.projection.studs_to_screen(
                tracked.info.position.x,
                tracked.info.position.y
            );

            let distance = screen_pos.distance(aircraft_pos);

            if distance < threshold {
                if let Some((_, min_dist)) = closest {
                    if distance < min_dist {
                        closest = Some((callsign, distance));
                    }
                } else {
                    closest = Some((callsign, distance));
                }
            }
        }

        if let Some((callsign, _)) = closest {
            self.renderer.selected_aircraft = Some(callsign.clone());
        } else {
            self.renderer.selected_aircraft = None;
        }
    }

    /// Draw center crosshair
    fn draw_center_crosshair(&self, ui: &egui::Ui, rect: egui::Rect) {
        let center = rect.center();
        let size = 10.0;
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100);

        ui.painter().line_segment(
            [
                egui::Pos2::new(center.x - size, center.y),
                egui::Pos2::new(center.x + size, center.y),
            ],
            egui::Stroke::new(1.0, color)
        );

        ui.painter().line_segment(
            [
                egui::Pos2::new(center.x, center.y - size),
                egui::Pos2::new(center.x, center.y + size),
            ],
            egui::Stroke::new(1.0, color)
        );
    }
}
