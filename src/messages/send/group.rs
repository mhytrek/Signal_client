use std::time::{SystemTime, UNIX_EPOCH};

use crate::account_management::create_registered_manager;
use crate::groups::find_master_key;
use crate::messages::receive::MessageDto;
use anyhow::{Result, bail};
use presage::proto::data_message::{Delete, Quote};
use presage::proto::{DataMessage, GroupContextV2};
use presage::{Manager, libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered};
use presage_store_sqlite::SqliteStore;
use tracing::error;

use crate::messages::attachments::create_attachment;

pub async fn send_message_tui(
    master_key: GroupMasterKeyBytes,
    text_message: String,
    mut manager: Manager<SqliteStore, Registered>,
    quoted_message: Option<MessageDto>,
) -> Result<()> {
    send_message(&mut manager, master_key, text_message, quoted_message).await
}

pub async fn send_delete_message_tui(
    master_key: GroupMasterKeyBytes,
    mut manager: Manager<SqliteStore, Registered>,
    target_send_timestamp: u64,
) -> Result<()> {
    send_delete_message(&mut manager, master_key, target_send_timestamp).await
}

pub async fn send_delete_message_cli(recipient: String, target_send_timestamp: u64) -> Result<()> {
    let mut manager = create_registered_manager().await?;

    let master_key = find_master_key(recipient, &mut manager).await?;
    let master_key = match master_key {
        Some(mk) => mk,
        None => bail!("Group with given name does not exist."),
    };

    send_delete_message(&mut manager, master_key, target_send_timestamp).await
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    text_message: String,
    quoted_message: Option<MessageDto>,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

    let data_message = create_data_message(text_message, &master_key, timestamp, quoted_message);

    let send_result = send(manager, &master_key, data_message, timestamp).await;
    if let Err(e) = send_result {
        error!("{e}");
    }
    Ok(())
}

async fn send_delete_message(
    manager: &mut Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    target_send_timestamp: u64,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let data_message = create_delete_data_message(&master_key, timestamp, target_send_timestamp);

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
    quote_message: Option<MessageDto>,
) -> DataMessage {
    let master_key = master_key.to_vec();
    let quote = match quote_message {
        Some(mes) => Some(Quote {
            id: Some(mes.timestamp),
            text: Some(mes.text),
            author_aci: Some(mes.uuid.to_string()),
            ..Default::default()
        }),
        None => None,
    };
    DataMessage {
        body: Some(text_message),
        group_v2: Some(GroupContextV2 {
            master_key: Some(master_key),

            // NOTE: This needs to be checked what does it do
            revision: Some(0),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        quote,
        ..Default::default()
    }
}

/// Send message with attachment
async fn send_attachment(
    manager: &mut Manager<SqliteStore, Registered>,
    master_key: &GroupMasterKeyBytes,
    text_message: String,
    attachment_path: String,
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

    let mut data_message = create_data_message(text_message, master_key, timestamp, None);
    data_message.attachments = vec![attachment_pointer];

    send(manager, master_key, data_message, timestamp).await?;

    Ok(())
}

/// sends attachment to recipient ( phone number or name ), for usage with TUI
pub async fn send_attachment_tui(
    master_key: &GroupMasterKeyBytes,
    text_message: String,
    attachment_path: String,
    mut manager: Manager<SqliteStore, Registered>,
) -> Result<()> {
    send_attachment(&mut manager, master_key, text_message, attachment_path).await
}

pub fn create_delete_data_message(
    master_key: &GroupMasterKeyBytes,
    timestamp: u64,
    target_send_timestamp: u64,
) -> DataMessage {
    let master_key = master_key.to_vec();
    let del_mes = Delete {
        target_sent_timestamp: Some(target_send_timestamp),
    };

    DataMessage {
        group_v2: Some(GroupContextV2 {
            master_key: Some(master_key),

            // NOTE: This needs to be checked what does it do
            revision: Some(0),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        delete: Some(del_mes),
        ..Default::default()
    }
}
