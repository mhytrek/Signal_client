use directories::UserDirs;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::env::SIGNAL_CONFIG_DIR;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub color_mode: bool,  // true for color, false for black-and-white
    pub show_images: bool, // true to show images, false to hide them
    pub attachment_save_dir: PathBuf,
    pub current_account: Option<String>,
    pub notifications_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        let download_dir = match UserDirs::new() {
            Some(user_dir) => user_dir
                .download_dir()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("./attachments")),
            None => PathBuf::from("./attachments"),
        };
        Config {
            color_mode: true,
            show_images: true,
            current_account: None,
            notifications_enabled: true,
            attachment_save_dir: download_dir,
        }
    }
}

impl Config {
    fn get_config_path() -> PathBuf {
        if let Ok(config_dir) = std::env::var(SIGNAL_CONFIG_DIR) {
            PathBuf::from(config_dir).join("config.json")
        } else if cfg!(debug_assertions) {
            PathBuf::from("./signal_client/config.json")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("signal_client")
                .join("config.json")
        }
    }

    pub fn set_current_account(&mut self, account_name: String) {
        self.current_account = Some(account_name);
    }

    pub fn get_current_account(&self) -> Option<&String> {
        self.current_account.as_ref()
    }

    pub fn clear_current_account(&mut self) {
        self.current_account = None;
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

    /// Enbale/Disable notifications for new messages
    pub fn toggle_notifications(&mut self) {
        self.notifications_enabled = !self.notifications_enabled;
    }
}
