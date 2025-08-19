use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use presage::proto::{DataMessage, GroupContextV2};
use presage::{Manager, libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered};
use presage_store_sqlite::SqliteStore;
use tokio::sync::Mutex;
use tracing::error;

use crate::contacts::get_contacts_cli;
use crate::groups::find_master_key;
use crate::messages::receive::receiving_loop;
use crate::{AsyncContactsMap, create_registered_manager};

pub async fn send_message_tui(
    master_key: GroupMasterKeyBytes,
    text_message: String,
    manager: &mut Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    send_message(manager, master_key, text_message, current_contacts_mutex).await
}

pub async fn send_message_cli(group_name: String, text_message: String) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    let master_key = find_master_key(group_name, &mut manager).await?;
    let master_key = match master_key {
        Some(mk) => mk,
        None => return Err(anyhow!("Group doesn't exist.")),
    };
    send_message(
        &mut manager,
        master_key,
        text_message,
        current_contacts_mutex,
    )
    .await
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    text_message: String,
    current_contacts_mutex: AsyncContactsMap,
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
