use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub color_mode: bool,       // true for color, false for black-and-white
    pub show_images: bool,      // true to show images, false to hide them
    pub compact_messages: bool, // true to have compact messages display
}

impl Default for Config {
    fn default() -> Self {
        Config {
            color_mode: true,
            show_images: true,
            compact_messages: false,
        }
    }
}

impl Config {
    fn get_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("signal-tui")
            .join("config.json")
    }

    pub fn load() -> Self {
        let config_path = Self::get_config_path();

        if let Ok(contents) = fs::read_to_string(&config_path)
            && let Ok(config) = serde_json::from_str::<Config>(&contents)
        {
            return config;
        }

        let default_config = Config::default();
        let _ = default_config.save();
        default_config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)?;
        fs::write(config_path, contents)?;
        Ok(())
    }

    /// Toggle color mode
    pub fn toggle_color_mode(&mut self) {
        self.color_mode = !self.color_mode;
    }

    /// Toggle image display
    pub fn toggle_show_images(&mut self) {
        self.show_images = !self.show_images;
    }

    pub fn toggle_compact_messages(&mut self) {
        self.compact_messages = !self.compact_messages
    }

    /// Get primary color based on color mode
    pub fn get_primary_color(&self) -> Color {
        Color::default()
    }

    /// Get secondary color based on color mode
    pub fn get_secondary_color(&self) -> Color {
        if self.color_mode {
            Color::Green
        } else {
            Color::Gray
        }
    }

    /// Get accent color based on color mode
    pub fn get_accent_color(&self) -> Color {
        if self.color_mode {
            Color::Yellow
        } else {
            Color::default()
        }
    }

    /// Get error color based on color mode
    pub fn get_error_color(&self) -> Color {
        if self.color_mode {
            Color::Red
        } else {
            Color::default()
        }
    }

    /// Get success color based on color mode
    pub fn get_success_color(&self) -> Color {
        if self.color_mode {
            Color::Green
        } else {
            Color::default()
        }
    }
}
