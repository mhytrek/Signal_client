use log::debug;
use presage::proto::GroupContextV2;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{create_registered_manager, messages::receive::receiving_loop};
use anyhow::Result;
use presage::store::ContentsStore;
use presage::{
    libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered, model::groups::Group,
    proto::DataMessage, Manager,
};
use presage_store_sled::{SledStore, SledStoreError};

fn create_data_message(
    text_message: String,
    group_master_key: GroupMasterKeyBytes,
    timestamp: u64,
) -> Result<DataMessage> {
    let data_msg = DataMessage {
        body: Some(
            text_message
                .parse()
                .map_err(|_| anyhow::anyhow!("Failed to parse text message!"))?,
        ),
        group_v2: Some(GroupContextV2 {
            master_key: Some(group_master_key.to_vec()),
            revision: Some(0),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        ..Default::default()
    };
    Ok(data_msg)
}

async fn find_master_key(
    recipient: String,
    manager: &mut Manager<SledStore, Registered>,
) -> Result<GroupMasterKeyBytes> {
    let groups: Vec<Result<(GroupMasterKeyBytes, Group), SledStoreError>> =
        manager.store().groups().await?.collect();

    let master_key = groups
        .into_iter()
        .filter_map(|g| g.ok())
        .find(|(_key, group)| group.title == recipient)
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
    let group_master_key = find_master_key(recipient, manager).await?;
    let data_message = create_data_message(text_message, group_master_key, timestamp)?;

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, None).await?;

    send(manager, group_master_key, data_message, timestamp).await?;

    Ok(())
}

/// Sends text message to group
pub async fn send_message_to_group_cli(group_name: String, text_message: String) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    send_message(&mut manager, group_name, text_message).await
}
