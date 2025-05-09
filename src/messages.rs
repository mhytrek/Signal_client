use std::str::FromStr;

use crate::create_registered_manager;
use crate::AsyncRegisteredManager;
use anyhow::Result;
use futures::pin_mut;
use futures::StreamExt;
use presage::libsignal_service::prelude::Content;
use presage::libsignal_service::prelude::Uuid;
use presage::manager::Registered;
use presage::model::messages::Received;
use presage::store::ContentsStore;
use presage::store::Thread;
use presage::Manager;
use presage_store_sled::{SledStore, SledStoreError};

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

pub async fn receive_messages_cli() -> Result<Vec<Content>> {
    let mut manager = create_registered_manager().await?;
    let messages = manager.receive_messages().await?;
    pin_mut!(messages);

    let mut contents = Vec::new();
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(content) => {
                contents.push(*content);
            }
        }
    }

    Ok(contents)
}
