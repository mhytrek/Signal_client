use anyhow::Result;
use presage::libsignal_service::groups_v2::{AccessControl, Member, Role};
use presage::libsignal_service::prelude::{ProfileKey, Uuid};
use presage::model::contacts::Contact;
use presage::model::groups::Group;
use presage::store::ContentsStore;
use presage_store_sqlite::SqliteStore;
use std::collections::HashMap;

pub async fn inject_contact(
    store: &SqliteStore,
    name: &str,
    uuid: Uuid,
    phone: Option<String>,
) -> Result<()> {
    let contact = Contact {
        uuid,
        name: name.to_string(),
        phone_number: phone.and_then(|p| p.parse().ok()),
        color: None,
        verified: Default::default(),
        profile_key: Default::default(),
        expire_timer: 0,
        expire_timer_version: 0,
        inbox_position: 0,
        archived: false,
        avatar: None,
    };

    unsafe {
        let store_ptr = store as *const SqliteStore as *mut SqliteStore;
        (*store_ptr).save_contact(&contact).await?;
    }

    Ok(())
}

pub async fn inject_group(
    store: &SqliteStore,
    title: &str,
    master_key: [u8; 32],
    members: Vec<Uuid>,
) -> Result<()> {
    let group_members: Vec<Member> = members
        .into_iter()
        .map(|uuid| Member {
            uuid: uuid.into(),
            role: Role::Default,
            profile_key: ProfileKey::create([0u8; 32]),
            joined_at_revision: 0,
        })
        .collect();

    let group = Group {
        title: title.to_string(),
        avatar: String::new(),
        description: None,
        members: group_members,
        revision: 0,
        invite_link_password: Vec::new(),
        access_control: Some(AccessControl {
            attributes: 1.try_into()?,
            members: 1.try_into()?,
            add_from_invite_link: 0.try_into()?,
        }),
        disappearing_messages_timer: None,
        pending_members: Vec::new(),
        requesting_members: Vec::new(),
    };

    unsafe {
        let store_ptr = store as *const SqliteStore as *mut SqliteStore;
        (*store_ptr).save_group(master_key, group).await?;
    }

    Ok(())
}

pub async fn get_all_contacts(store: &SqliteStore) -> Result<HashMap<Uuid, Contact>> {
    let contacts = store.contacts().await?;
    let mut map = HashMap::new();

    for contact_result in contacts {
        let contact = contact_result?;
        map.insert(contact.uuid, contact);
    }

    Ok(map)
}

pub fn generate_test_master_key(seed: u8) -> [u8; 32] {
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = seed.wrapping_add(i as u8);
    }
    key
}
