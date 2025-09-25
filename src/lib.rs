use crate::config::Config;
use anyhow::{Error, Result};
use presage::{
    libsignal_service::prelude::Uuid,
    manager::{Manager, Registered},
    model::{contacts::Contact, identity::OnNewIdentity},
};
use presage_store_sqlite::{SqliteConnectOptions, SqliteStore, SqliteStoreError};
use std::fs;
use std::path::Path;
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

pub mod account_management;
mod retry_manager;

pub mod sending {}

pub type AsyncContactsMap = Arc<Mutex<HashMap<Uuid, Contact>>>;
pub const ACCOUNTS_DIR: &str = "accounts";

pub async fn open_store(path: &str) -> Result<SqliteStore, SqliteStoreError> {
    let options = SqliteConnectOptions::from_str(path)?.create_if_missing(true);
    SqliteStore::open_with_options(options, OnNewIdentity::Trust).await
}

/// Creates new manager in registered state
pub async fn create_registered_manager() -> Result<Manager<SqliteStore, Registered>> {
    let config = Config::load();

    match config.get_current_account() {
        Some(account_name) => create_registered_manager_for_account(account_name).await,
        None => {
            let store = open_store(paths::STORE).await?;
            match Manager::load_registered(store).await {
                Ok(manager) => Ok(manager),
                Err(err) => Err(Error::new(err)),
            }
        }
    }
}

pub fn get_account_store_path(account_name: &str) -> String {
    format!("{}/{}/store.db", ACCOUNTS_DIR, account_name)
}

pub fn get_account_assets_path(account_name: &str) -> String {
    format!("{}/{}/assets", ACCOUNTS_DIR, account_name)
}

pub fn ensure_accounts_dir() -> Result<()> {
    if !Path::new(ACCOUNTS_DIR).exists() {
        fs::create_dir_all(ACCOUNTS_DIR)?;
    }
    Ok(())
}

pub fn list_accounts() -> Result<Vec<String>> {
    ensure_accounts_dir()?;
    let mut accounts = Vec::new();

    if let Ok(entries) = fs::read_dir(ACCOUNTS_DIR) {
        for entry in entries {
            if let Ok(entry) = entry
                && entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                let store_path = get_account_store_path(name);
                if Path::new(&store_path).exists() {
                    accounts.push(name.to_string());
                }
            }
        }
    }

    Ok(accounts)
}

/// Creates new manager for specific account
pub async fn create_registered_manager_for_account(
    account_name: &str,
) -> Result<Manager<SqliteStore, Registered>> {
    let store_path = get_account_store_path(account_name);
    let store = open_store(&store_path).await?;

    match Manager::load_registered(store).await {
        Ok(manager) => Ok(manager),
        Err(err) => Err(Error::new(err)),
    }
}
