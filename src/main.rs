use anyhow::Result;
use clap::Parser;

use log::debug;
use signal_client::args::{Cli, Command};
use signal_client::messages;
use signal_client::{cli, contacts, devices, tui};

#[tokio::main]
async fn main() -> Result<()> {
    // env_logger::init();
    debug!("Starting app!");
    let cli = Cli::parse();

    match cli.command {
        Command::SyncContacts => contacts::sync_contacts_cli().await?,
        Command::LinkDevice(args) => devices::link_new_device_cli(args.device_name).await?,
        Command::ListContacts => cli::print_contacts().await?,
        Command::RunApp => tui::run_tui().await?,
        Command::SendMessage(args) => {
            messages::send::contacts::send_message_to_contact_cli(args.recipient, args.text_message)
                .await?
        }
        Command::ListMessages(args) => cli::print_messages(args.recipient, args.from).await?,
        Command::Receive => cli::print_received_message().await?,
        Command::GetProfile => cli::print_profile().await?,
        Command::SendAttachment(args) => {
            messages::send::contacts::send_attachment_to_contact_cli(
                args.recipient,
                args.text_message,
                args.attachment_path,
            )
            .await?
        }
        Command::ListGroups => cli::print_groups().await?,
        Command::SendMessageToGroup(args) => {
            messages::send::groups::send_message_to_group_cli(args.group_name, args.text_message)
                .await?
        }
    };

    Ok(())
}
