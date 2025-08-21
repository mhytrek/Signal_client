use anyhow::{Error, Result};
use presage::{
    libsignal_service::{prelude::Uuid, zkgroup::GroupMasterKeyBytes},
    manager::{Manager, Registered},
    model::{contacts::Contact, groups::Group, identity::OnNewIdentity},
};
use presage_store_sqlite::{SqliteConnectOptions, SqliteStore, SqliteStoreError};
use std::sync::Arc;
use std::{collections::HashMap, str::FromStr};
use tokio::sync::Mutex;

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
pub mod paths;
pub mod profile;
pub mod tui;
pub mod ui;

pub mod sending {}

pub type AsyncContactsMap = Arc<Mutex<HashMap<Uuid, Contact>>>;

pub async fn open_store(path: &str) -> Result<SqliteStore, SqliteStoreError> {
    let options = SqliteConnectOptions::from_str(path)?.create_if_missing(true);
    SqliteStore::open_with_options(options, OnNewIdentity::Trust).await
}

/// Creates new manager in registered state
pub async fn create_registered_manager() -> Result<Manager<SqliteStore, Registered>> {
    let store = open_store(paths::STORE).await?;

    match Manager::load_registered(store).await {
        Ok(manager) => Ok(manager),
        Err(err) => Err(Error::new(err)),
    }
}
