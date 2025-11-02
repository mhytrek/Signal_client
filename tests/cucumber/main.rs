use cucumber::{World, given, then, when};
use std::process::{Command, Output};
use tempfile::TempDir;
use std::fs;
use std::sync::Mutex;

mod fixtures;
use fixtures::{TestAccount, copy_account_to_temp};
use fixtures::builder::MessageBuilder;
use fixtures::setup;

use presage::libsignal_service::prelude::ProfileKey;
use presage::model::contacts::Contact;
use presage::store::ContentsStore;
use presage_store_sqlite::SqliteStore;
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct TestWorld {
    accounts_dir: TempDir,
    alice: Option<TestAccount>,
    bob: Option<TestAccount>,
    test_group_key: Option<[u8; 32]>,
    current_account: Mutex<Option<String>>,
    last_output: Mutex<Option<Output>>,
}

impl TestWorld {
    fn new() -> Self {
        Self {
            accounts_dir: TempDir::new().unwrap(),
            alice: None,
            bob: None,
            test_group_key: None,
            current_account: Mutex::new(None),
            last_output: Mutex::new(None),
        }
    }

    fn run_cli_command(&self, args: &[&str]) -> Output {
        Command::new("cargo")
            .arg("run")
            .arg("--")
            .args(args)
            .env("ACCOUNTS_DIR", self.accounts_dir.path())
            .output()
            .expect("Failed to execute command")
    }

    fn get_output_string(&self) -> String {
        let output = self.last_output.lock().unwrap();
        let output = output.as_ref().expect("No command output");
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    async fn cleanup(&mut self) {
        self.alice = None;
        self.bob = None;
        self.test_group_key = None;
    }
}

#[given(regex = r#"two registered accounts "([^"]*)" and "([^"]*)" exist"#)]
async fn two_accounts_exist(world: &mut TestWorld, alice_name: String, bob_name: String) {
    world.cleanup().await;

    copy_account_to_temp(&alice_name, world.accounts_dir.path()).unwrap();
    copy_account_to_temp(&bob_name, world.accounts_dir.path()).unwrap();

    let alice = TestAccount::load(&alice_name).await.unwrap();
    let bob = TestAccount::load(&bob_name).await.unwrap();

    {
        let alice_store = alice.get_store().await.unwrap();
        let bob_store = bob.get_store().await.unwrap();

        let contact_alice = Contact {
            uuid: alice.uuid,
            name: "alice".to_string(),
            phone_number: None,
            color: None,
            verified: Default::default(),
            profile_key: ProfileKey::generate([0u8; 32]).get_bytes().to_vec(),
            expire_timer: 0,
            expire_timer_version: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        };

        let contact_bob = Contact {
            uuid: bob.uuid,
            name: "bob".to_string(),
            phone_number: None,
            color: None,
            verified: Default::default(),
            profile_key: ProfileKey::generate([0u8; 32]).get_bytes().to_vec(),
            expire_timer: 0,
            expire_timer_version: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        };

        unsafe {
            let bob_store_ptr = &bob_store as *const SqliteStore as *mut SqliteStore;
            (*bob_store_ptr).save_contact(&contact_alice).await.unwrap();

            let alice_store_ptr = &alice_store as *const SqliteStore as *mut SqliteStore;
            (*alice_store_ptr).save_contact(&contact_bob).await.unwrap();
        }

        drop(alice_store);
        drop(bob_store);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    world.alice = Some(alice);
    world.bob = Some(bob);
}

#[given(regex = r#"account "([^"]*)" is active"#)]
async fn account_is_active(world: &mut TestWorld, account: String) {
    *world.current_account.lock().unwrap() = Some(account.clone());
    let output = world.run_cli_command(&["switch-account", "--account-name", &account]);
    *world.last_output.lock().unwrap() = Some(output);
}

#[given(regex = r#"a test file "([^"]*)" exists"#)]
async fn test_file_exists(world: &mut TestWorld, filename: String) {
    let path = world.accounts_dir.path().join(&filename);
    fs::write(&path, b"test content").unwrap();
}

#[given(regex = r#"account "([^"]*)" sent "([^"]*)" to "([^"]*)" at timestamp "([^"]*)""#)]
async fn account_sent_message_at_time(
    world: &mut TestWorld,
    sender: String,
    message: String,
    _recipient: String,
    timestamp: String,
) {
    let timestamp = timestamp.parse::<u64>().unwrap();

    let (sender_account, recipient_account) = if sender == "alice" {
        (world.alice.as_ref().unwrap(), world.bob.as_ref().unwrap())
    } else {
        (world.bob.as_ref().unwrap(), world.alice.as_ref().unwrap())
    };

    let store = recipient_account.get_store().await.unwrap();
    let mut builder = MessageBuilder::new(store, sender_account.uuid);

    builder
        .add_received_message(sender_account.uuid, &message, Some(timestamp))
        .await
        .unwrap();
}

#[given(regex = r#"account "([^"]*)" sent "([^"]*)" to "([^"]*)""#)]
async fn account_sent_message(
    world: &mut TestWorld,
    sender: String,
    message: String,
    _recipient: String,
) {
    let (sender_account, recipient_account) = if sender == "alice" {
        (world.alice.as_ref().unwrap(), world.bob.as_ref().unwrap())
    } else {
        (world.bob.as_ref().unwrap(), world.alice.as_ref().unwrap())
    };

    let store = recipient_account.get_store().await.unwrap();
    let mut builder = MessageBuilder::new(store, sender_account.uuid);

    builder
        .add_received_message(sender_account.uuid, &message, None)
        .await
        .unwrap();
}

#[given(regex = r#"I sent "([^"]*)" to "([^"]*)" at timestamp "([^"]*)""#)]
async fn i_sent_message_at_time(
    world: &mut TestWorld,
    message: String,
    _recipient: String,
    timestamp: String,
) {
    let timestamp = timestamp.parse::<u64>().unwrap();
    let current = world.current_account.lock().unwrap().clone().unwrap();

    let (sender_account, recipient_account) = if current == "alice" {
        (world.alice.as_ref().unwrap(), world.bob.as_ref().unwrap())
    } else {
        (world.bob.as_ref().unwrap(), world.alice.as_ref().unwrap())
    };

    let store = sender_account.get_store().await.unwrap();
    let mut builder = MessageBuilder::new(store, recipient_account.uuid);

    builder
        .add_sent_message(&message, Some(timestamp))
        .await
        .unwrap();
}

#[when(regex = r#"I run "([^"]*)""#)]
async fn run_command(world: &mut TestWorld, command: String) {
    let args: Vec<&str> = command.split_whitespace().collect();
    let output = world.run_cli_command(&args);
    *world.last_output.lock().unwrap() = Some(output);
}

#[then(regex = r#"I should see "([^"]*)" in the output"#)]
async fn should_see_in_output(world: &mut TestWorld, text: String) {
    let output = world.get_output_string();
    assert!(
        output.contains(&text),
        "Expected '{}' in output: {}",
        text,
        output
    );
}

#[then(regex = r#"I should not see "([^"]*)" in the output"#)]
async fn should_not_see_in_output(world: &mut TestWorld, text: String) {
    let output = world.get_output_string();
    assert!(
        !output.contains(&text),
        "Did not expect '{}' in output: {}",
        text,
        output
    );
}

#[then("the message should be sent successfully")]
async fn message_sent_successfully(world: &mut TestWorld) {
    let output = world.last_output.lock().unwrap();
    let output = output.as_ref().unwrap();
    assert!(output.status.success());
}

#[given(regex = r#"a group "([^"]*)" exists with members "([^"]*)" and "([^"]*)""#)]
async fn group_exists(
    world: &mut TestWorld,
    group_name: String,
    _member1: String,
    _member2: String,
) {
    let alice = world.alice.as_ref().unwrap();
    let bob = world.bob.as_ref().unwrap();

    let master_key = setup::generate_test_master_key(42);
    world.test_group_key = Some(master_key);
}

#[given(regex = r#"group "([^"]*)" has message "([^"]*)" from "([^"]*)""#)]
async fn group_has_message(
    world: &mut TestWorld,
    _group_name: String,
    message: String,
    sender: String,
) {
    let master_key = world.test_group_key.as_ref().unwrap();

    let (sender_account, recipient_account) = if sender == "alice" {
        (world.alice.as_ref().unwrap(), world.bob.as_ref().unwrap())
    } else {
        (world.bob.as_ref().unwrap(), world.alice.as_ref().unwrap())
    };

    let store = recipient_account.get_store().await.unwrap();
    let mut builder = MessageBuilder::new_group(store, *master_key);

    builder
        .add_group_message(sender_account.uuid, &message, master_key, None)
        .await
        .unwrap();
}

#[then(regex = r#"account "([^"]*)" should receive "([^"]*)" from "([^"]*)""#)]
async fn account_receives_message(
    world: &mut TestWorld,
    recipient: String,
    message: String,
    sender: String,
) {
    let recipient_account = if recipient == "alice" {
        world.alice.as_ref().unwrap()
    } else {
        world.bob.as_ref().unwrap()
    };

    let store = recipient_account.get_store().await.unwrap();

    let sender_uuid = if sender == "alice" {
        world.alice.as_ref().unwrap().uuid
    } else {
        world.bob.as_ref().unwrap().uuid
    };

    use presage::store::{ContentsStore, Thread};
    let thread = Thread::Contact(sender_uuid);

    let messages: Vec<_> = store.messages(&thread, 0..).await.unwrap().collect();

    let found = messages.iter().any(|msg_result| {
        if let Ok(msg) = msg_result {
            if let presage::libsignal_service::content::ContentBody::DataMessage(data_msg) =
                &msg.body
            {
                data_msg
                    .body
                    .as_ref()
                    .map_or(false, |body| body.contains(&message))
            } else {
                false
            }
        } else {
            false
        }
    });

    assert!(
        found,
        "Message '{}' not found in recipient's messages",
        message
    );
}

#[then(regex = r#"account "([^"]*)" should receive an attachment "([^"]*)" from "([^"]*)""#)]
async fn account_receives_attachment(
    world: &mut TestWorld,
    recipient: String,
    filename: String,
    sender: String,
) {
    let recipient_account = if recipient == "alice" {
        world.alice.as_ref().unwrap()
    } else {
        world.bob.as_ref().unwrap()
    };

    let store = recipient_account.get_store().await.unwrap();

    let sender_uuid = if sender == "alice" {
        world.alice.as_ref().unwrap().uuid
    } else {
        world.bob.as_ref().unwrap().uuid
    };

    use presage::store::{ContentsStore, Thread};
    let thread = Thread::Contact(sender_uuid);

    let messages: Vec<_> = store.messages(&thread, 0..).await.unwrap().collect();

    let found = messages.iter().any(|msg_result| {
        if let Ok(msg) = msg_result {
            if let presage::libsignal_service::content::ContentBody::DataMessage(data_msg) =
                &msg.body
            {
                data_msg.attachments.iter().any(|att| {
                    att.file_name
                        .as_ref()
                        .map_or(false, |name| name == &filename)
                })
            } else {
                false
            }
        } else {
            false
        }
    });

    assert!(
        found,
        "Attachment '{}' not found in recipient's messages",
        filename
    );
}

#[then(regex = r#"group "([^"]*)" should receive "([^"]*)""#)]
async fn group_receives_message(world: &mut TestWorld, _group_name: String, message: String) {
    let master_key = world.test_group_key.as_ref().unwrap();
    let alice = world.alice.as_ref().unwrap();
    let store = alice.get_store().await.unwrap();

    use presage::store::{ContentsStore, Thread};
    let thread = Thread::Group(*master_key);

    let messages: Vec<_> = store.messages(&thread, 0..).await.unwrap().collect();

    let found = messages.iter().any(|msg_result| {
        if let Ok(msg) = msg_result {
            if let presage::libsignal_service::content::ContentBody::DataMessage(data_msg) =
                &msg.body
            {
                data_msg
                    .body
                    .as_ref()
                    .map_or(false, |body| body.contains(&message))
            } else {
                false
            }
        } else {
            false
        }
    });

    assert!(found, "Message '{}' not found in group messages", message);
}

#[then(regex = r#"the sender should be "([^"]*)""#)]
async fn sender_should_be(world: &mut TestWorld, sender: String) {
    let output = world.get_output_string();
    assert!(
        output.contains(&sender),
        "Sender '{}' not found in output",
        sender
    );
}

#[then("contacts should be synchronized successfully")]
async fn contacts_synchronized(world: &mut TestWorld) {
    let output = world.last_output.lock().unwrap();
    let output = output.as_ref().unwrap();
    assert!(output.status.success(), "Contact sync failed");
}

#[then(regex = r#"I should see contact "([^"]*)" in the output"#)]
async fn should_see_contact(world: &mut TestWorld, contact: String) {
    let output = world.get_output_string();
    assert!(output.contains(&contact), "Contact '{}' not found", contact);
}

#[then("the contact should have a UUID")]
async fn contact_has_uuid(world: &mut TestWorld) {
    let output = world.get_output_string();
    assert!(
        output.contains("UUID") || output.contains("uuid"),
        "UUID not found in output"
    );
}

#[tokio::main]
async fn main() {
    TestWorld::cucumber()
        .with_default_cli()
        .run("tests/features")
        .await;
}
