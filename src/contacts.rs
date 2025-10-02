use crate::AsyncContactsMap;
use crate::account_management::create_registered_manager;
use crate::messages::receive::receiving_loop;
use anyhow::Result;
use presage::Manager;
use presage::libsignal_service::prelude::Uuid;
use presage::manager::Registered;
use presage::model::contacts::Contact;
use presage::store::ContentsStore;
use presage_store_sqlite::{SqliteStore, SqliteStoreError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn sync_contacts(
    manager: &mut Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let messages = manager.receive_messages().await?;

    let new_contacts_mutex = Arc::clone(&current_contacts_mutex);
    receiving_loop(messages, manager, None, new_contacts_mutex).await?;
    manager.request_contacts().await?;
    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, current_contacts_mutex).await?;
    Ok(())
}

/// Function to sync contacts when CLI is used
pub async fn sync_contacts_cli() -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts(&manager).await?));
    sync_contacts(&mut manager, current_contacts_mutex).await
}

async fn get_contacts(
    manager: &Manager<SqliteStore, Registered>,
) -> Result<HashMap<Uuid, Contact>> {
    let contact_vec = list_contacts(manager).await?;

    // No error handling for now, however it'll have to be done
    let mut contacts_map: HashMap<Uuid, Contact> = HashMap::new();
    for contact in contact_vec.into_iter().flatten() {
        let uuid = contact.uuid;
        contacts_map.insert(uuid, contact);
    }
    Ok(contacts_map)
}

pub async fn get_contacts_cli(
    manager: &Manager<SqliteStore, Registered>,
) -> Result<HashMap<Uuid, Contact>> {
    // let manager = create_registered_manager().await?;
    get_contacts(manager).await
}

pub async fn get_contacts_tui(
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<HashMap<Uuid, Contact>> {
    get_contacts(manager).await
}

async fn list_contacts(
    manager: &Manager<SqliteStore, Registered>,
) -> Result<Vec<Result<Contact, SqliteStoreError>>> {
    Ok(manager.store().contacts().await?.collect())
}

/// Returns iterator over stored contacts, for use in CLI
pub async fn list_contacts_cli() -> Result<Vec<Result<Contact, SqliteStoreError>>> {
    let manager = create_registered_manager().await?;
    list_contacts(&manager).await
}

/// Returns iterator over stored contacts, for use in TUI
pub async fn list_contacts_tui(
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<Vec<Result<Contact, SqliteStoreError>>> {
    list_contacts(manager).await
}

pub async fn initial_sync_cli(manager: &mut Manager<SqliteStore, Registered>) -> Result<()> {
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts(manager).await?));
    sync_contacts(manager, current_contacts_mutex).await
}
