use anyhow::Result;
use presage::libsignal_service::content::ContentBody;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::protocol::ServiceId;
use presage::manager::Registered;
use presage::model::contacts::Contact;
use presage::store::ContentsStore;
use presage::Manager;
use presage_store_sled::{SledStore, SledStoreError};
use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use crate::{contacts::{receiving_loop, sync_contacts_tui}, create_registered_manager, AsyncRegisteredManager};

/// finds contact uuid from string that can be contact_name or contact phone_number
pub async fn find_uuid(
    recipient_info: String,
    manager_mutex: AsyncRegisteredManager,
) -> Result<Uuid> {
    let manager = manager_mutex.lock().await;
    let contacts: Vec<Result<Contact, SledStoreError>> =
        manager.store().contacts().await?.collect();
    let uuid = contacts
        .into_iter()
        .filter_map(|c| c.ok())
        .find(|c| {
            c.name == recipient_info
                || c.phone_number.clone().unwrap().to_string() == recipient_info
        })
        .map(|c| c.uuid);

    uuid.ok_or_else(|| anyhow::anyhow!("Recipient '{}' not found", recipient_info))
}

/// sends text message to recipient ( phone number or name )
pub async fn send_message(recipient: String, text_message: String, manager_mutex: AsyncRegisteredManager) -> Result<()> {
    // let mut manager = create_registered_manager().await?;

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    
    let new_mutex = Arc::clone(&manager_mutex);
    let recipient_uuid = find_uuid(recipient, new_mutex).await?;

    let recipient_address = ServiceId::Aci(recipient_uuid.into());

    let data_message = presage::proto::DataMessage {
        body: Some(
            text_message
                .parse()
                .map_err(|_| anyhow::anyhow!("Failed to parse text message"))?,
        ),
        timestamp: Some(timestamp),
        ..Default::default()
    };

    println!("Sending");

    let mut manager = manager_mutex.lock().await;

    let messages = manager.receive_messages().await?;
    receiving_loop(messages).await;

    manager
        .send_message(
            recipient_address,
            data_message,
            timestamp,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

    // let _ = manager.receive_messages().await?;

    println!("Send");

    Ok(())
}
