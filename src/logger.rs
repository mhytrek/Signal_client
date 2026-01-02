use anyhow::{Result, bail};
use std::path::PathBuf;
use std::{
    env::{self},
    fs,
    sync::OnceLock,
};

use tracing::level_filters::LevelFilter;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt::writer::BoxMakeWriter};

use crate::env::{SIGNAL_ENABLE_LOGGER, SIGNAL_LOGGER_LEVEL, SIGNAL_ONSCREEN_LOGGER};

#[cfg(target_os = "macos")]
fn ensure_log_dir() -> Result<PathBuf> {
    let home_dir = match dirs::home_dir() {
        Some(home_dir) => home_dir,
        None => bail!("Unable to resolve home directory."),
    };
    let state_dir = home_dir.join(".local/state");
    if !fs::exists(&state_dir)? {
        fs::create_dir_all(&state_dir)?;
    }

    Ok(state_dir.join("signal_client/logs"))
}

#[cfg(target_os = "linux")]
fn ensure_log_dir() -> Result<PathBuf> {
    match dirs::state_dir() {
        Some(state_dir) => {
            if !fs::exists(state_dir.join("signal_client/logs"))? {
                fs::create_dir_all(&state_dir)?;
            }
            Ok(state_dir.join("signal_client/logs"))
        }
        None => bail!("Unable to resolve logging directory."),
    }
}

fn logs_directory() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            PathBuf::from("./signal_client/logs")
        } else {
            match ensure_log_dir() {
                Ok(log_dir) => log_dir,
                Err(_) => PathBuf::from("./signal_client/logs"),
            }
        }
    })
    .into()
}

pub fn init_logger() {
    if cfg!(debug_assertions) || env::var(SIGNAL_ENABLE_LOGGER).is_ok() {
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .with_env_var(SIGNAL_LOGGER_LEVEL)
            .from_env()
            .unwrap_or_default();
        let onscreen_logs = env::var(SIGNAL_ONSCREEN_LOGGER).is_ok();

        if onscreen_logs {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(true)
                .init();
        } else {
            let file_writer = rolling::hourly(logs_directory(), "signal_client.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_writer);

            tracing_subscriber::fmt()
                .with_writer(BoxMakeWriter::new(non_blocking))
                .with_env_filter(filter)
                .with_ansi(false)
                .init();

            // Drops ownership of the value, but keeps it alive
            std::mem::forget(_guard);
        }
    }
}
