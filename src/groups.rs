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

pub async fn find_master_key(
    group_name: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<Option<GroupMasterKeyBytes>> {
    // WARN: Right now it assumes that all groups have unique names this is. This has to be handled
    // correctly in future.
    let group = manager
        .store()
        .groups()
        .await?
        .filter_map(|g| g.ok())
        .find(|(_, group)| group.title == group_name);

    let key = group.map(|g| g.0);
    Ok(key)
}
