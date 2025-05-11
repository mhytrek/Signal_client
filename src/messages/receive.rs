use futures::Stream;
use futures::{pin_mut, StreamExt};
use presage::manager::Registered;
use presage::model::messages::Received;
use presage_store_sled::SledStore;
use presage::store::ContentsStore;
use presage::manager::Manager;
use presage::model::contacts::Contact;
use anyhow::Result;

use crate::AsyncContactsMap;

/// Function receives messages from the primary device
pub async fn receiving_loop(
    messages: impl Stream<Item = Received>,
    manager: &Manager<SledStore, Registered>,
    current_contacts_mutex: AsyncContactsMap
) -> Result<()> {
    pin_mut!(messages);
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(_) => continue,
        }
    }
    check_contacts(manager, current_contacts_mutex).await

}

async fn check_contacts(
    manager: &mut Manager<SledStore, Registered>,
    current_contacts_mutex: AsyncContactsMap
) -> Result<()> {
    let mut current_contacts = current_contacts_mutex.lock().await;
    let store_contacts: Vec<Contact> = manager
        .store()
        .contacts()
        .await?
        .filter_map(|c_res| c_res.ok())
        .collect();
    
    for mut contact in store_contacts {
        let old_contact = match current_contacts.get(&contact.uuid) {
            Some(c) => c,
            None => continue,
        };

        if contact.name.is_empty() && !old_contact.name.is_empty() {
            contact.name = old_contact.name.clone(); 
        }

        if contact.phone_number.is_none() && old_contact.phone_number.is_some() {
            contact.phone_number = old_contact.phone_number.clone();
        }

        // Maybe it works, maybe it doesn't, requires behavioral testing
        manager.store().to_owned().save_contact(&contact).await?;
        current_contacts.insert(contact.uuid, contact);

    }
    
    Ok(())
}
