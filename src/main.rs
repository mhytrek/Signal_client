use anyhow::Result;
use clap::Parser;

use signal_client::account_management::{create_account_cli, delete_account_cli, get_current_account_cli, list_accounts_cli, switch_account_cli};
use signal_client::args::{Cli, Command};
use signal_client::logger::init_logger;
use signal_client::{cli, contacts, tui};
use signal_client::{devices, messages};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    init_logger();
    let cli = Cli::parse();

    match cli.command {
        Command::SyncContacts => contacts::sync_contacts_cli().await?,
        Command::LinkDevice(args) => devices::link_new_device_cli(args.device_name).await?,
        Command::ListContacts => cli::print_contacts().await?,
        Command::ListGroups => cli::print_groups().await?,
        Command::RunApp => tui::run_tui().await?,
        Command::SendMessage(args) => {
            messages::send::contact::send_message_cli(args.recipient, args.text_message).await?
        }
        Command::SendToGroup(args) => {
            messages::send::group::send_message_cli(args.group, args.text_message).await?
        }
        Command::ListMessages(args) => match (args.contact, args.group) {
            (Some(c), None) => cli::print_messages_from_contact(c, args.from).await?,
            (None, Some(g)) => cli::print_messages_from_group(g, args.from).await?,
            _ => unreachable!(),
        },
        Command::Receive => cli::print_received_message().await?,
        Command::GetProfile => cli::print_profile().await?,
        Command::SendAttachment(args) => {
            messages::send::send_attachment_cli(
                args.recipient,
                args.text_message,
                args.attachment_path,
            )
            .await?
        }
        Command::CreateAccount(args) => {
            create_account_cli(args.account_name, args.device_name).await?;
            contacts::sync_contacts_cli().await?;
        }
        Command::ListAccounts => list_accounts_cli().await?,
        Command::SwitchAccount(args) => switch_account_cli(args.account_name).await?,
        Command::GetCurrentAccount => get_current_account_cli().await?,
        Command::DeleteAccount(args) => {
            delete_account_cli(args.account_name).await?
        }
    }

    Ok(())
}
