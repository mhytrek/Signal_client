use anyhow::{Error, Result};
use presage::{
    manager::{Manager, Registered},
    model::identity::OnNewIdentity,
};
use presage_store_sled::{MigrationConflictStrategy, SledStore};

pub mod app;
pub mod args;
pub mod cli;
pub mod contacts;
pub mod devices;
pub mod paths;
pub mod sending_text;
pub mod tui;
pub mod ui;

pub mod sending {}

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
