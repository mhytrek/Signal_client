use anyhow::Result;
use std::{
    env::{self, home_dir},
    fs,
    path::Path,
    sync::OnceLock,
};

use tracing::level_filters::LevelFilter;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt::writer::BoxMakeWriter};

use crate::env::{SIGNAL_LOGGER_LEVEL, SIGNAL_ONSCREEN_LOGGER};

fn ensure_local_state(home_dir: &Path) -> Result<()> {
    if !fs::exists(home_dir.join(".local/state"))? {
        fs::create_dir_all(home_dir.join(".local/state"))?;
    }
    Ok(())
}

fn logs_directory() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(debug_assertions) {
            "./signal_client/logs".to_string()
        } else {
            match home_dir() {
                Some(home_dir) => match ensure_local_state(&home_dir) {
                    Ok(_) => home_dir
                        .join(".local/state/signal_client/logs")
                        .to_str()
                        .unwrap_or("./signal_client/logs")
                        .to_string(),
                    Err(_) => "./signal_client/logs".to_string(),
                },
                None => "./signal_client/logs".to_string(),
            }
        }
    })
    .into()
}

pub fn init_logger() {
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
