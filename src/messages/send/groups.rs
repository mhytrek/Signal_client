use std::time::{SystemTime, UNIX_EPOCH};

use presage::{libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered, model::groups::Group, proto::DataMessage, Manager};
use presage_store_sled::{SledStore, SledStoreError};
use presage::store::ContentsStore;
use anyhow::Result;
use crate::{create_registered_manager, messages::{receive::receiving_loop, send::create_data_message}};

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

async fn send(
    manager: &mut Manager<SledStore, Registered>,
    destination_group: GroupMasterKeyBytes,
    data_message: DataMessage,
    timestamp: u64,
) -> Result<()> {
    manager
        .send_message_to_group(&destination_group, data_message, timestamp)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
    Ok(())
}

async fn send_message(
    manager: &mut Manager<SledStore, Registered>,
    recipient: String,
    text_message: String,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let destination_group = find_master_key(recipient, manager).await?;
    let data_message = create_data_message(text_message, timestamp)?;

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, None).await?;

    send(manager, destination_group, data_message, timestamp).await?;

    Ok(())
}
