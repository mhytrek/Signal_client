use crate::contacts::list_contacts_cli;
use anyhow::Result;
use presage::model::contacts::Contact;

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
