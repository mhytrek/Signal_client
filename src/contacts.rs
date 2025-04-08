use anyhow::Result;
use futures::{pin_mut, StreamExt};
use presage::Manager;
use presage::manager::Registered;
use presage::model::contacts::Contact;
use presage::model::messages::Received;
use presage::store::ContentsStore;
use presage_store_sled::{SledStore, SledStoreError};
use crate::create_registered_manager;

/// Function to sync contacts
pub async fn sync_contacts() -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let messages = manager.receive_messages().await?;
    pin_mut!(messages);
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => {},
            Received::Content(_) => continue,
        }
    }
    manager.request_contacts().await?;
    let messages = manager.receive_messages().await?;
    pin_mut!(messages);
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => {},
            Received::Content(_) => continue,
        };
    }
    Ok(())
}

/// Returns iterator over stored contacts
pub async fn list_contacts() -> Result<Vec<Result<Contact, SledStoreError>>> {
    let manager = create_registered_manager().await?;
    let contacts: Vec<Result<Contact, SledStoreError>> =
        manager.store().contacts().await?.collect();
    Ok(contacts)
}
