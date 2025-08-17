use std::env;

use tracing::level_filters::LevelFilter;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt::writer::BoxMakeWriter};

use crate::env::{SIGNAL_LOGGER_LEVEL, SIGNAL_ONSCREEN_LOGGER};

pub fn init_logger() {
    // let logger_level = env::var(SIGNAL_LOGGER_LEVEL).unwrap_or(String::new());
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
        let file_writer = rolling::hourly(".", "signal_client.log");
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
