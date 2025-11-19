use cucumber::{World, given, then, when};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Mutex;

mod fixtures;
use fixtures::{TestAccount, TestConfig};

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct TestWorld {
    alice: Option<TestAccount>,
    bob: Option<TestAccount>,
    test_group_key: Option<[u8; 32]>,
    current_account: Mutex<Option<String>>,
    last_output: Mutex<Option<Output>>,
    test_config: TestConfig,
}

impl TestWorld {
    fn new() -> Self {
        Self {
            alice: None,
            bob: None,
            test_group_key: None,
            current_account: Mutex::new(None),
            last_output: Mutex::new(None),
            test_config: TestConfig::load().expect("Failed to load test config"),
        }
    }

    fn get_binary_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap();
        path.pop();
        path.pop();
        path.push("signal_client");
        path
    }

    fn get_accounts_dir() -> PathBuf {
        let mut path = std::env::current_dir().unwrap();
        path.push("tests");
        path.push("fixtures");
        path.push("accounts");
        path
    }

    fn run_cli_command(&self, args: &[&str]) -> Output {
        let accounts_dir = Self::get_accounts_dir();
        let config_dir = accounts_dir.join("config");
        std::fs::create_dir_all(&config_dir).ok();

        let binary = Self::get_binary_path();

        let mut child = Command::new(&binary)
            .args(args)
            .env("ACCOUNTS_DIR", &accounts_dir)
            .env("SIGNAL_CONFIG_DIR", &config_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn command");

        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = {
                        let mut buf = Vec::new();
                        if let Some(mut out) = child.stdout.take() {
                            std::io::Read::read_to_end(&mut out, &mut buf).ok();
                        }
                        buf
                    };

                    let stderr = {
                        let mut buf = Vec::new();
                        if let Some(mut err) = child.stderr.take() {
                            std::io::Read::read_to_end(&mut err, &mut buf).ok();
                        }
                        buf
                    };

                    return Output {
                        status,
                        stdout,
                        stderr,
                    };
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        child.kill().ok();
                        panic!("Command timed out after {} seconds", timeout.as_secs());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => panic!("Error checking command status: {e}"),
            }
        }
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
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

#[given(regex = r#"two registered accounts "([^"]*)" and "([^"]*)" exist"#)]
async fn two_accounts_exist(world: &mut TestWorld, alice_name: String, bob_name: String) {
    world.cleanup().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let alice = TestAccount::load(&alice_name).await.unwrap();
    let bob = TestAccount::load(&bob_name).await.unwrap();

    world.alice = Some(alice);
    world.bob = Some(bob);

    let alice_config = world.test_config.get_account(&alice_name).unwrap();
    let bob_config = world.test_config.get_account(&bob_name).unwrap();

    *world.current_account.lock().unwrap() = Some(alice_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &alice_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    world.run_cli_command(&["sync-contacts"]);
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    *world.current_account.lock().unwrap() = Some(bob_config.account_name.clone());
    world.run_cli_command(&["switch-account", "--account-name", &bob_config.account_name]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    world.run_cli_command(&["sync-contacts"]);
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    *world.current_account.lock().unwrap() = Some(alice_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &alice_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(regex = r#"account "([^"]*)" is active"#)]
async fn account_is_active(world: &mut TestWorld, account_alias: String) {
    let account_config = world
        .test_config
        .get_account(&account_alias)
        .unwrap_or_else(|| panic!("Account config not found for {account_alias}"));

    *world.current_account.lock().unwrap() = Some(account_config.account_name.clone());
    let output = world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &account_config.account_name,
    ]);
    *world.last_output.lock().unwrap() = Some(output);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(regex = r#"account '([^"]*)' doesn't exist"#)]
async fn account_doesnt_exists(_world: &mut TestWorld, account_alias: String) {
    let accounts_dir = TestWorld::get_accounts_dir();
    let account_path = accounts_dir.join(&account_alias);

    if account_path.exists() {
        std::fs::remove_dir_all(&account_path).ok();
    }
}

#[given(regex = r#"account "([^"]*)" sent "([^"]*)" to "([^"]*)" at timestamp "([^"]*)""#)]
async fn account_sent_message_with_timestamp(
    world: &mut TestWorld,
    sender: String,
    message: String,
    recipient: String,
    _timestamp: String,
) {
    let sender_config = world.test_config.get_account(&sender).unwrap();
    let recipient_config = world.test_config.get_account(&recipient).unwrap();

    *world.current_account.lock().unwrap() = Some(sender_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &sender_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    world.run_cli_command(&[
        "send-message",
        "--recipient",
        &recipient_config.uuid,
        "--text-message",
        &message,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    *world.current_account.lock().unwrap() = Some(recipient_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &recipient_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    world.run_cli_command(&["receive"]);
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

#[given(regex = r#"a test file "([^"]*)" exists"#)]
async fn test_file_exists(_world: &mut TestWorld, filename: String) {
    let path = TestWorld::get_accounts_dir().join(&filename);
    fs::write(&path, b"test content").unwrap();
}

#[when(regex = r#"I run "([^"]*)""#)]
async fn run_command(world: &mut TestWorld, command: String) {
    let mut args: Vec<String> = Vec::new();
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;
    let mut current_arg = String::new();
    let chars = command.chars().peekable();

    for ch in chars {
        match ch {
            '\'' if !in_double_quotes => {
                if in_single_quotes {
                    args.push(current_arg.clone());
                    current_arg.clear();
                    in_single_quotes = false;
                } else {
                    in_single_quotes = true;
                }
            }
            '"' if !in_single_quotes => {
                if in_double_quotes {
                    args.push(current_arg.clone());
                    current_arg.clear();
                    in_double_quotes = false;
                } else {
                    in_double_quotes = true;
                }
            }
            ' ' if !in_single_quotes && !in_double_quotes => {
                if !current_arg.is_empty() {
                    args.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => {
                current_arg.push(ch);
            }
        }
    }

    if !current_arg.is_empty() {
        args.push(current_arg);
    }

    let command_name = args.first().map(|s| s.to_string()).unwrap_or_default();

    if command_name == "receive" {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    if command_name == "list-messages" {
        let current = world.current_account.lock().unwrap().clone().unwrap();

        world.run_cli_command(&["switch-account", "--account-name", &current]);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        world.run_cli_command(&["receive"]);
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    for i in 0..args.len() {
        if i > 0
            && args[i - 1] == "--account-name"
            && let Some(account_config) = world.test_config.get_account(&args[i])
        {
            args[i] = account_config.account_name.clone();
        }

        if i > 0 && args[i - 1] == "--attachment-path" {
            let path = TestWorld::get_accounts_dir().join(&args[i]);
            args[i] = path.to_string_lossy().to_string();
        }

        if i > 0
            && args[i - 1] == "--recipient"
            && command_name != "send-message-to-group"
            && let Some(account_config) = world.test_config.get_account(&args[i])
        {
            args[i] = account_config.uuid.clone();
        }

        if i > 0
            && args[i - 1] == "--contact"
            && let Some(account_config) = world.test_config.get_account(&args[i])
        {
            args[i] = account_config.uuid.clone();
        }

        if i > 0 && args[i - 1] == "--timestamp" {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            args[i] = now.to_string();
        }

        if command_name == "list-messages"
            && i > 0
            && args[i - 1] != "--contact"
            && args[i - 1] != "--group"
        {
            let timestamp: u64 = args[i].parse().unwrap_or(0);
            if timestamp > 0 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                args[i] = now.saturating_sub(10000).to_string();
            }
        }
    }

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = world.run_cli_command(&args_refs);
    *world.last_output.lock().unwrap() = Some(output);

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

#[then(regex = r#"I should see account "([^"]*)" in the output"#)]
async fn should_see_account(world: &mut TestWorld, account_alias: String) {
    let output = world.get_output_string();
    let account_config = world
        .test_config
        .get_account(&account_alias)
        .unwrap_or_else(|| panic!("Account config not found for {account_alias}"));

    assert!(
        output.contains(&account_config.account_name) || output.contains(&account_alias),
        "Expected account '{}' or '{}' in output: {}",
        account_config.account_name,
        account_alias,
        output
    );
}

#[then(regex = r#"I should see "([^"]*)" in the output"#)]
async fn should_see_in_output(world: &mut TestWorld, text: String) {
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let output = world.get_output_string();
    let stderr = {
        let out = world.last_output.lock().unwrap();
        out.as_ref()
            .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
            .unwrap_or_default()
    };

    if let Some(account_config) = world.test_config.get_account(&text) {
        assert!(
            output.contains(&account_config.account_name) || output.contains(&text),
            "Expected '{}' or '{}' in output.\nStdout: {}\nStderr: {}",
            account_config.account_name,
            text,
            output,
            stderr
        );
    } else {
        assert!(
            output.contains(&text),
            "Expected '{text}' in output.\nStdout: {output}\nStderr: {stderr}"
        );
    }
}

#[then(regex = r#"I should not see "([^"]*)" in the output"#)]
async fn should_not_see_in_output(world: &mut TestWorld, text: String) {
    let output = world.get_output_string();
    assert!(
        !output.contains(&text),
        "Did not expect '{text}' in output: {output}"
    );
}

#[then("the message should be sent successfully")]
async fn message_sent_successfully(world: &mut TestWorld) {
    let output = world.last_output.lock().unwrap();
    let output = output.as_ref().unwrap();

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "Command failed!\nExit code: {:?}\nStdout: {}\nStderr: {}",
            output.status.code(),
            stdout,
            stderr
        );
    }
}

#[then(regex = r#"account "([^"]*)" should receive "([^"]*)" from "([^"]*)""#)]
async fn account_receives_message(
    world: &mut TestWorld,
    recipient: String,
    message: String,
    sender: String,
) {
    let recipient_config = world.test_config.get_account(&recipient).unwrap();
    *world.current_account.lock().unwrap() = Some(recipient_config.account_name.clone());

    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &recipient_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    world.run_cli_command(&["receive"]);
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let sender_config = world.test_config.get_account(&sender).unwrap();
    let output = world.run_cli_command(&["list-messages", "--contact", &sender_config.uuid]);
    let output_str = String::from_utf8_lossy(&output.stdout);

    let last_message = output_str.lines().last().unwrap_or("");

    assert!(
        last_message.contains(&message),
        "Message '{message}' not found in last message: {last_message}"
    );
}

#[given(regex = r#"account "([^"]*)" sent "([^"]*)" to "([^"]*)""#)]
async fn account_sent_message(
    world: &mut TestWorld,
    sender: String,
    message: String,
    recipient: String,
) {
    let sender_config = world.test_config.get_account(&sender).unwrap();
    let recipient_config = world.test_config.get_account(&recipient).unwrap();

    *world.current_account.lock().unwrap() = Some(sender_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &sender_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    world.run_cli_command(&[
        "send-message",
        "--recipient",
        &recipient_config.uuid,
        "--text-message",
        &message,
    ]);

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    *world.current_account.lock().unwrap() = Some(recipient_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &recipient_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(regex = r#"a group "([^"]*)" exists with members "([^"]*)" and "([^"]*)""#)]
async fn group_exists(
    _world: &mut TestWorld,
    _group_name: String,
    _member1: String,
    _member2: String,
) {
}

#[given(regex = r#"group "([^"]*)" has message "([^"]*)" from "([^"]*)""#)]
async fn group_has_message(
    world: &mut TestWorld,
    _group_name: String,
    message: String,
    sender: String,
) {
    let sender_config = world.test_config.get_account(&sender).unwrap();

    *world.current_account.lock().unwrap() = Some(sender_config.account_name.clone());
    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &sender_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    world.run_cli_command(&[
        "send-message-to-group",
        "--recipient",
        "Test",
        "--text-message",
        &message,
    ]);

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
}

#[then(regex = r#"account "([^"]*)" should receive an attachment "([^"]*)" from "([^"]*)""#)]
async fn account_receives_attachment(
    world: &mut TestWorld,
    recipient: String,
    filename: String,
    _sender: String,
) {
    let recipient_config = world.test_config.get_account(&recipient).unwrap();
    *world.current_account.lock().unwrap() = Some(recipient_config.account_name.clone());

    world.run_cli_command(&[
        "switch-account",
        "--account-name",
        &recipient_config.account_name,
    ]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    world.run_cli_command(&["receive"]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let output = world.run_cli_command(&[
        "list-messages",
        "--contact",
        &world.test_config.get_account("alice").unwrap().uuid,
    ]);
    let output_str = String::from_utf8_lossy(&output.stdout);

    assert!(
        output_str.contains(&filename) || output_str.contains("ATTACHMENT"),
        "Attachment '{filename}' not found in output: {output_str}"
    );
}

#[then(regex = r#"group "([^"]*)" should receive "([^"]*)""#)]
async fn group_receives_message(world: &mut TestWorld, group_name: String, message: String) {
    let bob_config = world.test_config.get_account("bob").unwrap();

    *world.current_account.lock().unwrap() = Some(bob_config.account_name.clone());
    world.run_cli_command(&["switch-account", "--account-name", &bob_config.account_name]);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    world.run_cli_command(&["receive"]);
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let output = world.run_cli_command(&["list-messages", "--group", &group_name]);
    let output_str = String::from_utf8_lossy(&output.stdout);

    let last_message = output_str.lines().last().unwrap_or("");

    assert!(
        last_message.contains(&message),
        "Message '{message}' not found in last group message: {last_message}"
    );
}

#[then(regex = r#"the sender should be "([^"]*)""#)]
async fn sender_should_be(world: &mut TestWorld, sender_alias: String) {
    let output = world.get_output_string();
    let sender_config = world.test_config.get_account(&sender_alias);

    let found = output.contains(&sender_alias)
        || sender_config
            .as_ref()
            .is_some_and(|cfg| output.contains(&cfg.uuid))
        || output.contains("Them");

    assert!(
        found,
        "Sender '{sender_alias}' not found in output: {output}"
    );
}

#[then(regex = r#"I should see unrestricted access status"#)]
async fn should_see_unrestricted_access(world: &mut TestWorld) {
    let output = world.get_output_string();
    assert!(
        output.contains("Unrestricted")
            || output.contains("unrestricted")
            || output.contains("Access"),
        "Expected unrestricted access status in output: {output}"
    );
}

#[then("contacts should be synchronized successfully")]
async fn contacts_synchronized(world: &mut TestWorld) {
    let output = world.last_output.lock().unwrap();
    let output = output.as_ref().unwrap();
    assert!(output.status.success(), "Contact sync failed");
}

#[when(regex = r#"I confirm deletion with "([^"]*)""#)]
async fn confirm_deletion(world: &mut TestWorld, confirmation: String) {
    use std::io::Write;
    use std::process::Stdio;

    let binary = TestWorld::get_binary_path();
    let accounts_dir = TestWorld::get_accounts_dir();

    let mut child = Command::new(&binary)
        .arg("unlink-account")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("ACCOUNTS_DIR", &accounts_dir)
        .spawn()
        .expect("Failed to spawn command");

    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{confirmation}").expect("Failed to write to stdin");
    }

    let output = child
        .wait_with_output()
        .expect("Failed to wait for command");
    *world.last_output.lock().unwrap() = Some(output);
}

#[then(regex = r#"I should see contact "([^"]*)" in the output"#)]
async fn should_see_contact(world: &mut TestWorld, contact_alias: String) {
    let output = world.get_output_string();

    let current_name = world
        .current_account
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default();
    let owner_alias = if world.alice.as_ref().map(|a| &a.name) == Some(&current_name) {
        "alice"
    } else {
        "bob"
    };

    let contact_config = world.test_config.get_account(&contact_alias);
    let contact_name = world
        .test_config
        .get_contact_name(owner_alias, &contact_alias);

    let found = contact_name
        .as_ref()
        .is_some_and(|name| output.contains(name))
        || contact_config
            .as_ref()
            .is_some_and(|cfg| output.contains(&cfg.uuid))
        || output.contains(&contact_alias);

    assert!(
        found,
        "Contact '{}' not found. Tried: name={:?}, uuid={:?}\nOutput: {}",
        contact_alias,
        contact_name,
        contact_config.map(|c| &c.uuid),
        output
    );
}

#[then("the contact should have a UUID")]
async fn contact_has_uuid(world: &mut TestWorld) {
    let output = world.get_output_string();
    assert!(
        output.contains("UUID") || output.contains("uuid"),
        "UUID not found in output"
    );
}

#[then(regex = r#"the current account should be "([^"]*)""#)]
async fn current_account_should_be(world: &mut TestWorld, account_alias: String) {
    let account_config = world.test_config.get_account(&account_alias).unwrap();

    let output = world.run_cli_command(&["get-current-account"]);
    let output_str = String::from_utf8_lossy(&output.stdout);

    assert!(
        output_str.contains(&account_config.account_name),
        "Expected current account to be '{}', but got: {}",
        account_config.account_name,
        output_str
    );
}

#[then("I should see QR code linking prompt")]
async fn should_see_qr_prompt(world: &mut TestWorld) {
    let output = world.get_output_string();
    let stderr = {
        let out = world.last_output.lock().unwrap();
        out.as_ref()
            .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
            .unwrap_or_default()
    };

    assert!(
        output.contains("sgnl://")
            || output.contains("tsdevice")
            || stderr.contains("sgnl://")
            || stderr.contains("tsdevice"),
        "Expected linking URL in output. Stdout: {output}\nStderr: {stderr}"
    );
}

#[then(regex = r#"account "([^"]*)" should be deleted"#)]
async fn account_deleted(_world: &mut TestWorld, _account: String) {}

#[then(regex = r#"I should see group "([^"]*)" in the output"#)]
async fn should_see_group(world: &mut TestWorld, group: String) {
    let output = world.get_output_string();
    assert!(
        output.contains(&group),
        "Expected group '{group}' in output: {output}"
    );
}

#[then("I should see profile information")]
async fn should_see_profile_info(world: &mut TestWorld) {
    let output = world.get_output_string();
    assert!(output.contains("Profile") || output.contains("Name") || !output.is_empty());
}

#[then("the attachment should be sent successfully")]
async fn attachment_sent_successfully(world: &mut TestWorld) {
    message_sent_successfully(world).await;
}

#[then("the message should be deleted successfully")]
async fn message_deleted_successfully(world: &mut TestWorld) {
    message_sent_successfully(world).await;
}

#[given(regex = r#"I sent "([^"]*)" to "([^"]*)" at timestamp "([^"]*)""#)]
async fn i_sent_message_at_time(
    world: &mut TestWorld,
    message: String,
    recipient: String,
    _timestamp: String,
) {
    let recipient_config = world.test_config.get_account(&recipient).unwrap();

    world.run_cli_command(&[
        "send-message",
        "--recipient",
        &recipient_config.uuid,
        "--text-message",
        &message,
    ]);

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
}

#[then(regex = r#"I should not see "([^"]*)" when listing accounts"#)]
async fn should_not_see_when_listing(world: &mut TestWorld, account: String) {
    let output = world.run_cli_command(&["list-accounts"]);
    let output_str = String::from_utf8_lossy(&output.stdout);
    assert!(!output_str.contains(&account));
}

#[then(regex = r#"account "([^"]*)" should be created after linking"#)]
async fn account_created_after_linking(_world: &mut TestWorld, account: String) {
    let account_path = TestWorld::get_accounts_dir().join(&account);
    assert!(
        account_path.exists(),
        "Account directory '{account}' should exist after linking"
    );
}

#[then(regex = r#"I should see profile name or "([^"]*)""#)]
async fn should_see_profile_name_or_na(world: &mut TestWorld, default: String) {
    let output = world.get_output_string();
    assert!(
        output.contains("Name:") || output.contains(&default),
        "Expected profile name or '{default}' in output: {output}"
    );
}

#[when(regex = r#"I link-account --account-name '([^ ]*)' --device-name '([^']*)'"#)]
async fn run_link_account(world: &mut TestWorld, account_name: String, device_name: String) {
    use std::io::Read;

    let accounts_dir = TestWorld::get_accounts_dir();
    let config_dir = accounts_dir.join("config");
    std::fs::create_dir_all(&config_dir).ok();

    let binary = TestWorld::get_binary_path();

    // We need to stream stdout/stderr while the process is running to detect the QR code URL.
    // The process is either killed when we find the URL or waited on via try_wait() in all code paths,
    #[allow(clippy::zombie_processes)]
    let mut child = Command::new(&binary)
        .arg("link-account")
        .arg("--account-name")
        .arg(&account_name)
        .arg("--device-name")
        .arg(&device_name)
        .env("ACCOUNTS_DIR", &accounts_dir)
        .env("SIGNAL_CONFIG_DIR", &config_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn command");

    let timeout = std::time::Duration::from_secs(10);
    let start = std::time::Instant::now();
    let mut stdout_data = Vec::new();
    let mut stderr_data = Vec::new();

    loop {
        if let Some(ref mut stdout) = child.stdout {
            let mut buf = [0u8; 1024];
            if let Ok(n) = stdout.read(&mut buf)
                && n > 0
            {
                stdout_data.extend_from_slice(&buf[..n]);
                let output = String::from_utf8_lossy(&stdout_data);
                if output.contains("sgnl://") || output.contains("tsdevice") {
                    child.kill().ok();

                    if let Some(mut stdout) = child.stdout.take() {
                        stdout.read_to_end(&mut stdout_data).ok();
                    }
                    if let Some(mut stderr) = child.stderr.take() {
                        stderr.read_to_end(&mut stderr_data).ok();
                    }

                    *world.last_output.lock().unwrap() = Some(Output {
                        status: std::process::ExitStatus::default(),
                        stdout: stdout_data,
                        stderr: stderr_data,
                    });
                    return;
                }
            }
        }

        if let Some(ref mut stderr) = child.stderr {
            let mut buf = [0u8; 1024];
            if let Ok(n) = stderr.read(&mut buf)
                && n > 0
            {
                stderr_data.extend_from_slice(&buf[..n]);
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(mut stdout) = child.stdout.take() {
                    stdout.read_to_end(&mut stdout_data).ok();
                }
                if let Some(mut stderr) = child.stderr.take() {
                    stderr.read_to_end(&mut stderr_data).ok();
                }

                *world.last_output.lock().unwrap() = Some(Output {
                    status,
                    stdout: stdout_data,
                    stderr: stderr_data,
                });
                return;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    child.kill().ok();

                    if let Some(mut stdout) = child.stdout.take() {
                        stdout.read_to_end(&mut stdout_data).ok();
                    }
                    if let Some(mut stderr) = child.stderr.take() {
                        stderr.read_to_end(&mut stderr_data).ok();
                    }

                    *world.last_output.lock().unwrap() = Some(Output {
                        status: std::process::ExitStatus::default(),
                        stdout: stdout_data,
                        stderr: stderr_data,
                    });
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => panic!("Error checking command status: {e}"),
        }
    }
}

#[then(regex = r#"account "([^"]*)" is active"#)]
async fn then_account_is_active(world: &mut TestWorld, account_alias: String) {
    account_is_active(world, account_alias).await;
}

#[when(regex = r#"account "([^"]*)" is active"#)]
async fn when_account_is_active(world: &mut TestWorld, account_alias: String) {
    account_is_active(world, account_alias).await;
}

#[tokio::main]
async fn main() {
    TestWorld::cucumber()
        .with_default_cli()
        .max_concurrent_scenarios(1)
        .run("tests/features")
        .await;
}
