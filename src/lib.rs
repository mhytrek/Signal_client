use crate::config::Config;
use anyhow::Result;
use presage::model::identity::OnNewIdentity;
use presage_store_sqlite::{SqliteConnectOptions, SqliteStore, SqliteStoreError};
use std::{path::Path, str::FromStr};

pub mod account_management;
pub mod app;
pub mod args;
pub mod cli;
pub mod config;
pub mod contacts;
pub mod devices;
pub mod env;
pub mod groups;
pub mod logger;
pub mod messages;
mod notifications;
pub mod paths;
pub mod profile;
mod retry_manager;
pub mod tui;
pub mod ui;

pub mod sending {}

pub async fn open_store(path: &Path) -> Result<SqliteStore, SqliteStoreError> {
    let options = SqliteConnectOptions::from_str(
        path.to_str()
            .expect("Failed to resolve database path from `PathBuf`"),
    )?
    .create_if_missing(true);
    SqliteStore::open_with_options(options, OnNewIdentity::Trust).await
}
