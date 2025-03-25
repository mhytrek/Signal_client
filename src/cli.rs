use anyhow::Result;
use presage::model::contacts::Contact;
use crate::contacts::list_contacts;

fn print_contact(contact: &Contact) {
    println!("Name: {}", contact.name);
    println!("UUID: {}", contact.uuid);
    if let Some(phone_number) = &contact.phone_number {
        println!("Phone number: {}", phone_number);
    }
}

pub async fn print_contacts() -> Result<()> {
    let contacts = list_contacts().await?;
    
    for contact_res in contacts {
        if let Ok(contact) = contact_res {
            print_contact(&contact);
            println!("================");
        }
    }
    Ok(())
}