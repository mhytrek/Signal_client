use std::env;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use futures::{StreamExt, pin_mut};
use presage::libsignal_service::content::ContentBody;
use presage::libsignal_service::prelude::Content;
use presage::libsignal_service::prelude::Uuid;
use presage::manager::Manager;
use presage::manager::Registered;
use presage::model::messages::Received;
use presage::proto::{DataMessage, SyncMessage, sync_message::Sent};
use presage::store::Thread;
use presage::store::{ContentExt, ContentsStore};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};
use tokio::sync::Mutex;
use tracing::trace;

use crate::AsyncContactsMap;
use crate::AsyncRegisteredManager;
use crate::contacts::get_contacts_cli;
use crate::create_registered_manager;
use crate::env::SIGNAL_DISPLAY_FLAGS;

pub enum MessageStatus {
    Sent,
    Delivered,
    Read,
}

pub struct MessageDto {
    pub uuid: Uuid,
    pub timestamp: u64,
    pub text: String,
    pub sender: bool,
    pub status: MessageStatus,
}

async fn loop_no_contents(messages: impl Stream<Item = Received>) {
    pin_mut!(messages);
    while let Some(received) = messages.next().await {
        match received {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(content) => {
                trace!("{:#?}", content.body);
            }
        }
    }
}

async fn loop_with_contents(messages: impl Stream<Item = Received>, contents: &mut Vec<Content>) {
    pin_mut!(messages);
    while let Some(received) = messages.next().await {
        match received {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(content) => {
                trace!("{:#?}", content.body);
                contents.push(*content);
            }
        }
    }
}

/// Function receives messages from the primary device
pub async fn receiving_loop(
    messages: impl Stream<Item = Received>,
    manager: &mut Manager<SqliteStore, Registered>,
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
    manager: &Manager<SqliteStore, Registered>,
    recipient: String,
    from: String,
) -> Result<Vec<Result<Content, SqliteStoreError>>> {
    let recipient_uuid = Uuid::from_str(&recipient)?;
    let thread = Thread::Contact(recipient_uuid);
    let from_u64 = u64::from_str(&from)?;

    Ok(manager
        .store()
        .messages(&thread, from_u64..)
        .await?
        .collect())
}

/// Format `Content` to a MessageDto or returns None
pub fn format_message(content: &Content) -> Option<MessageDto> {
    let timestamp: u64 = content.timestamp();
    let uuid = content.metadata.sender.raw_uuid();
    let mut sender = false;
    let text: Option<String> = match &content.body {
        ContentBody::NullMessage(_) => Some("[NULL] <null message>".to_string()),
        ContentBody::DataMessage(data_message) => match data_message {
            DataMessage {
                body: Some(body), ..
            } => Some(body.to_string()),
            DataMessage {
                flags: Some(flag), ..
            } if env::var(SIGNAL_DISPLAY_FLAGS).is_ok() => {
                Some(format!("[FLAG] Data message (flag: {flag})"))
            }

            _ => None,
        },
        ContentBody::SynchronizeMessage(sync_message) => match sync_message {
            SyncMessage {
                sent:
                    Some(Sent {
                        message: Some(message),
                        ..
                    }),
                ..
            } => {
                sender = true;
                match message {
                    DataMessage {
                        body: Some(body), ..
                    } => Some(body.to_string()),

                    // comment next case to turn off the messages with [FLAG]
                    DataMessage {
                        flags: Some(flag), ..
                    } => Some(format!("[FLAG] Synced data message (flag: {flag})")),

                    _ => None,
                }
            }
            _ => None,
        },
        ContentBody::CallMessage(_) => Some("[CALL]".to_string()),
        ContentBody::ReceiptMessage(_) => None,
        // ContentBody::TypingMessage(_) => Some("Typing...".to_string()),
        ContentBody::TypingMessage(_) => None,
        ContentBody::StoryMessage(_) => Some("[STORY] <story message>".to_string()),
        ContentBody::PniSignatureMessage(_) => None,
        ContentBody::EditMessage(_) => Some("[EDIT] <edit message>".to_string()),
    };
    text.map(|text| MessageDto {
        uuid,
        timestamp,
        text,
        sender,
        status: MessageStatus::Sent,
    })
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in TUI
pub async fn list_messages_tui(
    recipient: String,
    from: String,
    manager_mutex: AsyncRegisteredManager,
) -> Result<Vec<MessageDto>> {
    let manager = manager_mutex.read().await;

    let messages = list_messages(&manager, recipient, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
    }
    Ok(result)
}

/// Function to receive messages for TUI interface
pub async fn receive_messages_tui(
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<Vec<MessageDto>> {
    let mut manager = manager_mutex.write().await;

    let messages = manager.receive_messages().await?;
    let mut contents = Vec::new();

    receiving_loop(
        messages,
        &mut manager,
        Some(&mut contents),
        current_contacts_mutex,
    )
    .await?;

    let mut result = Vec::new();

    for content in contents {
        if let Some(formatted_message) = format_message(&content) {
            result.push(formatted_message);
        }
    }

    Ok(result)
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in CLI
pub async fn list_messages_cli(recipient: String, from: String) -> Result<Vec<MessageDto>> {
    let manager = create_registered_manager().await?;
    let messages = list_messages(&manager, recipient, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
    }
    Ok(result)
}

/// Function to receive messages for CLI interface
pub async fn receive_messages_cli() -> Result<Vec<MessageDto>> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    let messages = manager.receive_messages().await?;
    let mut contents = Vec::new();

    receiving_loop(
        messages,
        &mut manager,
        Some(&mut contents),
        current_contacts_mutex,
    )
    .await?;

    let mut result = Vec::new();

    for content in contents {
        if let Some(formatted_message) = format_message(&content) {
            result.push(formatted_message);
        }
    }

    Ok(result)
}

async fn check_contacts(
    manager: &mut Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let mut current_contacts = current_contacts_mutex.lock().await;

    for contact_res in manager.store().contacts().await? {
        let mut contact = match contact_res {
            Ok(c) => c,
            Err(_) => continue,
        };

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
        // manager.store().to_owned().save_contact(&contact).await?;
        // IT DEFINITELY REQUIRES TESTING
        // However avoids unnecessary copying
        // It SHOULD work as long as receiving loop function is run with manager
        // on write lock or owned instance (shouldn't be a problem, because function needs)
        // a mutable reference, write lock is required for that
        unsafe {
            let store = manager.store() as *const SqliteStore as *mut SqliteStore;
            (*store).save_contact(&contact).await?;
        }
        current_contacts.insert(contact.uuid, contact);
    }
    Ok(())
}
