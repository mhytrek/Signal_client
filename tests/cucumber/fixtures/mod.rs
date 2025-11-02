use anyhow::Result;
use presage::libsignal_service::prelude::Uuid;
use presage::model::contacts::Contact;
use presage::store::{ContentsStore, StateStore};
use presage_store_sqlite::SqliteStore;
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURES_DIR: &str = "tests/fixtures/accounts";

#[derive(Debug)]
pub struct TestAccount {
    pub name: String,
    pub path: PathBuf,
    pub uuid: Uuid,
}

impl TestAccount {
    pub async fn load(name: &str) -> Result<Self> {
        let path = PathBuf::from(FIXTURES_DIR).join(name);
        let store = open_test_store(&path).await?;

        let uuid = get_account_uuid(&store).await?;

        Ok(Self {
            name: name.to_string(),
            path,
            uuid,
        })
    }

    pub fn store_path(&self) -> PathBuf {
        self.path.join("store.db")
    }

    pub async fn get_store(&self) -> Result<SqliteStore> {
        open_test_store(&self.path).await
    }

    pub async fn add_contact(
        &self,
        contact_name: &str,
        contact_uuid: Uuid,
        phone: Option<String>,
    ) -> Result<()> {
        let store = self.get_store().await?;

        let contact = Contact {
            uuid: contact_uuid,
            name: contact_name.to_string(),
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
            let store_ptr = &store as *const SqliteStore as *mut SqliteStore;
            (*store_ptr).save_contact(&contact).await?;
        }

        Ok(())
    }

    pub async fn close_store(&self) -> Result<()> {
        Ok(())
    }
}

async fn open_test_store(account_path: &Path) -> Result<SqliteStore> {
    use presage::model::identity::OnNewIdentity;
    use presage_store_sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    let store_path = account_path.join("store.db");
    let path_str = format!("sqlite://{}", store_path.display());

    let options = SqliteConnectOptions::from_str(&path_str)?;
    SqliteStore::open_with_options(options, OnNewIdentity::Trust)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to open store: {}", e))
}

async fn get_account_uuid(store: &SqliteStore) -> Result<Uuid> {
    let registration = store
        .load_registration_data()
        .await?
        .ok_or_else(|| anyhow::anyhow!("No registration data found"))?;
    Ok(registration.service_ids.aci)
}

pub fn copy_account_to_temp(account_name: &str, temp_dir: &Path) -> Result<PathBuf> {
    let source = PathBuf::from(FIXTURES_DIR).join(account_name);
    let dest = temp_dir.join(account_name);

    fs::create_dir_all(&dest)?;
    copy_dir_recursive(&source, &dest)?;

    Ok(dest)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

pub async fn setup_test_contacts(alice: &TestAccount, bob: &TestAccount) -> Result<()> {
    alice
        .add_contact("bob", bob.uuid, Some("+1234567890".to_string()))
        .await?;
    bob.add_contact("alice", alice.uuid, Some("+0987654321".to_string()))
        .await?;
    Ok(())
}

pub mod builder;
pub mod setup;

pub async fn setup_test_group(
    alice: &TestAccount,
    bob: &TestAccount,
    group_name: &str,
) -> Result<[u8; 32]> {
    let master_key = setup::generate_test_master_key(42);
    let members = vec![alice.uuid, bob.uuid];

    let alice_store = alice.get_store().await?;
    setup::inject_group(&alice_store, group_name, master_key, members.clone()).await?;

    let bob_store = bob.get_store().await?;
    setup::inject_group(&bob_store, group_name, master_key, members).await?;

    Ok(master_key)
}
