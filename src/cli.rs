use crate::{
    contacts::list_contacts_cli,
    messages::receive::{list_messages_cli, receive_messages_cli},
};
use anyhow::Result;
use presage::{
    libsignal_service::{content::ContentBody, prelude::Content},
    model::contacts::Contact,
    proto::{sync_message::Sent, DataMessage, SyncMessage},
    store::ContentExt,
};

fn print_contact(contact: &Contact) {
    println!("Name: {}", contact.name);
    println!("UUID: {}", contact.uuid);
    if let Some(phone_number) = &contact.phone_number {
        println!("Phone number: {}", phone_number);
    }
}

pub async fn print_contacts() -> Result<()> {
    let contacts = list_contacts_cli().await?;

    for contact in contacts.into_iter().flatten() {
        print_contact(&contact);
        println!("================");
    }
    Ok(())
}

fn print_message(content: &Content) {
    let ts = content.timestamp();
    let text: Option<String> = match &content.body {
        ContentBody::NullMessage(_) => Some("[NULL] <null message>".to_string()),
        ContentBody::DataMessage(data_message) => match data_message {
            DataMessage {
                body: Some(body), ..
            } => Some(format!(
                "-> Them [{}]: {}",
                content.metadata.sender.raw_uuid(),
                body
            )),
            // DataMessage{attachments:_,..} =>{
            //     Some(format!("-> Them [{}]: [ATTACHMENT]", content.metadata.sender.raw_uuid()))
            // }
            DataMessage {
                flags: Some(flag), ..
            } => Some(format!("[FLAG] Data message (flag: {})", flag)),
            // _ => Some("[DATA?] <unhandled data message>".to_string()),
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
            } => match message {
                DataMessage {
                    body: Some(body), ..
                } => Some(format!(
                    "<- Me [{}]: {}",
                    content.metadata.sender.raw_uuid(),
                    body
                )),
                // DataMessage{attachments:_,..} =>{
                //     Some(format!("<- Me [{}]: [ATTACHMENT]", content.metadata.sender.raw_uuid()))
                // }
                DataMessage {
                    flags: Some(flag), ..
                } => Some(format!("[FLAG] Synced data message (flag: {})", flag)),
                // _ => Some("[SYNC?] <unhandled synchronized data message>".to_string()),
                _ => None,
            },
            // _ => Some("[SYNC?] <unhandled sync message>".to_string()),
            _ => None,
        },
        ContentBody::CallMessage(_) => Some("[CALL] <call message>".to_string()),
        ContentBody::ReceiptMessage(_) => Some("[RECEIPT] <receipt message>".to_string()),
        ContentBody::TypingMessage(_) => Some("[TYPING] <typing message>".to_string()),
        ContentBody::StoryMessage(_) => Some("[STORY] <story message>".to_string()),
        ContentBody::PniSignatureMessage(_) => {
            Some("[SIGNATURE] <pni signature message>".to_string())
        }
        ContentBody::EditMessage(_) => Some("[EDIT] <edit message>".to_string()),
    };

    if let Some(text) = text {
        println!("[{}] {}", ts, text);
    }
}

pub async fn print_messages(recipient: String, from: String) -> Result<()> {
    let messages = list_messages_cli(recipient, from).await?;
    for message in messages.into_iter().flatten() {
        print_message(&message);
    }
    Ok(())
}

pub async fn print_received_message() -> Result<()> {
    let messages = receive_messages_cli().await?;
    for message in messages {
        print_message(&message);
    }
    Ok(())
}
