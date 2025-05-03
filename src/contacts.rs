use crate::create_registered_manager;
use crate::messages::receive::receiving_loop;
use crate::AsyncRegisteredManager;
use anyhow::Result;
use presage::libsignal_service::prelude::Uuid;
use presage::manager::Registered;
use presage::model::contacts::Contact;
use presage::store::ContentsStore;
use presage::Manager;
use presage_store_sled::{SledStore, SledStoreError};
use std::collections::HashMap;

async fn sync_contacts(manager: &mut Manager<SledStore, Registered>) -> Result<()> {
    let messages = manager.receive_messages().await?;
    receiving_loop(messages).await;
    manager.request_contacts().await?;
    let messages = manager.receive_messages().await?;
    receiving_loop(messages).await;
    Ok(())
}

/// Function to sync contacts when CLI is used
pub async fn sync_contacts_cli() -> Result<()> {
    let mut manager = create_registered_manager().await?;
    sync_contacts(&mut manager).await
}

/// Function to sync contacts when TUI is used
pub async fn sync_contacts_tui(manager_mutex: AsyncRegisteredManager) -> Result<()> {
    let mut manager = manager_mutex.write().await;
    sync_contacts(&mut manager).await
}

async fn get_contacts(manager: &Manager<SledStore, Registered>) -> Result<HashMap<Uuid, Contact>> {
    let contact_vec = list_contacts(manager).await?;

    // No error handling for now, however it'll have to be done
    let mut contacts_map: HashMap<Uuid, Contact> = HashMap::new();
    for contact in contact_vec.into_iter().flatten() {
        let uuid = contact.uuid;
        contacts_map.insert(uuid, contact);
    }
    Ok(contacts_map)
}

pub async fn get_contacts_cli() -> Result<HashMap<Uuid, Contact>> {
    let manager = create_registered_manager().await?;
    get_contacts(&manager).await
}

pub async fn get_contacts_tui(
    manager_mutex: AsyncRegisteredManager,
) -> Result<HashMap<Uuid, Contact>> {
    let manager = manager_mutex.read().await;
    get_contacts(&manager).await
}

async fn list_contacts(
    manager: &Manager<SledStore, Registered>,
) -> Result<Vec<Result<Contact, SledStoreError>>> {
    Ok(manager.store().contacts().await?.collect())
}

/// Returns iterator over stored contacts, for use in CLI
pub async fn list_contacts_cli() -> Result<Vec<Result<Contact, SledStoreError>>> {
    let manager = create_registered_manager().await?;
    list_contacts(&manager).await
}

/// Returns iterator over stored contacts, for use in CLI
pub async fn list_contacts_tui(
    manager_mutex: AsyncRegisteredManager,
) -> Result<Vec<Result<Contact, SledStoreError>>> {
    let manager = manager_mutex.read().await;
    list_contacts(&manager).await
}
