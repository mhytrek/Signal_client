use std::env;
use std::str::FromStr;
use std::sync::Arc;

use crate::AsyncContactsMap;
use crate::account_management::create_registered_manager;
use crate::contacts::get_contacts_cli;
use crate::env::SIGNAL_DISPLAY_FLAGS;
use anyhow::Result;
use futures::Stream;
use futures::{StreamExt, pin_mut};
use presage::libsignal_service::content::ContentBody;
use presage::libsignal_service::prelude::Content;
use presage::libsignal_service::prelude::Uuid;
use presage::manager::{Manager, Registered};
use presage::model::messages::Received;
use presage::proto::data_message::Quote;
use presage::proto::{
    AttachmentPointer, DataMessage, GroupContextV2, SyncMessage, sync_message::Sent,
};
use presage::store::{ContentExt, ContentsStore, Thread};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};
use tokio::sync::Mutex;
use tracing::trace;

pub mod contact;
pub mod group;

#[derive(Clone)]
pub struct MessageDto {
    pub uuid: Uuid,
    pub timestamp: u64,
    pub text: String,
    pub sender: bool,
    pub group_context: Option<GroupContextV2>,
    pub attachment: Option<AttachmentPointer>,
    pub quote: Option<Quote>,
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

/// Function receives messages from the primary device, use only in `CLI`.
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

fn format_data_message(data_message: &DataMessage) -> (Option<String>, Option<Quote>) {
    match data_message {
        DataMessage {
            body: Some(body),
            quote,
            ..
        } => {
            let text = body.to_string();
            ((!text.is_empty()).then_some(text), quote.clone())
        }
        DataMessage {
            flags: Some(flag),
            quote,
            ..
        } if env::var(SIGNAL_DISPLAY_FLAGS).is_ok() => (
            Some(format!("[FLAG] Data message (flag: {flag})")),
            quote.clone(),
        ),
        _ => (None, None),
    }
}

/// format Content to a MessageDto or returns None
pub fn format_message(content: &Content) -> Option<MessageDto> {
    let timestamp: u64 = content.timestamp();
    let uuid = content.metadata.sender.raw_uuid();
    let (text, sender, quote) = get_message_text(content);
    let group_context = get_message_group_context(content);
    text.map(|text| MessageDto {
        uuid,
        timestamp,
        text,
        sender,
        group_context,
        attachment: None,
        quote,
    })
}

fn get_message_text(content: &Content) -> (Option<String>, bool, Option<Quote>) {
    let mut sender = false;
    let (text, quote): (Option<String>, Option<Quote>) = match &content.body {
        ContentBody::NullMessage(_) => (Some("[NULL] <null message>".to_string()), None),
        ContentBody::DataMessage(data_message) => format_data_message(data_message),
        ContentBody::SynchronizeMessage(sync_message) => match sync_message {
            SyncMessage {
                sent:
                    Some(Sent {
                        message: Some(data_message),
                        ..
                    }),
                ..
            } => {
                sender = true;
                format_data_message(data_message)
            }
            _ => (None, None),
        },
        ContentBody::CallMessage(_) => (Some("[CALL]".to_string()), None),
        ContentBody::ReceiptMessage(_) => (None, None),
        // ContentBody::TypingMessage(_) => Some("Typing...".to_string()),
        ContentBody::TypingMessage(_) => (None, None),
        ContentBody::StoryMessage(_) => (Some("[STORY] <story message>".to_string()), None),
        ContentBody::PniSignatureMessage(_) => (None, None),
        ContentBody::EditMessage(_) => (Some("[EDIT] <edit message>".to_string()), None),
    };
    (text, sender, quote)
}

fn get_message_group_context(content: &Content) -> Option<GroupContextV2> {
    match &content.body {
        ContentBody::DataMessage(data_msg) => data_msg.group_v2.clone(),
        ContentBody::SynchronizeMessage(sync_msg) => match &sync_msg.sent {
            Some(sent) => match &sent.message {
                Some(data_msg) => data_msg.group_v2.clone(),
                None => None,
            },
            None => None,
        },
        _ => None,
    }
}

/// Map a single AttachmentPointer to MessageDto
fn map_attachment_to_message(
    att: &AttachmentPointer,
    uuid: Uuid,
    timestamp: u64,
    group_context: Option<GroupContextV2>,
) -> MessageDto {
    let file_name = att.file_name.clone().unwrap_or_else(|| {
        let extension = mime_guess::get_mime_extensions_str(att.content_type())
            .and_then(|exts| exts.first().map(|e| e.to_string()))
            .unwrap_or_else(|| "bin".to_string());
        format!("unknown.{extension}")
    });

    MessageDto {
        uuid,
        timestamp,
        text: format!("[ATTACHMENT] {file_name}"),
        sender: true,
        group_context,
        attachment: Some(att.clone()),
        quote: None,
    }
}

/// Format attachements in Content to list of MessageDto
pub fn format_attachments(content: &Content) -> Vec<MessageDto> {
    let timestamp = content.timestamp();
    let uuid = content.metadata.sender.raw_uuid();
    let group_context = get_message_group_context(content);

    match &content.body {
        ContentBody::DataMessage(DataMessage { attachments, .. }) => attachments
            .iter()
            .map(|att| map_attachment_to_message(att, uuid, timestamp, group_context.clone()))
            .collect(),
        ContentBody::SynchronizeMessage(SyncMessage {
            sent:
                Some(Sent {
                    message: Some(DataMessage { attachments, .. }),
                    ..
                }),
            ..
        }) => attachments
            .iter()
            .map(|att| map_attachment_to_message(att, uuid, timestamp, group_context.clone()))
            .collect(),
        _ => vec![],
    }
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in TUI
pub async fn list_messages_tui(
    recipient: String,
    from: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<Vec<MessageDto>> {
    let messages = list_messages(manager, recipient, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
        let attachment_msgs = format_attachments(&message);
        result.extend(attachment_msgs);
    }
    Ok(result)
}

/// Function to receive messages for TUI interface
pub async fn receive_messages_tui(
    manager: &mut Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<Vec<MessageDto>> {
    let messages = manager.receive_messages().await?;
    let mut contents = Vec::new();

    receiving_loop(
        messages,
        manager,
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
        let attachment_msgs = format_attachments(&content);
        result.extend(attachment_msgs);
    }

    Ok(result)
}

pub async fn check_contacts(
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
