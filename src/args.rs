use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Link this device to your signal account
    LinkDevice(LinkDeviceArgs),
    /// Synchronizes contacts with primary device
    SyncContacts,
    /// Prints locally stored contacts
    ListContacts,
    /// Displays a prototype of a layout with example data
    RunApp,
    /// Send text message
    SendMessage(SendMessageArgs),
    /// Send attachment
    SendAttachment(SendAttachmentArgs),
    /// Prints messages from given point in time
    ListMessages(ListMessagesArgs),
    /// Prints received messeges
    Receive,
}

#[derive(Args)]
pub struct LinkDeviceArgs {
    /// Name of under which linked device should be saved
    #[arg(short, long)]
    pub device_name: String,
}

#[derive(Args)]
pub struct SendMessageArgs {
    /// Uuid of the contact that the message should be send to
    #[arg(short, long)]
    pub recipient: String,
    /// Content of the message
    pub text_message: String,
}

#[derive(Args)]
pub struct ListMessagesArgs {
    #[arg(short, long)]
    pub recipient: String,
    pub from: String,
}

#[derive(Args)]
pub struct SendAttachmentArgs {
    /// Uuid of the contact that the message should be send to
    #[arg(short, long)]
    pub recipient: String,
    /// Content of the message
    #[arg(short, long, default_value_t = String::from(""))]
    pub text_message: String,
    /// Full path to attachment
    pub attachment_path: String,
}
