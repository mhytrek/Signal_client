use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use presage::libsignal_service::protocol::ServiceId;
use presage::proto::DataMessage;
use presage::store::ContentsStore;
use presage::{
    Manager, libsignal_service::prelude::Uuid, manager::Registered, model::contacts::Contact,
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};
use tokio::sync::Mutex;

use crate::contacts::get_contacts_cli;
use crate::messages::receive::receiving_loop;
use crate::{AsyncContactsMap, AsyncRegisteredManager, create_registered_manager};

/// finds contact uuid from string that can be contact_name or contact phone_number
pub async fn find_uuid(
    recipient_info: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<Uuid> {
    let contacts: Vec<Result<Contact, SqliteStoreError>> =
        manager.store().contacts().await?.collect();
    let uuid = contacts
        .into_iter()
        .filter_map(|c| c.ok())
        .find(|c| {
            // Compare first by name, then by phone number
            // and finally by UUID
            (c.name == recipient_info)
                || (c.phone_number.is_some()
                    && c.phone_number.clone().unwrap().to_string() == recipient_info)
                || (c.uuid.to_string() == recipient_info)
        })
        .map(|c| c.uuid);

    uuid.ok_or_else(|| anyhow::anyhow!("Recipient '{}' not found", recipient_info))
}

pub async fn get_address(
    recipient: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<ServiceId> {
    let recipient_uuid = find_uuid(recipient, manager).await?;
    // let recipient_uuid = Uuid::from_str(&recipient)?;
    Ok(ServiceId::Aci(recipient_uuid.into()))
}

pub fn create_data_message(text_message: String, timestamp: u64) -> Result<DataMessage> {
    let data_msg = DataMessage {
        body: Some(
            text_message
                .parse()
                .map_err(|_| anyhow::anyhow!("Failed to parse text message!"))?,
        ),
        timestamp: Some(timestamp),
        ..Default::default()
    };
    Ok(data_msg)
}

pub async fn send(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient_addr: ServiceId,
    data_message: DataMessage,
    timestamp: u64,
) -> Result<()> {
    manager
        .send_message(recipient_addr, data_message, timestamp)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
    Ok(())
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: String,
    text_message: String,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = get_address(recipient, manager).await?;
    let data_message = create_data_message(text_message, timestamp)?;

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, current_contacts_mutex).await?;

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

/// sends text message to recipient ( phone number or name ), for usage with TUI
pub async fn send_message_tui(
    recipient: String,
    text_message: String,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    // let mut manager = create_registered_manager().await?;
    let mut manager = manager_mutex.write().await;
    send_message(
        &mut manager,
        recipient,
        text_message,
        current_contacts_mutex,
    )
    .await
}

/// sends text message to recipient ( phone number or name ), for usage with CLI
pub async fn send_message_cli(recipient: String, text_message: String) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    send_message(
        &mut manager,
        recipient,
        text_message,
        current_contacts_mutex,
    )
    .await
}
