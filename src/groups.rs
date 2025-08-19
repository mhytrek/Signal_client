use anyhow::Result;
use presage::libsignal_service::zkgroup::GroupMasterKeyBytes;
use presage::model::groups::Group;
use presage::store::ContentsStore;
use presage::{Manager, manager::Registered};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};

use crate::{AsyncRegisteredManager, create_registered_manager};

async fn list_groups(
    manager: &Manager<SqliteStore, Registered>,
) -> Result<Vec<Result<(GroupMasterKeyBytes, Group), SqliteStoreError>>> {
    Ok(manager.store().groups().await?.collect())
}

pub async fn list_groups_cli() -> Result<Vec<Result<(GroupMasterKeyBytes, Group), SqliteStoreError>>>
{
    let manager = create_registered_manager().await?;
    list_groups(&manager).await
}

pub async fn list_groups_tui(
    manager_mutex: AsyncRegisteredManager,
) -> Result<Vec<Result<(GroupMasterKeyBytes, Group), SqliteStoreError>>> {
    let manager = manager_mutex.read().await;
    list_groups(&manager).await
}
