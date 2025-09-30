use crate::config::Config;
use anyhow::Result;
use presage::{
    libsignal_service::prelude::Uuid,
    model::{contacts::Contact, identity::OnNewIdentity},
};
use presage_store_sqlite::{SqliteConnectOptions, SqliteStore, SqliteStoreError};
use std::sync::Arc;
use std::{collections::HashMap, str::FromStr};
use tokio::sync::Mutex;

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
pub mod paths;
pub mod profile;
mod retry_manager;
pub mod tui;
pub mod ui;
mod notifications;

pub mod sending {}

pub type AsyncContactsMap = Arc<Mutex<HashMap<Uuid, Contact>>>;

pub async fn open_store(path: &str) -> Result<SqliteStore, SqliteStoreError> {
    let options = SqliteConnectOptions::from_str(path)?.create_if_missing(true);
    SqliteStore::open_with_options(options, OnNewIdentity::Trust).await
}
