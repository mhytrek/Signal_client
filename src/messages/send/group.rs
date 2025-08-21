use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use presage::proto::{DataMessage, GroupContextV2, data_message};
use presage::store::ContentsStore;
use presage::{Manager, libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered};
use presage_store_sqlite::SqliteStore;

async fn find_master_key(
    group_name: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<Option<GroupMasterKeyBytes>> {
    // WARN: Right now it assumes that all groups have unique names this is. This has to be handled
    // differently in future.
    let group = manager
        .store()
        .groups()
        .await?
        .filter_map(|g| g.ok())
        .find(|(_, group)| group.title == group_name);

    let key = group.map(|g| g.0);
    Ok(key)
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    group_name: String,
    text_message: String,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let master_key = find_master_key(group_name, manager).await?;
    let master_key = match master_key {
        Some(mk) => mk,
        None => return Err(anyhow!("Group with given name not found!")),
    };

    let data_message = create_data_message(text_message, master_key, timestamp);

    send(manager, &master_key, data_message, timestamp).await
}

pub async fn send(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: &GroupMasterKeyBytes,
    data_message: DataMessage,
    timestamp: u64,
) -> Result<()> {
    manager
        .send_message_to_group(recipient, data_message, timestamp)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))
}

pub fn create_data_message(
    text_message: String,
    master_key: GroupMasterKeyBytes,
    timestamp: u64,
) -> DataMessage {
    let master_key = master_key.to_vec();
    DataMessage {
        body: Some(text_message),
        group_v2: Some(GroupContextV2 {
            master_key: Some(master_key),
            revision: Some(0),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        ..Default::default()
    }
}
