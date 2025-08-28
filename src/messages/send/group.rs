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
use crate::messages::send::create_attachment;
use crate::{AsyncContactsMap, AsyncRegisteredManager, create_registered_manager};

pub async fn send_message_tui(
    master_key: GroupMasterKeyBytes,
    text_message: String,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let mut manager = manager_mutex.write().await;
    send_message(
        &mut manager,
        master_key,
        text_message,
        current_contacts_mutex,
    )
    .await
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

    let data_message = create_data_message(text_message, &master_key, timestamp)?;

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, current_contacts_mutex).await?;

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
) -> Result<DataMessage> {
    let master_key = master_key.to_vec();
    Ok(DataMessage {
        body: Some(
            text_message
                .parse()
                .map_err(|e| anyhow!("Failed to parse text message: {e}"))?,
        ),
        group_v2: Some(GroupContextV2 {
            master_key: Some(master_key),

            // NOTE: This needs to be checked what does it do
            revision: Some(0),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        ..Default::default()
    })
}

async fn send_attachment(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: GroupMasterKeyBytes,
    text_message: String,
    attachment_path: String,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let attachment_spec = create_attachment(attachment_path).await?;

    let attachment_specs = vec![attachment_spec];

    let attachments: Result<Vec<_>, _> = manager
        .upload_attachments(attachment_specs)
        .await?
        .into_iter()
        .collect();
    let attachments = attachments?;

    let attachment_pointer = attachments
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to get attachment pointer"))?;

    let mut data_message = create_data_message(text_message, &recipient, timestamp)?;
    data_message.attachments = vec![attachment_pointer];

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, current_contacts_mutex).await?;

    send(manager, &recipient, data_message, timestamp).await?;

    Ok(())
}

/// Sends attachment to recipient ( phone number or name ), for usage with TUI
pub async fn send_attachment_tui(
    recipient: GroupMasterKeyBytes,
    text_message: String,
    attachment_path: String,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let mut manager = manager_mutex.write().await;
    send_attachment(
        &mut manager,
        recipient,
        text_message,
        attachment_path,
        current_contacts_mutex,
    )
    .await
}

/// Sends attachment to recipient ( phone number or name ), for usage with CLI
pub async fn send_attachment_cli(
    recipient: String,
    text_message: String,
    attachment_path: String,
) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    let master_key = find_master_key(recipient, &mut manager).await?;
    let master_key = match master_key {
        Some(mk) => mk,
        None => return Err(anyhow!("Group not found.")),
    };
    send_attachment(
        &mut manager,
        master_key,
        text_message,
        attachment_path,
        current_contacts_mutex,
    )
    .await
}
