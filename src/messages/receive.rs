use std::str::FromStr;
use std::sync::Arc;

use futures::Stream;
use futures::{pin_mut, StreamExt};
use presage::manager::Registered;
use presage::model::messages::Received;
use presage_store_sled::{SledStore, SledStoreError};
use presage::store::ContentsStore;
use presage::manager::Manager;
use presage::model::contacts::Contact;
use anyhow::Result;
use presage::libsignal_service::prelude::Content;
use presage::libsignal_service::prelude::Uuid;
use presage::store::Thread;
use tokio::sync::Mutex;

use crate::AsyncContactsMap;
use crate::create_registered_manager;
use crate::AsyncRegisteredManager;
use crate::contacts::get_contacts_cli;

async fn loop_no_contents(messages: impl Stream<Item = Received>) {
    pin_mut!(messages);
    while let Some(received) = messages.next().await {
        match received {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(_) => {}
        }
    }
}

async fn loop_with_contents(
    messages: impl Stream<Item = Received>,
    contents: &mut Vec<Content>,
) {
    pin_mut!(messages);
    while let Some(received) = messages.next().await {
        match received {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(content) => {
                contents.push(*content);
            }
        }
    }
}

/// Function receives messages from the primary device
pub async fn receiving_loop(
    messages: impl Stream<Item = Received>,
    manager: &mut Manager<SledStore, Registered>,
    contents_optional: Option<&mut Vec<Content>>,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    match contents_optional {
        Some(contents) => loop_with_contents(messages, contents).await,
        None => loop_no_contents(messages).await,
    };
    check_contacts(manager, current_contacts_mutex).await

}

async fn list_messages(
    manager: &Manager<SledStore, Registered>,
    recipient: String,
    from: String,
) -> Result<Vec<Result<Content, SledStoreError>>> {
    let recipient_uuid = Uuid::from_str(&recipient)?;
    let thread = Thread::Contact(recipient_uuid);
    let from_u64 = u64::from_str(&from)?;

    Ok(manager.store().messages(&thread,from_u64..).await?.collect())
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in TUI
pub async fn list_messages_tui(
    recipient: String,
    from:String,
    manager_mutex: AsyncRegisteredManager,
) -> Result<Vec<Result<Content, SledStoreError>>> {
    let manager = manager_mutex.read().await;
    list_messages(&manager,recipient, from).await
}


/// Returns iterator over stored messeges from certain time for given contact uuid, for use in CLI
pub async fn list_messages_cli(recipient: String, from:String) -> Result<Vec<Result<Content, SledStoreError>>> {
    let manager = create_registered_manager().await?;
    list_messages(&manager,recipient, from).await
    // print!("{:?}", mess);
}

/// Function to receive messages for CLI interface
pub async fn receive_messages_cli() -> Result<Vec<Content>> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap = Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    let messages = manager.receive_messages().await?;
    let mut contents = Vec::new();

    receiving_loop(
        messages,
        &mut manager,
        Some(&mut contents),
        current_contacts_mutex
    );

    Ok(contents)
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
