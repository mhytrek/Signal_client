use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use presage::proto::{DataMessage, GroupContextV2};
use presage::{Manager, libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered};
use presage_store_sqlite::SqliteStore;
use tracing::error;

use crate::create_registered_manager;
use crate::groups::find_master_key;

pub async fn send_message_tui(
    master_key: GroupMasterKeyBytes,
    text_message: String,
    mut manager: Manager<SqliteStore, Registered>,
) -> Result<()> {
    send_message(&mut manager, master_key, text_message).await
}

pub async fn send_message_cli(group_name: String, text_message: String) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let master_key = find_master_key(group_name, &mut manager).await?;
    let master_key = match master_key {
        Some(mk) => mk,
        None => return Err(anyhow!("Group doesn't exist.")),
    };
    send_message(&mut manager, master_key, text_message).await
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    text_message: String,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

    let data_message = create_data_message(text_message, &master_key, timestamp);

    let send_result = send(manager, &master_key, data_message, timestamp).await;
    if let Err(e) = send_result {
        error!("{e}");
    }
    Ok(())
}

pub async fn send(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: &GroupMasterKeyBytes,
    data_message: DataMessage,
    timestamp: u64,
) -> Result<()> {
    manager
        .send_message_to_group(recipient, data_message.clone(), timestamp)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))
}

pub fn create_data_message(
    text_message: String,
    master_key: &GroupMasterKeyBytes,
    timestamp: u64,
) -> DataMessage {
    let master_key = master_key.to_vec();
    DataMessage {
        body: Some(text_message),
        group_v2: Some(GroupContextV2 {
            master_key: Some(master_key),

            // NOTE: This needs to be checked what does it do
            revision: Some(0),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        ..Default::default()
    }
}
