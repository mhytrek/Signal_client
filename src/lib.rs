use anyhow::{Error, Result};
use presage::{
    libsignal_service::prelude::Uuid,
    manager::{Manager, Registered},
    model::{contacts::Contact, identity::OnNewIdentity},
};
use presage_store_sled::{MigrationConflictStrategy, SledStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub mod app;
pub mod args;
pub mod cli;
pub mod contacts;
pub mod devices;
pub mod messages;
pub mod paths;
pub mod tui;
pub mod ui;

pub mod profile;

pub mod sending {}

pub type AsyncRegisteredManager = Arc<RwLock<Manager<SledStore, Registered>>>;

pub type AsyncContactsMap = Arc<Mutex<HashMap<Uuid, Contact>>>;

/// Creates new manager in registered state
pub async fn create_registered_manager() -> Result<Manager<SledStore, Registered>> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    )
    .await?;

    // Sadly it has to be done this way, because anyhow::Error doesn't cover errors
    // from presage
    match Manager::load_registered(store).await {
        Ok(manager) => Ok(manager),
        Err(err) => Err(Error::new(err)),
    }
}
