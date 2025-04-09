use anyhow::Result;
use futures::Stream;
use futures::{pin_mut, StreamExt};
use presage::model::contacts::Contact;
use presage::model::messages::Received;
use presage::store::ContentsStore;
use presage_store_sled::SledStoreError;
use crate::create_registered_manager;
use crate::AsyncRegisteredManager;

pub async fn receiving_loop(messages: impl Stream<Item = Received>) {
    pin_mut!(messages);
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => {},
            Received::Content(_) => continue,
        }
    }
}

/// Function to sync contacts when CLI is used
pub async fn sync_contacts_cli() -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let messages = manager.receive_messages().await?;
    receiving_loop(messages).await;
    manager.request_contacts().await?;
    let messages = manager.receive_messages().await?;
    receiving_loop(messages).await;
    Ok(())
}

/// Function to sync contacts when TUI is used
pub async fn sync_contacts_tui(manager_mutex: AsyncRegisteredManager) -> Result<()> {
    let mut manager = manager_mutex.lock().await;
    let messages = manager.clone().receive_messages().await?;
    receiving_loop(messages).await;
    manager.request_contacts().await?;
    let messages = manager.clone().receive_messages().await?;
    receiving_loop(messages).await;
    Ok(())
}

/// Returns iterator over stored contacts, for use in CLI
pub async fn list_contacts_cli() -> Result<Vec<Result<Contact, SledStoreError>>> {
    let manager = create_registered_manager().await?;
    let contacts: Vec<Result<Contact, SledStoreError>> =
        manager.store().contacts().await?.collect();
    Ok(contacts)
}

/// Returns iterator over stored contacts, for use in CLI
pub async fn list_contacts_tui(manager_mutex: AsyncRegisteredManager) -> Result<Vec<Result<Contact, SledStoreError>>> {
    let manager = manager_mutex.lock().await;
    let contacts: Vec<Result<Contact, SledStoreError>> =
        manager.store().contacts().await?.collect();
    Ok(contacts)
}