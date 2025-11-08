use std::env::home_dir;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;
use tracing::error;

pub const ACCOUNTS_DIR: &str = "accounts";

fn ensure_local_share_dir(home_dir: &Path) -> Result<()> {
    if !fs::exists(home_dir.join(".local/share"))? {
        fs::create_dir_all(home_dir.join(".local/share"))?;
    }
    Ok(())
}

pub fn store() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            "sqlite://signal_store.db".to_string()
        } else {
            match home_dir() {
                Some(home_dir) => match ensure_local_share_dir(&home_dir) {
                    Ok(_) => home_dir
                        .join(".local/share/signal_client/store.db")
                        .to_str()
                        .unwrap_or("sqlite://signal_store.db")
                        .to_string(),
                    Err(error) => {
                        error!(%error, "Unable to ensure if ~/.local/share directory exists.");
                        "sqlite://signal_store.db".to_string()
                    }
                },
                None => "sqlite://signal_store.db".to_string(),
            }
        }
    })
    .into()
}

pub fn qrcode() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            "./assets/signal_client/qrcode".to_string()
        } else {
            match home_dir() {
                Some(home_dir) => match ensure_local_share_dir(&home_dir) {
                    Ok(_) => home_dir
                        .join(".local/share/signal_client/assets/qrcode")
                        .to_str()
                        .unwrap_or("./assets/signal_client/qrcode")
                        .to_string(),
                    Err(error) => {
                        error!(%error, "Unable to ensure if ~/.local/share directory exists.");
                        "./assets/signal_client/qrcode".to_string()
                    }
                },
                None => "./assets/signal_client/qrcode".to_string(),
            }
        }
    })
    .into()
}

pub fn assets() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            "./signal_client/assets".to_string()
        } else {
            match home_dir() {
                Some(home_dir) => match ensure_local_share_dir(&home_dir) {
                    Ok(_) => home_dir
                        .join(".local/share/signal_client/assets")
                        .to_str()
                        .unwrap_or("./signal_client/assets")
                        .to_string(),
                    Err(error) => {
                        error!(%error, "Unable to ensure if ~/.local/share directory exists.");
                        "./signal_client/assets".to_string()
                    }
                },
                None => "./signal_client/assets".to_string(),
            }
        }
    })
    .into()
}

pub fn accounts_dir() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            "./signal_client/accounts".to_string()
        } else {
            match home_dir() {
                Some(home_dir) => match ensure_local_share_dir(&home_dir) {
                    Ok(_) => home_dir
                        .join(".local/share/signal_client/accounts")
                        .to_str()
                        .unwrap_or("./signal_client/accounts")
                        .to_string(),
                    Err(error) => {
                        error!(%error, "Unable to ensure if ~/.local/share directory exists.");
                        "./signal_client/accounts".to_string()
                    }
                },
                None => "./signal_client/accounts".to_string(),
            }
        }
    })
    .into()
}
