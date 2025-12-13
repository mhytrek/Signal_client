use crate::{
    contacts::list_contacts_cli,
    groups::list_groups_cli,
    messages::receive::{MessageDto, contact, group, receive_messages_cli},
    profile::{get_my_profile_avatar_cli, get_profile_cli},
    ui::utils::get_local_timestamp,
};
use anyhow::Result;
use presage::model::{contacts::Contact, groups::Group};
use viuer::{Config, print_from_file};

fn print_contact(contact: &Contact) {
    println!("Name: {}", contact.name);
    println!("UUID: {}", contact.uuid);
    if let Some(phone_number) = &contact.phone_number {
        println!("Phone number: {phone_number}");
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
    if let Some(desc) = &group.description {
        println!("Description: {desc}");
    }
}

pub async fn print_groups() -> Result<()> {
    let groups = list_groups_cli().await?;

    let groups = groups
        .into_iter()
        .flatten()
        .map(|(_, group)| group)
        .collect::<Vec<_>>();

    for group in groups {
        print_group(&group);
        println!("================");
    }

    Ok(())
}

fn print_message(message: &MessageDto) {
    print_quote(message);
    print_body(message);
    print_reactions(message);
    println!();
}

fn print_quote(message: &MessageDto) {
    if let Some(qu) = &message.quote {
        let datetime_quote_local = get_local_timestamp(qu.id());

        println!(
            "┆ Reply:\n┆ {}\n┆ {}",
            datetime_quote_local.format("%Y-%m-%d %H:%M:%S"),
            qu.text.clone().unwrap_or("<no text>".to_string())
        );
    }
}

fn print_body(message: &MessageDto) {
    let datetime_local = get_local_timestamp(message.timestamp);

    if message.sender {
        println!(
            "[{}] Me -> {}",
            datetime_local.format("%Y-%m-%d %H:%M:%S"),
            message.text
        );
    } else {
        println!(
            "[{}] Them <- {}",
            datetime_local.format("%Y-%m-%d %H:%M:%S"),
            message.text
        );
    }
}

fn print_reactions(message: &MessageDto) {
    if message.reactions.is_empty() {
        return;
    }

    for reaction in message.reactions.values() {
        let removed = reaction.remove.unwrap_or(false);
        if removed {
            continue;
        }

        let emoji = reaction.emoji.clone().unwrap_or("?".to_string());
        println!("┆ Reaction: {emoji}");
    }
}
pub async fn print_messages_from_contact(recipient: String, from: Option<String>) -> Result<()> {
    let mut messages = contact::list_messages_cli(recipient, from).await?;

    // reversing the order of messages to print them out from the oldest to the latest
    messages.reverse();

    for message in messages {
        print_message(&message);
    }
    Ok(())
}

pub async fn print_messages_from_group(group: String, from: Option<String>) -> Result<()> {
    let mut messages = group::list_messages_cli(group, from).await?;

    messages.reverse();

    for message in messages {
        print_message(&message);
    }
    Ok(())
}

pub async fn print_received_message() -> Result<()> {
    let mut messages = receive_messages_cli().await?;

    // reversing the order of messages to print them out from the oldest to the latest
    messages.reverse();

    for message in messages {
        print_message(&message);
    }
    Ok(())
}

pub async fn print_profile() -> Result<()> {
    let profile = get_profile_cli().await?;

    println!("Profile:");
    if let Some(name) = &profile.name {
        println!("Name: {name}");
    } else {
        println!("Name: N/A");
    }
    if let Some(about) = &profile.about {
        println!("About: {about}");
    } else {
        println!("About: N/A");
    }
    if let Some(about_emoji) = &profile.about_emoji {
        println!("About Emoji: {about_emoji}");
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
                println!("Could not display avatar in terminal: {e}");
            }
        },
        Ok(None) => {
            println!("No avatar set");
        }
        Err(e) => {
            println!("Error retrieving avatar: {e}");
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
