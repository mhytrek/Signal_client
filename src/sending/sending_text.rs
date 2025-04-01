use presage::libsignal_service::prelude::Uuid;
use presage::{Manager};
use presage::libsignal_service::protocol::ServiceId;
use presage::libsignal_service::content::ContentBody;
use presage::model::{identity::OnNewIdentity};
use presage_store_sled::{MigrationConflictStrategy, SledStore};
use crate::paths;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn send_message(recipient: Uuid, text_message: String) -> Result<(), Box<dyn std::error::Error>> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    ).await?;

    let mut manager = Manager::load_registered(store).await?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    let recipient_address = ServiceId::Aci(recipient.into());

    manager.send_message(recipient_address, ContentBody::DataMessage(text_message.into()), timestamp).await.expect("Failed to send message");

    Ok(())
}


