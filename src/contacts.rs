use anyhow::Result;
use futures::{pin_mut, StreamExt};
use presage::model::messages::Received;
use presage::Manager;
use presage::model::{identity::OnNewIdentity, contacts::Contact};
use presage::store::ContentsStore;
use presage_store_sled::{MigrationConflictStrategy, SledStore, SledStoreError};

use crate::paths;

/// Function to sync contacts
pub async fn sync_contacts() -> Result<()> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    )
    .await?;
    let mut manager = Manager::load_registered(store).await?;
    let messages = manager.receive_messages().await?;
    pin_mut!(messages);
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => println!("Got contacts!"),
            Received::Content(_) => continue,
        }
    }
    Ok(())
}

/// Returns iterator over stored contacts
pub async fn list_contacts() -> Result<Vec<Result<Contact, SledStoreError>>> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    )
    .await?;

    
    let manager = Manager::load_registered(store).await?;
    let contacts: Vec<Result<Contact, SledStoreError>> = manager.store().contacts().await?.collect();
    Ok(contacts)
}