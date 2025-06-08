use crate::{create_registered_manager, AsyncRegisteredManager};
use anyhow::Result;
use presage::manager::Registered;
use presage::model::groups::Group;
use presage::store::ContentsStore;
use presage::{libsignal_service::zkgroup::GroupMasterKeyBytes, Manager};
use presage_store_sled::{SledStore, SledStoreError};
// use std::sync::Arc;
// use tokio::sync::Mutex;

async fn list_groups(
    manager: &Manager<SledStore, Registered>,
) -> Result<Vec<Result<(GroupMasterKeyBytes, Group), SledStoreError>>> {
    Ok(manager.store().groups().await?.collect())
}

/// Returns iterator over stored contacts, for use in CLI
pub async fn list_groups_cli() -> Result<Vec<Result<(GroupMasterKeyBytes, Group), SledStoreError>>>
{
    let manager = create_registered_manager().await?;
    list_groups(&manager).await
}

/// Returns iterator over stored contacts, for use in TUI
pub async fn list_groups_tui(
    manager_mutex: AsyncRegisteredManager,
) -> Result<Vec<Result<(GroupMasterKeyBytes, Group), SledStoreError>>> {
    let manager = manager_mutex.read().await;
    list_groups(&manager).await
}
