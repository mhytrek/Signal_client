use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use presage::proto::data_message::Quote;
use presage::proto::{DataMessage, GroupContextV2};
use presage::{Manager, libsignal_service::zkgroup::GroupMasterKeyBytes, manager::Registered};
use presage_store_sqlite::SqliteStore;
use tracing::error;
use crate::messages::receive::MessageDto;

pub async fn send_message_tui(
    master_key: GroupMasterKeyBytes,
    text_message: String,
    mut manager: Manager<SqliteStore, Registered>,
    quoted_message: Option<MessageDto>
) -> Result<()> {
    send_message(&mut manager, master_key, text_message,quoted_message).await
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    text_message: String,
    quoted_message: Option<MessageDto>
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

    let data_message = create_data_message(text_message, &master_key, timestamp,quoted_message);

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
    quote_message: Option<MessageDto>
) -> DataMessage {
    let master_key = master_key.to_vec();
    let quote = match quote_message {
        Some(mes) => Some(Quote{id:Some(mes.timestamp),text:Some(mes.text), author_aci:Some(mes.uuid.to_string()), ..Default::default()}),
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
