use anyhow::{Result};
use presage::libsignal_service::prelude::Uuid;
use presage::{Manager};
use presage::libsignal_service::protocol::ServiceId;
use presage::libsignal_service::content::ContentBody;
use presage::model::{identity::OnNewIdentity};
use presage_store_sled::{MigrationConflictStrategy, SledStore, SledStoreError};
use crate::{paths};
use std::time::{SystemTime, UNIX_EPOCH};
use presage::manager::Registered;
use presage::model::contacts::Contact;
use presage::store::ContentsStore;

/// finds contact uuid from string that can be contact_name or contact phone_number
pub async fn find_uuid(recipient_info: String, manager: Manager<SledStore, Registered>) -> Result<Uuid> {
    let contacts: Vec<Result<Contact, SledStoreError>> =
        manager.store().contacts().await?.collect();
    let uuid = contacts.into_iter()
        .filter_map(|c| c.ok())
        .find(|c| c.name == recipient_info || c.phone_number.clone().unwrap().to_string() == recipient_info)
        .map(|c| c.uuid);

    uuid.ok_or_else(|| anyhow::anyhow!("Recipient '{}' not found", recipient_info))
}

/// sends text message to recipient ( phone number or name )
pub async fn send_message(recipient: String, text_message: String) -> Result<()> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    ).await?;

    let mut manager = Manager::load_registered(store).await?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    let recipient_uuid = find_uuid(recipient, manager.clone()).await?;

    let recipient_address = ServiceId::Aci(recipient_uuid.into());

    let data_message = presage::proto::DataMessage {
        body: Some(text_message.parse().map_err(|_| anyhow::anyhow!("Failed to parse text message"))?),
        timestamp: Some(timestamp),
        ..Default::default()
    };

    manager.send_message(recipient_address, ContentBody::from(data_message), timestamp).await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

    let _ = manager.receive_messages().await?;

    Ok(())
}
