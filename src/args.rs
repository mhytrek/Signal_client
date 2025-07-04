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

    /// Prints locally stored groups
    ListGroups,

    /// Displays a prototype of a layout with example data
    RunApp,

    /// Send text message
    SendMessage(SendMessageArgs),

    /// Send text message to group
    SendMessageToGroup(SendMessageToGroupArgs),

    /// Send attachment
    SendAttachment(SendAttachmentArgs),

    /// Prints messages from given point in time
    ListMessages(ListMessagesArgs),

    /// Prints the messages received from the last synchronization
    Receive,

    /// Prints profile info
    GetProfile,
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
pub struct SendMessageToGroupArgs {
    /// Name of the group to send message to
    pub group_name: String,

    /// Content of the message
    pub text_message: String,
}

#[derive(Args)]
pub struct ListMessagesArgs {
    /// Uuid of the contact that the message history should be shown
    #[arg(short, long)]
    pub recipient: String,
    /// The timestamp from which messages start being displayed
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
