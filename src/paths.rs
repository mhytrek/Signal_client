use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Result, bail};
use tracing::error;

pub const STORE: &str = "sqlite://store.db";

fn ensure_data_dir() -> Result<PathBuf> {
    match dirs::data_dir() {
        Some(data_dir) => {
            if !fs::exists(&data_dir)? {
                fs::create_dir(&data_dir)?;
            }
            Ok(data_dir)
        }
        None => bail!("Unable to resolve directory to hold client data."),
    }
}

pub fn store() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            PathBuf::from("sqlite://signal_store.db")
        } else {
            match ensure_data_dir() {
                Ok(data_dir) => data_dir.join("signal_client/store.db"),
                Err(error) => {
                    error!(?error);
                    PathBuf::from("sqlite://signal_store.db")
                }
            }
        }
    })
    .into()
}

pub fn qrcode() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            PathBuf::from("./signal_client/assets/qrcode")
        } else {
            match ensure_data_dir() {
                Ok(data_dir) => data_dir.join("signal_client/assets/qrcode"),
                Err(error) => {
                    error!(?error);
                    PathBuf::from("./signal_client/assets/qrcode")
                }
            }
        }
    })
    .into()
}

pub fn assets() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            PathBuf::from("./signal_client/assets")
        } else {
            match ensure_data_dir() {
                Ok(data_dir) => data_dir.join("signal_client/assets"),
                Err(error) => {
                    error!(?error);
                    PathBuf::from("./signal_client/assets")
                }
            }
        }
    })
    .into()
}

pub fn accounts_dir() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            PathBuf::from("./signal_client/accounts")
        } else {
            match ensure_data_dir() {
                Ok(data_dir) => data_dir.join("signal_client/accounts"),
                Err(error) => {
                    error!(?error);
                    PathBuf::from("./signal_client/accounts")
                }
            }
        }
    })
    .into()
}

pub fn account_store_path(account_name: &str) -> PathBuf {
    accounts_dir().join(account_name).join("store.db")
}
