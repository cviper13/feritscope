use egui::{Color32, Pos2, Stroke, Vec2};
use std::collections::HashMap;

use crate::config::{ColorConfig, DisplayConfig};
use crate::types::TrackedAircraft;

/// Coordinate projection system for converting PTFS studs to screen pixels
#[derive(Debug, Clone)]
pub struct Projection {
    /// Center point in studs (x, y)
    pub center: (f64, f64),
    
    /// Zoom level (studs per pixel)
    pub studs_per_pixel: f64,
    
    /// Screen dimensions
    pub screen_width: f32,
    pub screen_height: f32,
}

impl Projection {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            center: (0.0, 0.0),
            studs_per_pixel: 100.0, // Default zoom
            screen_width,
            screen_height,
        }
    }
    
    /// Convert PTFS studs coordinates to screen pixels
    /// Note: In PTFS, -y is North, -x is West
    pub fn studs_to_screen(&self, studs_x: f64, studs_y: f64) -> Pos2 {
        // Calculate offset from center
        let dx = studs_x - self.center.0;
        let dy = studs_y - self.center.1;
        
        // Convert to screen space
        // In screen space: +x is right, +y is down
        // In PTFS: -x is West (left), -y is North (up)
        let screen_x = self.screen_width / 2.0 + (dx / self.studs_per_pixel) as f32;
        let screen_y = self.screen_height / 2.0 + (dy / self.studs_per_pixel) as f32;
        
        Pos2::new(screen_x, screen_y)
    }
    
    /// Convert screen pixels to PTFS studs coordinates
    pub fn screen_to_studs(&self, screen_pos: Pos2) -> (f64, f64) {
        let dx = (screen_pos.x - self.screen_width / 2.0) as f64 * self.studs_per_pixel;
        let dy = (screen_pos.y - self.screen_height / 2.0) as f64 * self.studs_per_pixel;
        
        (self.center.0 + dx, self.center.1 + dy)
    }
    
    /// Pan the view by screen pixels
    pub fn pan(&mut self, delta_screen: Vec2) {
        let delta_studs_x = delta_screen.x as f64 * self.studs_per_pixel;
        let delta_studs_y = delta_screen.y as f64 * self.studs_per_pixel;
        
        self.center.0 -= delta_studs_x;
        self.center.1 -= delta_studs_y;
    }
    
    /// Zoom in/out (positive = zoom in, negative = zoom out)
    pub fn zoom(&mut self, delta: f32, mouse_pos: Option<Pos2>) {
        let zoom_factor = if delta > 0.0 { 0.9 } else { 1.1 };
        
        // If mouse position provided, zoom towards it
        if let Some(mouse) = mouse_pos {
            let studs_before = self.screen_to_studs(mouse);
            self.studs_per_pixel *= zoom_factor;
            let studs_after = self.screen_to_studs(mouse);
            
            // Adjust center to keep mouse position stable
            self.center.0 += studs_before.0 - studs_after.0;
            self.center.1 += studs_before.1 - studs_after.1;
        } else {
            self.studs_per_pixel *= zoom_factor;
        }
        
        // Clamp zoom
        self.studs_per_pixel = self.studs_per_pixel.clamp(1.0, 1000.0);
    }
    
    /// Update screen dimensions
    pub fn update_screen_size(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }
}

/// Radar rendering engine
pub struct RadarRenderer {
    /// Selected aircraft callsign
    pub selected_aircraft: Option<String>,
}

impl RadarRenderer {
    pub fn new() -> Self {
        Self {
            selected_aircraft: None,
        }
    }
    
    /// Render all aircraft on the radar
    pub fn render(
        &self,
        painter: &egui::Painter,
        projection: &Projection,
        aircraft: &HashMap<String, TrackedAircraft>,
        display_config: &DisplayConfig,
        color_config: &ColorConfig,
        tag_config: &crate::config::DataTagConfig,
        time_millis: i64,
    ) {
        // Render in layers for proper z-order
        
        // 1. History trails
        if display_config.show_history {
            for tracked in aircraft.values() {
                self.render_history(painter, projection, tracked, display_config, color_config);
            }
        }
        
        // 2. Predictive vectors
        if display_config.show_vectors {
            for tracked in aircraft.values() {
                self.render_vector(painter, projection, tracked, display_config, color_config);
            }
        }
        
        // 3. Aircraft targets
        for tracked in aircraft.values() {
            self.render_target(painter, projection, tracked, display_config, color_config, time_millis);
        }
        
        // 4. Data tags
        if display_config.show_tags {
            for tracked in aircraft.values() {
                self.render_data_tag(painter, projection, tracked, display_config, color_config, tag_config);
            }
        }
    }
    
    /// Render aircraft target symbol (diamond/square)
    fn render_target(
        &self,
        painter: &egui::Painter,
        projection: &Projection,
        tracked: &TrackedAircraft,
        display: &DisplayConfig,
        colors: &ColorConfig,
        time_millis: i64,
    ) {
        let pos = projection.studs_to_screen(
            tracked.info.position.x,
            tracked.info.position.y,
        );
        
        // Determine color
        let color = if tracked.info.is_emergency_occuring {
            // Flash emergency aircraft
            let flash = (time_millis / 500) % 2 == 0;
            if flash {
                parse_color(&colors.target_emergency)
            } else {
                Color32::TRANSPARENT
            }
        } else if Some(&tracked.callsign) == self.selected_aircraft.as_ref() {
            parse_color(&colors.target_selected)
        } else if tracked.info.is_on_ground.unwrap_or(false) {
            parse_color(&colors.ground)
        } else {
            parse_color(&colors.target)
        };
        
        // Draw target symbol (diamond shape)
        let size = 6.0 * display.target_scale;
        let points = vec![
            pos + Vec2::new(0.0, -size),      // Top
            pos + Vec2::new(size, 0.0),       // Right
            pos + Vec2::new(0.0, size),       // Bottom
            pos + Vec2::new(-size, 0.0),      // Left
        ];
        
        painter.add(egui::Shape::closed_line(
            points,
            Stroke::new(display.target_stroke, color),
        ));
        
        // Draw heading indicator
        let heading_rad = (tracked.info.heading - 90.0).to_radians(); // -90 to align with North
        let heading_len = size * 2.0;
        let heading_end = pos + Vec2::new(
            (heading_rad.cos() * heading_len as f64) as f32,
            (heading_rad.sin() * heading_len as f64) as f32,
        );
        
        painter.line_segment(
            [pos, heading_end],
            Stroke::new(display.target_stroke * 0.7, color),
        );
    }
    
    /// Render history trail dots
    fn render_history(
        &self,
        painter: &egui::Painter,
        projection: &Projection,
        tracked: &TrackedAircraft,
        display: &DisplayConfig,
        colors: &ColorConfig,
    ) {
        let color = parse_color(&colors.history);
        let dot_size = display.history_dot_size;
        
        for (x, y, _timestamp) in &tracked.history {
            let pos = projection.studs_to_screen(*x, *y);
            painter.circle_filled(pos, dot_size, color);
        }
    }
    
    /// Render predictive vector
    fn render_vector(
        &self,
        painter: &egui::Painter,
        projection: &Projection,
        tracked: &TrackedAircraft,
        display: &DisplayConfig,
        colors: &ColorConfig,
    ) {
        let current_pos = projection.studs_to_screen(
            tracked.info.position.x,
            tracked.info.position.y,
        );
        
        // Calculate predicted position based on ground speed and heading
        // PTFS uses 1 knot = 0.5442765 studs/sec
        let studs_per_knot_per_sec = 0.5442765;
        let seconds_ahead = display.vector_minutes * 60.0;
        
        let gs_knots = tracked.info.ground_speed;
        let heading_rad = (tracked.info.heading - 90.0).to_radians();
        
        let distance_studs = (gs_knots * studs_per_knot_per_sec as f64 * seconds_ahead as f64) as f64;
        
        let predicted_x = tracked.info.position.x + distance_studs * heading_rad.cos();
        let predicted_y = tracked.info.position.y + distance_studs * heading_rad.sin();
        
        let predicted_pos = projection.studs_to_screen(predicted_x, predicted_y);
        
        painter.line_segment(
            [current_pos, predicted_pos],
            Stroke::new(1.5, parse_color(&colors.vector)),
        );
    }
    
    /// Render aircraft data tag
    fn render_data_tag(
        &self,
        painter: &egui::Painter,
        projection: &Projection,
        tracked: &TrackedAircraft,
        display: &DisplayConfig,
        colors: &ColorConfig,
        tag_config: &crate::config::DataTagConfig,
    ) {
        let pos = projection.studs_to_screen(
            tracked.info.position.x,
            tracked.info.position.y,
        );
        
        // Apply offset from config
        let tag_pos = pos + Vec2::new(tag_config.offset.0, tag_config.offset.1);
        
        let text_color = parse_color(&colors.tag_text);
        
        // Build lines from template config
        let mut lines = Vec::new();
        lines.push(Self::format_tag_line(&tag_config.line1, tracked));
        lines.push(Self::format_tag_line(&tag_config.line2, tracked));
        
        if let Some(line3) = &tag_config.line3 {
            lines.push(Self::format_tag_line(line3, tracked));
        }
        
        if let Some(line4) = &tag_config.line4 {
            lines.push(Self::format_tag_line(line4, tracked));
        }
        
        // Render lines
        for (i, line) in lines.iter().enumerate() {
            let line_pos = tag_pos + Vec2::new(0.0, i as f32 * tag_config.line_spacing);
            
            painter.text(
                line_pos,
                egui::Align2::LEFT_TOP,
                line,
                egui::FontId::monospace(display.font_size),
                text_color,
            );
        }
    }
    
    /// Format data tag using template system
    fn format_data_tag(&self, tracked: &TrackedAircraft, _display: &DisplayConfig) -> Vec<String> {
        // We'll need to add this to RadarRenderer to get the full config
        // For now, use default formatting
        let altitude = (tracked.info.altitude / 100.0) as i32;
        let gs = tracked.info.ground_speed as i32;
        
        vec![
            tracked.callsign.clone(),
            format!("F{:03} {:03}KT", altitude, gs),
        ]
    }
    
    /// Format data tag line using template string
    /// Supports variables: {callsign}, {altitude}, {speed}, {gs}, {heading}, {type}
    pub fn format_tag_line(template: &str, tracked: &TrackedAircraft) -> String {
        let altitude = (tracked.info.altitude / 100.0) as i32;
        let speed = tracked.info.speed as i32;
        let gs = tracked.info.ground_speed as i32;
        let heading = tracked.info.heading as i32;
        
        template
            .replace("{callsign}", &tracked.callsign)
            .replace("{altitude:03}", &format!("{:03}", altitude))
            .replace("{altitude}", &altitude.to_string())
            .replace("{speed:03}", &format!("{:03}", speed))
            .replace("{speed}", &speed.to_string())
            .replace("{gs:03}", &format!("{:03}", gs))
            .replace("{gs}", &gs.to_string())
            .replace("{heading:03}", &format!("{:03}", heading))
            .replace("{heading}", &heading.to_string())
            .replace("{type}", &tracked.info.aircraft_type)
    }
}

impl Default for RadarRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse hex color string to Color32
pub fn parse_color(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        Color32::from_rgb(r, g, b)
    } else {
        Color32::WHITE
    }
}