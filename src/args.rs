use clap::{ArgGroup, Args, Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
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

    /// Send attachment
    SendAttachment(SendAttachmentArgs),

    /// Delete message
    DeleteMessage(DeleteMessageArgs),

    /// Prints messages from given point in time
    ListMessages(ListMessagesArgs),

    /// Prints the messages received from the last synchronization
    Receive,

    /// Prints profile info
    GetProfile,

    /// Create a new account
    LinkAccount(CreateAccountArgs),

    /// List all accounts
    ListAccounts,

    /// Switch to an account
    SwitchAccount(SwitchAccountArgs),

    /// Get current active account
    GetCurrentAccount,

    /// Delete an account
    UnlinkAccount(DeleteAccountArgs),
}

#[derive(Args)]
pub struct DeleteAccountArgs {
    /// Name of the account to delete
    #[arg(short, long)]
    pub account_name: String,
}

#[derive(Args)]
pub struct CreateAccountArgs {
    /// Name of the new account
    #[arg(short, long)]
    pub account_name: String,

    /// Device name for Signal registration
    #[arg(short, long)]
    pub device_name: String,
}

#[derive(Args)]
pub struct SwitchAccountArgs {
    /// Name of the account to switch to
    #[arg(short, long)]
    pub account_name: String,
}

#[derive(Args)]
pub struct SendMessageArgs {
    /// Name, phone number or UUID of the contact that the message should be send to
    #[arg(short, long)]
    pub recipient: String,

    /// Content of the message
    pub text_message: String,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("recipient")
        .required(true)
))]
pub struct ListMessagesArgs {
    /// Uuid of the contact that the message history should be shown
    #[arg(short, long, group = "recipient")]
    pub contact: Option<String>,

    /// Name of the group that the message history should be shown
    #[arg(short, long, group = "recipient")]
    pub group: Option<String>,

    /// The timestamp from which messages start being displayed
    pub from: Option<String>,
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

#[derive(Args)]
#[command(group(
    ArgGroup::new("recipient")
        .required(true)
))]
pub struct DeleteMessageArgs {
    /// Uuid of the contact conversation from which the message would be deleted
    #[arg(short, long, group = "recipient")]
    pub contact: Option<String>,

    /// Name of the group from which the message would be deleted
    #[arg(short, long, group = "recipient")]
    pub group: Option<String>,

    /// timestamp of the message that would be deleted
    #[arg(short, long)]
    pub timestamp: u64,
}
