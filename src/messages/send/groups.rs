use presage::{libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered, model::groups::Group, Manager};
use presage_store_sled::{SledStore, SledStoreError};
use presage::store::ContentsStore;
use anyhow::Result;

async fn find_master_key(
    recipient: String,
    manager: &mut Manager<SledStore, Registered>,
) -> Result<GroupMasterKeyBytes> {
    let groups: Vec<Result<(GroupMasterKeyBytes, Group), SledStoreError>> =
        manager.store().groups().await?.collect();

    let master_key = groups.into_iter()
        .filter_map(|g| g.ok())
        .find(|(_key, group)| {
            group.title == recipient
        })
        .map(|k| k.0);

    master_key.ok_or_else(|| anyhow::anyhow!("Group '{}' not found", recipient))
}
