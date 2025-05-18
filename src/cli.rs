use crate::{
    contacts::list_contacts_cli,
    profile::get_profile_cli,
    messages::receive::{list_messages_cli, receive_messages_cli, MessageDto},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use presage::{
    model::contacts::Contact,
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

fn print_message(message:&MessageDto){
    let millis = message.timestamp;
    let secs = (millis / 1000) as i64;
    let datetime:DateTime<Utc> = DateTime::from_timestamp(secs, 0)
        .expect("Invalid timestamp"); 
                        
    match message.sender{
        true => {
            println!("[{}] Me -> {}", datetime.format("%Y-%m-%d %H:%M:%S"), message.text);  
        }
        false =>{
            println!("[{}] Them <- {}", datetime.format("%Y-%m-%d %H:%M:%S"), message.text);  

        }
    }
}
pub async fn print_messages(recipient: String, from: String) -> Result<()> {

    let messages = list_messages_cli(recipient, from).await?;
    for message in messages {
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

pub async fn print_profile() -> Result<()> {
    let profile = get_profile_cli().await?;

    println!("Profile:");
    if let Some(name) = &profile.name {
        println!("Name: {}", name);
    } else {
        println!("Name: N/A");
    }
    if let Some(about) = &profile.about {
        println!("About: {}", about);
    } else {
        println!("About: N/A");
    }
    if let Some(about_emoji) = &profile.about_emoji {
        println!("About Emoji: {}", about_emoji);
    } else {
        println!("About Emoji: N/A");
    }
    if let Some(avatar) = &profile.avatar {
        println!("Avatar: {}", avatar);
    } else {
        println!("Avatar: N/A");
    }
    println!(
        "Unrestricted Unidentified Access: {}",
        profile.unrestricted_unidentified_access
    );

    Ok(())
}
