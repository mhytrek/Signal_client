use crate::{
    contacts::list_contacts_cli,
    groups::list_groups_cli,
    messages::receive::{list_messages_cli, receive_messages_cli, MessageDto},
    profile::{get_my_profile_avatar_cli, get_profile_cli},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use presage::model::{contacts::Contact, groups::Group};

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

fn print_group(group: &Group) {
    println!("Name: {}", group.title);
}

pub async fn print_groups() -> Result<()> {
    let groups: Vec<Group> = list_groups_cli()
        .await?
        .into_iter()
        .flatten()
        .map(|x| x.1)
        .collect();

    for group in groups {
        print_group(&group);
        println!("================");
    }

    Ok(())
}

fn print_message(message: &MessageDto) {
    let millis = message.timestamp;
    let secs = (millis / 1000) as i64;
    let datetime: DateTime<Utc> = DateTime::from_timestamp(secs, 0).expect("Invalid timestamp");

    match message.sender {
        true => {
            println!(
                "[{}] Me -> {}",
                datetime.format("%Y-%m-%d %H:%M:%S"),
                message.text
            );
        }
        false => {
            println!(
                "[{}] Them <- {}",
                datetime.format("%Y-%m-%d %H:%M:%S"),
                message.text
            );
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

    println!(
        "Unrestricted Unidentified Access: {}",
        profile.unrestricted_unidentified_access
    );

    println!("\nAvatar:");
    match get_my_profile_avatar_cli().await {
        Ok(Some(avatar_data)) => match display_avatar_color(&avatar_data) {
            Ok(_) => println!("\n"),
            Err(e) => {
                println!("Could not display avatar in terminal: {}", e);
            }
        },
        Ok(None) => {
            println!("No avatar set");
        }
        Err(e) => {
            println!("Error retrieving avatar: {}", e);
        }
    }

    Ok(())
}

fn display_avatar_color(image_data: &[u8]) -> Result<()> {
    let temp_path = "/tmp/avatar_temp.jpg";
    std::fs::write(temp_path, image_data)?;

    let config = Config {
        width: Some(30),
        height: Some(30),
        absolute_offset: false,
        ..Default::default()
    };

    print_from_file(temp_path, &config)?;

    std::fs::remove_file(temp_path).ok();

    Ok(())
}
