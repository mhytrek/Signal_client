use crate::contacts::get_contacts_tui;
use crate::messages::attachments::save_attachment;
use crate::messages::receive::{self, MessageDto, check_contacts, contact, format_message};
use crate::messages::send::{self};
use crate::paths::{ACCOUNTS_DIR, QRCODE};
use crate::profile::get_profile_tui;
use crate::ui::render_ui;
use crate::{AsyncContactsMap, config::Config, contacts, groups};
use anyhow::{Error, Result, anyhow, bail};
use arboard::Clipboard;
use crossterm::event::{self, Event, KeyModifiers};
use crossterm::event::{KeyCode, KeyEventKind};
use futures::{StreamExt, pin_mut};
use presage::Manager;
use presage::libsignal_service::Profile;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::zkgroup::GroupMasterKeyBytes;
use presage::manager::Registered;
use presage::model::messages::Received;
use presage::proto::AttachmentPointer;
use presage_store_sqlite::SqliteStore;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use regex::Regex;
use std::collections::HashMap;
use std::io::Stderr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{fs, io};
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio_util::task::LocalPoolHandle;
use tracing::{Level, debug, error, info, span, trace, warn};

use crate::account_management::{
    create_registered_manager, create_registered_manager_for_account, ensure_accounts_dir,
    list_accounts,
};
use crate::devices::link_new_device_for_account;
use crate::notifications::send_notification;
use crate::retry_manager::{OutgoingMessage, RetryManager};
use image::ImageFormat;
use presage::store::ContentsStore;
use std::thread;
use std::time::Duration;
use tokio::time::interval;

#[derive(PartialEq, Clone)]
pub enum RecipientId {
    Contact(Uuid),
    Group(GroupMasterKeyBytes),
}

impl Default for RecipientId {
    fn default() -> Self {
        Self::Contact(Uuid::nil())
    }
}

pub trait DisplayRecipient: Send {
    fn display_name(&self) -> &str;
    fn id(&self) -> RecipientId;
}

#[derive(Clone, PartialEq)]
pub struct DisplayContact {
    display_name: String,
    uuid: Uuid,
}

impl DisplayContact {
    fn new(display_name: String, uuid: Uuid) -> Self {
        Self { display_name, uuid }
    }
}

impl DisplayRecipient for DisplayContact {
    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn id(&self) -> RecipientId {
        RecipientId::Contact(self.uuid)
    }
}

#[derive(Clone, PartialEq)]
pub struct DisplayGroup {
    display_name: String,
    master_key: GroupMasterKeyBytes,
}
#[derive(Clone)]
pub struct UiStatusInfo {
    pub status_message: UiStatusMessage,
    last_screen: CurrentScreen,
}

impl DisplayGroup {
    fn new(display_name: String, master_key: GroupMasterKeyBytes) -> Self {
        Self {
            display_name,
            master_key,
        }
    }
}

impl DisplayRecipient for DisplayGroup {
    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn id(&self) -> RecipientId {
        RecipientId::Group(self.master_key)
    }
}

#[derive(Clone, PartialEq)]
pub enum CurrentScreen {
    Main,
    Syncing,
    LinkingNewDevice,
    InspectMesseges,
    Popup,
    Writing,
    Options,
    Exiting,
    ContactInfo,
    AccountSelector,
    CreatingAccount,
    ConfirmDelete,
    Recaptcha,
}

#[derive(PartialEq)]
pub enum LinkingStatus {
    Unlinked,
    InProgress,
    Linked,
    Error(String),
}

pub enum InputFocus {
    Message,
    Attachment,
}

#[derive(Clone)]
pub struct ContactInfo {
    pub uuid: String,
    pub name: String,
    pub phone_number: Option<String>,
    pub verified_state: Option<i32>,
    pub expire_timer: u32,
    pub has_avatar: bool,
}

#[derive(PartialEq, Clone)]
pub enum AccountLinkingField {
    AccountName,
    DeviceName,
}

pub struct App {
    pub uuid: Option<Uuid>,
    pub recipients: Vec<(Box<dyn DisplayRecipient>, String)>, // contact_uuid, contact_name, input for this contact

    pub current_account: Option<String>,
    pub deleting_account: Option<String>,
    pub available_accounts: Vec<String>,
    pub account_selected: usize,
    pub device_name_input: String,
    pub account_linking_field: AccountLinkingField,

    pub selected_recipient: usize,
    pub message_selected: usize,

    // New fields for contact info
    pub selected_contact_info: Option<ContactInfo>,
    pub contact_avatar_cache: Option<Vec<u8>>,
    pub contact_avatar_image: Option<StatefulProtocol>,

    pub current_screen: CurrentScreen,
    pub linking_status: LinkingStatus,
    pub network_status: NetworkStatus,

    pub character_index: usize,
    pub textarea: String,
    pub attachment_path: String,
    pub attachment_error: Option<String>,

    pub quoted_message: Option<MessageDto>,

    pub retry_manager: Arc<Mutex<RetryManager>>,
    pub message_id_map: HashMap<String, String>,

    pub input_focus: InputFocus,

    pub profile: Option<Profile>,

    pub ui_status_info: Option<UiStatusInfo>,

    pub avatar_cache: Option<Vec<u8>>,
    pub picker: Option<Picker>,
    pub avatar_image: Option<StatefulProtocol>,

    pub contact_messages: HashMap<String, Vec<MessageDto>>,
    pub group_messages: HashMap<GroupMasterKeyBytes, Vec<MessageDto>>,

    pub config: Config,
    pub config_selected: usize,

    pub manager: Option<Manager<SqliteStore, Registered>>,

    pub tx_thread: mpsc::Sender<EventApp>,
    pub rx_tui: mpsc::Receiver<EventApp>,

    pub tx_tui: mpsc::Sender<EventSend>,
    pub rx_thread: Option<mpsc::Receiver<EventSend>>,
    pub creating_account_name: Option<String>,

    pub captcha_token: Option<String>,
    pub captcha_input: String,

    pub clipboard: Option<Clipboard>,
}

#[derive(PartialEq, Clone)]
pub enum NetworkStatus {
    Connected,
    Disconnected(String),
}

#[derive(Clone)]
pub enum UiStatusMessage {
    Info(String),
    Error(String),
}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<Box<dyn DisplayRecipient>>),
    LinkingFinished((bool, Option<Manager<SqliteStore, Registered>>)),
    LinkingError(String),
    NetworkStatusChanged(NetworkStatus),

    ProfileReceived(Profile),
    AvatarReceived(Vec<u8>),

    ContactInfoReceived(ContactInfo),
    ContactAvatarReceived(Vec<u8>),

    GetContactMessageHistory(String, Vec<MessageDto>),
    GetGroupMessageHistory(GroupMasterKeyBytes, Vec<MessageDto>),
    ReceiveMessage,
    QrCodeGenerated,
    Resize(u16, u16),
    UiStatus(UiStatusMessage),
    CaptchaError(String),
}
pub enum EventSend {
    SendText(RecipientId, String, Option<MessageDto>),
    SendAttachment(RecipientId, String, String, Option<MessageDto>),
    GetMessagesForContact(String),
    GetMessagesForGroup(GroupMasterKeyBytes),
    GetContactInfo(String),
    SaveAttachment(Box<AttachmentPointer>, PathBuf),
}

impl App {
    pub fn new(linking_status: LinkingStatus) -> App {
        let (tx_thread, rx_tui) = mpsc::channel();
        let (tx_tui, rx_thread) = mpsc::channel();
        let picker = Picker::from_query_stdio().ok();

        let available_accounts = list_accounts().unwrap_or_default();
        let current_account = Config::load().get_current_account().cloned();

        let clipboard = match Clipboard::new() {
            Ok(clipboard) => Some(clipboard),
            Err(e) => {
                error!(error = %e, "Couldn't initialize clipboard, app will run without it.");
                None
            }
        };

        App {
            uuid: None,
            linking_status,
            current_account,
            deleting_account: None,
            available_accounts,
            device_name_input: String::new(),
            account_linking_field: AccountLinkingField::AccountName,
            account_selected: 0,
            recipients: vec![],
            selected_recipient: 0,
            message_selected: 0,
            character_index: 0,
            current_screen: CurrentScreen::LinkingNewDevice,
            textarea: String::new(),
            contact_messages: HashMap::new(),
            group_messages: HashMap::new(),
            network_status: NetworkStatus::Connected,
            attachment_path: String::new(),
            attachment_error: None,
            quoted_message: None,
            input_focus: InputFocus::Message,

            ui_status_info: None,

            retry_manager: Arc::new(Mutex::new(RetryManager::new())),
            message_id_map: HashMap::new(),

            profile: None,
            avatar_cache: None,
            picker,
            avatar_image: None,

            selected_contact_info: None,
            contact_avatar_cache: None,
            contact_avatar_image: None,

            config: Config::load(),
            config_selected: 0,

            manager: None,

            tx_thread,
            rx_tui,
            tx_tui,
            rx_thread: Some(rx_thread),
            creating_account_name: None,

            captcha_token: None,
            captcha_input: String::new(),

            clipboard,
        }
    }

    pub(crate) async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    ) -> io::Result<bool> {
        self.refresh_accounts();

        if self.linking_status == LinkingStatus::Linked
            && let Some(current) = self.current_account.clone()
        {
            if !self.available_accounts.contains(&current) {
                self.current_account = None;
                let mut config = Config::load();
                config.clear_current_account();
                let _ = config.save();
                self.linking_status = LinkingStatus::Unlinked;
            }
            if let Some(rx) = self.rx_thread.take() {
                match create_registered_manager_for_account(&current).await {
                    Ok(manager) => {
                        self.manager = Some(manager.clone());
                        let whoami_response = manager.whoami().await;
                        match whoami_response {
                            Ok(whoami) => self.uuid = Some(whoami.aci),
                            Err(error) => error!(%error, "Failed to fetch whoami info"),
                        }

                        if let Err(e) = init_background_threads(
                            self.tx_thread.clone(),
                            rx,
                            manager,
                            self.retry_manager.clone(),
                        )
                        .await
                        {
                            error!("Failed to init threads: {e:?}");
                        }
                        self.current_screen = CurrentScreen::Syncing;
                    }
                    Err(e) => {
                        self.current_screen = CurrentScreen::AccountSelector;
                        warn!(error = %e, "Getting manager for account wasn't successful");
                    }
                }
            }
        }

        let tx_key_events = self.tx_thread.clone();
        thread::spawn(move || {
            handle_input_events(tx_key_events);
        });

        loop {
            terminal.draw(|f| render_ui(f, self))?;

            if let Ok(event) = self.rx_tui.recv()
                && self.handle_event(event, &self.tx_tui.clone()).await?
            {
                return Ok(true);
            }
        }
    }

    pub async fn switch_account(&mut self, account_name: String) -> Result<()> {
        if !self.available_accounts.contains(&account_name) {
            bail!("Account '{}' does not exist", account_name);
        }

        let mut config = Config::load();
        config.set_current_account(account_name.clone());
        config
            .save()
            .map_err(|e| anyhow!("Failed to save config: {e}"))?;

        self.current_account = Some(account_name.clone());
        self.config = Config::load();

        let new_manager = create_registered_manager_for_account(&account_name).await?;
        self.manager = Some(new_manager.clone());

        let whoami_response = new_manager.whoami().await;
        match whoami_response {
            Ok(whoami) => self.uuid = Some(whoami.aci),
            Err(error) => error!(%error, "Failed to fetch whoami info"),
        }

        self.recipients.clear();
        self.selected_recipient = 0;
        self.contact_messages.clear();
        self.group_messages.clear();
        self.current_screen = CurrentScreen::Syncing;

        if self.rx_thread.is_none() {
            let (tx_tui, rx_thread) = mpsc::channel();
            self.tx_tui = tx_tui;
            self.rx_thread = Some(rx_thread);
        }

        if let Some(rx) = self.rx_thread.take()
            && let Err(e) = init_background_threads(
                self.tx_thread.clone(),
                rx,
                new_manager,
                self.retry_manager.clone(),
            )
            .await
        {
            bail!("Failed to initialize background threads: {}", e);
        }

        Ok(())
    }

    pub async fn delete_account(&mut self, account_name: String) -> Result<()> {
        use std::path::Path;
        let is_current = self.current_account.as_ref() == Some(&account_name);
        let account_dir = format!("{ACCOUNTS_DIR}/{account_name}");
        if Path::new(&account_dir).exists() {
            std::fs::remove_dir_all(&account_dir)?;
        }

        if is_current {
            let mut config = Config::load();
            config.clear_current_account();

            let remaining = list_accounts()?;
            if !remaining.is_empty() {
                config.set_current_account(remaining[0].clone());
                self.current_account = Some(remaining[0].clone());
            } else {
                self.current_account = None;
            }

            config
                .save()
                .map_err(|e| anyhow!("Failed to save config: {e}"))?;
            self.config = Config::load();
        }

        self.refresh_accounts();
        if self.account_selected >= self.available_accounts.len() {
            self.account_selected = self.available_accounts.len().saturating_sub(1);
        }

        Ok(())
    }

    pub fn refresh_accounts(&mut self) {
        self.available_accounts = list_accounts().unwrap_or_default();
    }

    pub fn load_avatar(&mut self) {
        if let (Some(avatar_data), Some(picker)) = (&self.avatar_cache, &mut self.picker) {
            match image::load_from_memory(avatar_data) {
                Ok(dynamic_image) => {
                    self.avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                }
                Err(_) => {
                    if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Png)
                    {
                        self.avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    } else if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Jpeg)
                    {
                        self.avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    }
                }
            }
        }
    }

    pub fn load_contact_avatar(&mut self) {
        if let (Some(avatar_data), Some(picker)) = (&self.contact_avatar_cache, &mut self.picker) {
            match image::load_from_memory(avatar_data) {
                Ok(dynamic_image) => {
                    self.contact_avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                }
                Err(_) => {
                    if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Png)
                    {
                        self.contact_avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    } else if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Jpeg)
                    {
                        self.contact_avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    }
                }
            }
        }
    }

    async fn handle_event(&mut self, event: EventApp, tx: &Sender<EventSend>) -> io::Result<bool> {
        match event {
            EventApp::KeyInput(key) => {
                if key.kind == KeyEventKind::Release {
                    return Ok(false);
                }
                self.handle_key_event(key, tx).await
            }
            EventApp::NetworkStatusChanged(status) => {
                self.network_status = status;
                Ok(false)
            }
            EventApp::LinkingError(error_msg) => {
                self.linking_status = LinkingStatus::Error(error_msg);
                Ok(false)
            }
            EventApp::CaptchaError(token) => {
                self.captcha_token = Some(token);
                self.current_screen = CurrentScreen::Recaptcha;
                Ok(false)
            }
            EventApp::ContactsList(recipients) => {
                if self.current_screen == CurrentScreen::Syncing {
                    self.current_screen = CurrentScreen::Main;
                }
                // This is added because contacts change order in the contact list
                // and if that happens the same contact should remain selected
                let selected_id = self
                    .recipients
                    .get(self.selected_recipient)
                    .map(|contact| contact.0.id())
                    .unwrap_or_default();

                self.recipients = recipients
                    .into_iter()
                    .map(|recipient| (recipient, String::new()))
                    .collect();

                self.selected_recipient = self
                    .recipients
                    .iter()
                    .position(|c| c.0.id() == selected_id)
                    .unwrap_or(0);
                Ok(false)
            }
            EventApp::ContactInfoReceived(contact) => {
                self.selected_contact_info = Some(contact);
                Ok(false)
            }
            EventApp::ContactAvatarReceived(avatar_data) => {
                self.contact_avatar_cache = Some(avatar_data);
                if self.config.show_images {
                    self.load_contact_avatar();
                }
                Ok(false)
            }
            EventApp::LinkingFinished((result, manager_optional)) => {
                match result {
                    true => {
                        self.linking_status = LinkingStatus::Linked;

                        if let Some(account_name) = self.creating_account_name.take() {
                            self.refresh_accounts();
                            self.current_account = Some(account_name.clone());
                            self.config = Config::load();
                        }

                        if self.rx_thread.is_none() {
                            let (tx_tui, rx_thread) = mpsc::channel();
                            self.tx_tui = tx_tui;
                            self.rx_thread = Some(rx_thread);
                        }

                        if let Some(rx) = self.rx_thread.take() {
                            let new_manager = match manager_optional {
                                Some(manager) => manager,
                                None => match create_registered_manager().await {
                                    Ok(manager) => manager,
                                    Err(_e) => {
                                        self.current_screen = CurrentScreen::Main;
                                        error!("Error getting the manager for account");
                                        return Ok(false);
                                    }
                                },
                            };

                            self.manager = Some(new_manager.clone());

                            let whoami_response = new_manager.whoami().await;
                            match whoami_response {
                                Ok(whoami) => self.uuid = Some(whoami.aci),
                                Err(error) => error!(%error, "Failed to fetch whoami info"),
                            }

                            if let Err(e) = init_background_threads(
                                self.tx_thread.clone(),
                                rx,
                                new_manager,
                                Arc::clone(&self.retry_manager),
                            )
                            .await
                            {
                                error!("Failed to init threads: {e:?}");
                                self.current_screen = CurrentScreen::Main;
                                return Ok(false);
                            }
                        }
                        self.current_screen = CurrentScreen::Syncing;
                    }
                    false => self.linking_status = LinkingStatus::Unlinked,
                }
                Ok(false)
            }
            EventApp::ProfileReceived(profile) => {
                self.profile = Some(profile);

                Ok(false)
            }
            EventApp::AvatarReceived(avatar_data) => {
                self.avatar_cache = Some(avatar_data);
                Ok(false)
            }
            EventApp::GetContactMessageHistory(uuid_str, messages) => {
                self.contact_messages.insert(uuid_str, messages);
                self.message_selected = 0;
                Ok(false)
            }
            EventApp::GetGroupMessageHistory(master_key, messages) => {
                self.group_messages.insert(master_key, messages);
                self.message_selected = 0;
                Ok(false)
            }
            EventApp::ReceiveMessage => {
                self.synchronize_messages_for_selected_recipient();
                Ok(false)
            }
            EventApp::QrCodeGenerated => Ok(false),
            EventApp::Resize(_, _) => Ok(false),
            EventApp::UiStatus(message) => {
                let ui_status_info: UiStatusInfo = UiStatusInfo {
                    status_message: message,
                    last_screen: self.current_screen.clone(),
                };
                self.ui_status_info = Some(ui_status_info);
                self.current_screen = CurrentScreen::Popup;
                Ok(false)
            }
        }
    }

    fn enter_char(&mut self, new_char: char) {
        if let Some((_, input)) = self.recipients.get_mut(self.selected_recipient) {
            input.push(new_char);
            self.character_index += 1;
        }
    }

    fn delete_char(&mut self) {
        if let Some((_, input)) = self.recipients.get_mut(self.selected_recipient)
            && self.character_index > 0
        {
            input.pop();
            self.character_index -= 1;
        }
    }

    pub fn is_account_name_valid(&self, name: &str) -> bool {
        !name.is_empty()
            && !self.available_accounts.contains(&name.to_string())
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    pub fn get_account_validation_message(&self) -> (String, bool) {
        if self.textarea.is_empty() {
            ("Account name cannot be empty".to_string(), false)
        } else if self.available_accounts.contains(&self.textarea) {
            ("Account name already exists".to_string(), false)
        } else if !self
            .textarea
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            (
                "Only letters, numbers, '_' and '-' allowed".to_string(),
                false,
            )
        } else {
            ("Ready to create account".to_string(), true)
        }
    }

    fn validate_attachment_path(&mut self) {
        if self.attachment_path.trim().is_empty() {
            self.attachment_error = None;
        } else if !Path::new(&self.attachment_path).exists() {
            self.attachment_error = Some("File does not exist".to_string());
        } else if !Path::new(&self.attachment_path).is_file() {
            self.attachment_error = Some("Path is not a file".to_string());
        } else {
            self.attachment_error = None;
        }
    }

    fn submit_message(&mut self, tx: &Sender<EventSend>) {
        let has_attachment = !self.attachment_path.trim().is_empty();
        if has_attachment {
            self.validate_attachment_path();
            if self.attachment_error.is_some() {
                return;
            }
        }
        if let Some((recipient, input)) = self.recipients.get_mut(self.selected_recipient) {
            let message_text = input.trim().to_string();
            let has_attachment = !self.attachment_path.trim().is_empty();
            let has_text = !message_text.is_empty();

            if has_text || has_attachment {
                let outgoing = OutgoingMessage::new(
                    recipient.id(),
                    message_text.clone(),
                    if has_attachment {
                        Some(self.attachment_path.clone())
                    } else {
                        None
                    },
                    self.quoted_message.clone(),
                );

                if let Ok(mut manager) = self.retry_manager.try_lock() {
                    let _message_id = manager.add_message(outgoing.clone());
                }

                if has_attachment {
                    tx.send(EventSend::SendAttachment(
                        recipient.id(),
                        message_text.clone(),
                        self.attachment_path.clone(),
                        self.quoted_message.clone(),
                    ))
                    .unwrap();
                    self.attachment_path.clear();
                } else {
                    tx.send(EventSend::SendText(
                        recipient.id(),
                        message_text,
                        self.quoted_message.clone(),
                    ))
                    .unwrap();
                }

                input.clear();
                self.character_index = 0;
                self.quoted_message = None;
            }
        }
    }

    // TODO: These unwraps must be handled gracefully
    fn synchronize_messages_for_selected_recipient(&mut self) {
        let recipient_id = match self.recipients.get(self.selected_recipient) {
            Some(recipient) => recipient.0.id(),
            None => return,
        };
        match recipient_id {
            RecipientId::Contact(uuid) => self
                .tx_tui
                .send(EventSend::GetMessagesForContact(uuid.to_string()))
                .unwrap(),
            RecipientId::Group(master_key) => self
                .tx_tui
                .send(EventSend::GetMessagesForGroup(master_key))
                .unwrap(),
        }
    }

    async fn handle_key_event(
        &mut self,
        key: event::KeyEvent,
        tx: &Sender<EventSend>,
    ) -> io::Result<bool> {
        use CurrentScreen::*;
        match self.current_screen {
            Main => match key.code {
                KeyCode::Right | KeyCode::Char('d') => {
                    self.synchronize_messages_for_selected_recipient();
                    self.current_screen = Writing;
                }
                KeyCode::Char('q') | KeyCode::Esc => self.current_screen = Exiting,
                KeyCode::Char('e') => self.current_screen = Options,
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.selected_recipient < self.recipients.len() - 1 {
                        self.selected_recipient += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.selected_recipient > 0 {
                        self.selected_recipient -= 1;
                    }
                }
                KeyCode::Char('a') => {
                    self.refresh_accounts();
                    self.current_screen = AccountSelector;
                }
                KeyCode::Char('i') => {
                    let selected_recipient_id = self.recipients[self.selected_recipient].0.id();
                    let contact_uuid = match selected_recipient_id {
                        RecipientId::Contact(uuid) => uuid.to_string(),
                        // TODO: (@jbrs) Get group info
                        RecipientId::Group(_) => return Ok(false),
                    };
                    self.tx_tui
                        .send(EventSend::GetContactInfo(contact_uuid))
                        .unwrap();
                    self.current_screen = ContactInfo;

                    self.contact_avatar_cache = None;
                    self.contact_avatar_image = None;
                }
                _ => {}
            },
            Exiting => match key.code {
                KeyCode::Char('y') | KeyCode::Esc | KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('n') => self.current_screen = Main,
                _ => {}
            },
            InspectMesseges => match key.code {
                KeyCode::Esc | KeyCode::Left => self.current_screen = Main,
                KeyCode::Char('q') => self.current_screen = Writing,
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.current_screen = Writing
                }

                KeyCode::Up => {
                    let recipient_id = self.recipients[self.selected_recipient].0.id();

                    let last_message = match recipient_id {
                        RecipientId::Contact(uuid) => self
                            .contact_messages
                            .get(&uuid.to_string())
                            .map(|msgs| msgs.len())
                            .unwrap_or(0),
                        RecipientId::Group(master_key) => self
                            .group_messages
                            .get(&master_key)
                            .map(|msgs| msgs.len())
                            .unwrap_or(0),
                    };

                    if last_message > 0 && self.message_selected < last_message - 1 {
                        self.message_selected += 1;
                    }
                }
                KeyCode::Down => {
                    if self.message_selected > 0 {
                        self.message_selected -= 1;
                    }
                }

                KeyCode::Char('r') => {
                    let selected_recipient_id = self.recipients[self.selected_recipient].0.id();
                    self.quoted_message = match selected_recipient_id {
                        RecipientId::Contact(uuid) => {
                            match self.contact_messages.get(&uuid.to_string()) {
                                Some(messeges) => messeges.get(self.message_selected).cloned(),
                                None => None,
                            }
                        }
                        RecipientId::Group(group_key) => {
                            match self.group_messages.get(&group_key) {
                                Some(messeges) => messeges.get(self.message_selected).cloned(),
                                None => None,
                            }
                        }
                    };
                    self.current_screen = Writing;
                }

                KeyCode::Char('s') => {
                    let selected_recipient_id = self.recipients[self.selected_recipient].0.id();
                    let msg = match selected_recipient_id {
                        RecipientId::Contact(uuid) => {
                            match self.contact_messages.get(&uuid.to_string()) {
                                Some(messeges) => messeges.get(self.message_selected),
                                None => None,
                            }
                        }
                        RecipientId::Group(group_key) => {
                            match self.group_messages.get(&group_key) {
                                Some(messeges) => messeges.get(self.message_selected),
                                None => None,
                            }
                        }
                    };

                    let attachment = match msg {
                        Some(message) => message.attachment.clone(),
                        None => None,
                    };

                    if let Some(att) = attachment {
                        // TODO: handle unwraps
                        self.tx_tui
                            .send(EventSend::SaveAttachment(
                                Box::new(att),
                                self.config.attachment_save_dir.clone(),
                            ))
                            .unwrap();
                    }
                }

                _ => {}
            },

            Writing => match key.code {
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.synchronize_messages_for_selected_recipient()
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.current_screen = InspectMesseges
                }
                KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.quoted_message = None;
                }
                KeyCode::Esc | KeyCode::Left => {
                    self.quoted_message = None;
                    self.current_screen = Main
                }
                KeyCode::Tab => {
                    self.input_focus = match self.input_focus {
                        InputFocus::Message => InputFocus::Attachment,
                        InputFocus::Attachment => InputFocus::Message,
                    };
                }

                KeyCode::Enter => {
                    self.submit_message(tx);
                    self.synchronize_messages_for_selected_recipient();
                }
                KeyCode::Char(to_insert) => match self.input_focus {
                    InputFocus::Message => self.enter_char(to_insert),
                    InputFocus::Attachment => {
                        self.attachment_path.push(to_insert);
                        self.validate_attachment_path();
                    }
                },
                KeyCode::Backspace => match self.input_focus {
                    InputFocus::Message => self.delete_char(),
                    InputFocus::Attachment => {
                        self.attachment_path.pop();
                        self.validate_attachment_path();
                    }
                },

                KeyCode::Up => {
                    let selected_recipient_id = self.recipients[self.selected_recipient].0.id();
                    let last_message = match selected_recipient_id {
                        RecipientId::Contact(uuid) => {
                            match self.contact_messages.get(&uuid.to_string()) {
                                Some(messeges) => messeges.len(),
                                None => 0,
                            }
                        }
                        RecipientId::Group(group_key) => {
                            match self.group_messages.get(&group_key) {
                                Some(messeges) => messeges.len(),
                                None => 0,
                            }
                        }
                    };

                    if last_message > 0 && self.message_selected < last_message - 1 {
                        self.message_selected += 1;
                    }
                }
                KeyCode::Down => {
                    if self.message_selected > 0 {
                        self.message_selected -= 1;
                    }
                }
                _ => {}
            },
            Popup => {
                let last_screen: CurrentScreen = match self.ui_status_info.clone() {
                    Some(status) => status.last_screen,
                    None => Main,
                };
                self.current_screen = last_screen;
                self.ui_status_info = None;
            }
            CreatingAccount => match key.code {
                KeyCode::Esc => {
                    let accounts = list_accounts().unwrap_or_default();
                    if accounts.is_empty() {
                        return Ok(false);
                    }

                    self.current_screen = AccountSelector;
                    self.textarea.clear();
                    self.device_name_input.clear();
                    self.account_linking_field = AccountLinkingField::AccountName;
                }
                KeyCode::Tab => {
                    self.account_linking_field = match self.account_linking_field {
                        AccountLinkingField::AccountName => AccountLinkingField::DeviceName,
                        AccountLinkingField::DeviceName => AccountLinkingField::AccountName,
                    };
                }
                KeyCode::Enter => {
                    let account_name = self.textarea.trim().to_string();
                    let device_name = if self.device_name_input.trim().is_empty() {
                        format!("{account_name}-device")
                    } else {
                        self.device_name_input.trim().to_string()
                    };

                    if account_name.is_empty() || self.available_accounts.contains(&account_name) {
                        return Ok(false);
                    }

                    self.creating_account_name = Some(account_name.clone());

                    if Path::new(QRCODE).exists() {
                        let _ = fs::remove_file(QRCODE);
                    }

                    let tx_qr = self.tx_thread.clone();
                    thread::spawn(move || {
                        handle_checking_qr_code(tx_qr);
                    });

                    let tx_link = self.tx_thread.clone();
                    thread::Builder::new()
                        .name(String::from("account_linking_thread"))
                        .stack_size(1024 * 1024 * 8)
                        .spawn(move || {
                            let runtime = Builder::new_multi_thread()
                                .thread_name("account_linking_runtime")
                                .enable_all()
                                .build()
                                .unwrap();
                            runtime.block_on(async move {
                                handle_linking_device_for_account(
                                    tx_link,
                                    account_name,
                                    device_name,
                                )
                                .await;
                            })
                        })
                        .unwrap();

                    self.current_screen = LinkingNewDevice;
                    self.linking_status = LinkingStatus::InProgress;
                    self.textarea.clear();
                    self.device_name_input.clear();
                    self.account_linking_field = AccountLinkingField::AccountName;
                }
                KeyCode::Backspace => match self.account_linking_field {
                    AccountLinkingField::AccountName => {
                        self.textarea.pop();
                    }
                    AccountLinkingField::DeviceName => {
                        self.device_name_input.pop();
                    }
                },
                KeyCode::Char(c) if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' => {
                    match self.account_linking_field {
                        AccountLinkingField::AccountName => {
                            if c != ' ' {
                                self.textarea.push(c);
                            }
                        }
                        AccountLinkingField::DeviceName => {
                            self.device_name_input.push(c);
                        }
                    }
                }
                _ => {}
            },
            AccountSelector => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.current_screen = Main,
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.account_selected > 0 {
                        self.account_selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.account_selected < self.available_accounts.len().saturating_sub(1) {
                        self.account_selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(account_name) = self.available_accounts.get(self.account_selected) {
                        if let Err(e) = self.switch_account(account_name.clone()).await {
                            warn!("Failed to switch account: {e:?}");
                        } else {
                            self.current_screen = Syncing;
                        }
                    }
                }
                KeyCode::Char('a') => {
                    self.current_screen = CreatingAccount;
                    self.textarea.clear();
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(account_name) = self.available_accounts.get(self.account_selected) {
                        if self.available_accounts.len() == 1 {
                            return Ok(false);
                        }
                        self.deleting_account = Some(account_name.clone());
                        self.current_screen = CurrentScreen::ConfirmDelete;
                    }
                }

                _ => {}
            },
            CurrentScreen::ConfirmDelete => match key.code {
                KeyCode::Char('y') => {
                    if let Some(account_name) = self.deleting_account.take() {
                        // Delete the account
                        if let Err(e) = self.delete_account(account_name).await {
                            error!("Failed to delete account: {e:?}");
                        }
                        self.refresh_accounts();
                        self.current_screen = CurrentScreen::AccountSelector;
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.deleting_account = None;
                    self.current_screen = CurrentScreen::AccountSelector;
                }
                _ => {}
            },
            Options => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.current_screen = Main,
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.config_selected > 0 {
                        self.config_selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.config_selected < 2 {
                        self.config_selected += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => match self.config_selected {
                    0 => {
                        self.config.toggle_color_mode();
                        if let Err(e) = self.config.save() {
                            warn!("Failed to save config: {e:?}");
                        }
                    }
                    1 => {
                        self.config.toggle_show_images();
                        if let Err(e) = self.config.save() {
                            warn!("Failed to save config: {e:?}");
                        }
                        if !self.config.show_images {
                            self.avatar_image = None;
                        } else if self.avatar_cache.is_some() {
                            self.load_avatar();
                        }
                    }
                    2 => {
                        self.config.toggle_notifications();
                        if let Err(e) = self.config.save() {
                            warn!("Failed to save config: {e:?}");
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            ContactInfo => match key.code {
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') => {
                    self.current_screen = Main;
                    self.selected_contact_info = None;
                    self.contact_avatar_cache = None;
                    self.contact_avatar_image = None;
                }
                _ => {}
            },
            LinkingNewDevice => match self.linking_status {
                LinkingStatus::Linked => self.current_screen = Syncing,
                LinkingStatus::Unlinked => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Enter => {
                                let accounts = list_accounts().unwrap_or_default();
                                let device_name = if self.textarea.trim().is_empty() {
                                    "My Device".to_string()
                                } else {
                                    self.textarea.trim().to_string()
                                };

                                if Path::new(QRCODE).exists() {
                                    fs::remove_file(QRCODE)?;
                                }

                                let tx_key_events = self.tx_thread.clone();
                                thread::spawn(move || {
                                    handle_checking_qr_code(tx_key_events);
                                });

                                if accounts.is_empty() && self.creating_account_name.is_none() {
                                    self.creating_account_name = Some("default".to_string());

                                    let tx_link = self.tx_thread.clone();
                                    let account_name = "default".to_string();
                                    thread::Builder::new()
                                        .name(String::from("initial_setup_thread"))
                                        .stack_size(1024 * 1024 * 8)
                                        .spawn(move || {
                                            let runtime = Builder::new_multi_thread()
                                                .thread_name("initial_setup_runtime")
                                                .enable_all()
                                                .build()
                                                .unwrap();
                                            runtime.block_on(async move {
                                                handle_linking_device_for_account(
                                                    tx_link,
                                                    account_name,
                                                    device_name,
                                                )
                                                .await;
                                            })
                                        })
                                        .unwrap();
                                }

                                self.linking_status = LinkingStatus::InProgress;
                                self.textarea.clear();
                            }
                            KeyCode::Backspace => {
                                self.textarea.pop();
                            }
                            KeyCode::Esc => {
                                if self.creating_account_name.is_some() {
                                    self.current_screen = CurrentScreen::AccountSelector;
                                    self.creating_account_name = None;
                                } else {
                                    self.current_screen = CurrentScreen::LinkingNewDevice;
                                }
                            }
                            KeyCode::Char(value) => self.textarea.push(value),
                            _ => {}
                        }
                    }
                }
                LinkingStatus::InProgress => {}
                LinkingStatus::Error(ref _error_msg) => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                if self.creating_account_name.is_some() {
                                    self.current_screen = CurrentScreen::AccountSelector;
                                    self.creating_account_name = None;
                                    self.linking_status = LinkingStatus::Unlinked;
                                } else {
                                    self.linking_status = LinkingStatus::Unlinked;
                                }
                            }
                            _ => {
                                self.linking_status = LinkingStatus::Unlinked;
                            }
                        }
                    }
                }
            },
            Syncing => {}
            Recaptcha => match key.code {
                KeyCode::Enter => {
                    const CAPTCHA_PREFIX: &str = "signalcaptcha://";
                    let captcha = self.captcha_input.clone();

                    match captcha.strip_prefix(CAPTCHA_PREFIX) {
                        Some(captcha) => {
                            if let Err(error) = self
                                .manager
                                .as_ref()
                                .expect("Manager not found")
                                .submit_recaptcha_challenge(
                                    self.captcha_token.as_ref().expect(
                                        "Captcha token not found during active captcha challenge",
                                    ),
                                    captcha,
                                )
                                .await
                            {
                                error!(%error, "Failed to complete captcha challenge.");
                            }
                        }
                        None => {
                            error!("Invalid captcha input");
                        }
                    };

                    self.current_screen = Main;
                }
                KeyCode::Char('p') => {
                    if let Some(clipboard) = &mut self.clipboard {
                        let input = clipboard.get_text().unwrap_or_default();
                        self.captcha_input = input;
                    }
                }
                KeyCode::Char('y') => {
                    const CAPTCHA_URL: &str = "https://signalcaptchas.org/challenge/generate";
                    if let Some(clipboard) = &mut self.clipboard
                        && let Err(error) = clipboard.set_text(CAPTCHA_URL)
                    {
                        error!(%error, "Unable to copy captcha url into clipboard.");
                    }
                }
                _ => {}
            },
        }
        Ok(false)
    }
}

fn is_connection_error(e: &Error) -> bool {
    let msg = e.to_string().to_lowercase();
    ["connection", "network", "websocket", "timeout"]
        .iter()
        .any(|keyword| msg.contains(keyword))
}

fn is_captcha_error(e: &Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("captcha")
}

fn is_delivery_confirmation_timeout(e: &Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("websocket closing while waiting for a response")
        || (msg.contains("websocket closing") && msg.contains("waiting"))
        || (msg.contains("websocket closing") && msg.contains("sending"))
        || (msg.contains("timeout") && msg.contains("response"))
}

/// Spawn thread to sync contacts and to send messeges
pub async fn init_background_threads(
    tx_thread: mpsc::Sender<EventApp>,
    rx_thread: mpsc::Receiver<EventSend>,
    mut manager: Manager<SqliteStore, Registered>,
    retry_manager: Arc<Mutex<RetryManager>>,
) -> Result<()> {
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_tui(&mut manager).await?));

    // let local_pool = LocalPoolHandle::new(4);

    //spawn thread to sync contacts and new messages
    let tx_synchronization_events = tx_thread.clone();
    let new_manager = manager.clone();
    let new_contacts = Arc::clone(&current_contacts_mutex);
    thread::Builder::new()
        .name(String::from("synchronization_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("synchronization_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_synchronization(tx_synchronization_events, new_manager, new_contacts).await;
            })
        })
        .unwrap();

    //spawn thread to receive background events
    let new_manager = manager.clone();
    let rx_sending_thread = rx_thread;
    let new_contacts = Arc::clone(&current_contacts_mutex);
    let tx_status_clone = tx_thread.clone();
    let retry_manager_clone = retry_manager.clone();
    thread::Builder::new()
        .name(String::from("background_events_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("background_events_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_background_events(
                    rx_sending_thread,
                    new_manager,
                    new_contacts,
                    tx_status_clone,
                    retry_manager_clone,
                )
                .await;
            })
        })
        .unwrap();

    // Add profile fetching
    let mut profile_manager = manager.clone();
    let tx_profile = tx_thread.clone();
    thread::Builder::new()
        .name(String::from("profile_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("profile_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                if let Ok(profile) = get_profile_tui(&mut profile_manager).await {
                    let _ = tx_profile.send(EventApp::ProfileReceived(profile));
                }
                if let Ok(Some(avatar_data)) =
                    crate::profile::get_my_profile_avatar_tui(&mut profile_manager).await
                {
                    let _ = tx_profile.send(EventApp::AvatarReceived(avatar_data));
                }
                tokio::time::sleep(std::time::Duration::from_secs(100)).await;
            })
        })
        .unwrap();

    Ok(())
}

pub async fn handle_linking_device_for_account(
    tx: mpsc::Sender<EventApp>,
    account_name: String,
    device_name: String,
) {
    let _ = ensure_accounts_dir();

    let result = link_new_device_for_account(account_name.clone(), device_name).await;

    match result {
        Ok(manager) => {
            let mut config = Config::load();
            config.set_current_account(account_name.clone());
            if let Err(e) = config.save() {
                warn!("Failed to save config: {e:?}");
            }

            if Path::new(QRCODE).exists() {
                match fs::remove_file(QRCODE) {
                    Ok(_) => {}
                    Err(e) => error!("Failed to remove file with QR code: {e}"),
                }
            }

            if tx
                .send(EventApp::LinkingFinished((true, Some(manager))))
                .is_err()
            {
                error!("Failed to send linking finished");
            }
        }
        Err(e) => {
            if Path::new(QRCODE).exists() {
                match fs::remove_file(QRCODE) {
                    Ok(_) => {}
                    Err(e) => error!("Failed to remove file with QR code: {e}"),
                }
            }

            let error_msg =
                if e.to_string().contains("connection") || e.to_string().contains("network") {
                    "Network error: Check your connection".to_string()
                } else {
                    e.to_string()
                };

            if tx.send(EventApp::LinkingError(error_msg)).is_err() {
                error!("Failed to send error");
            }
        }
    }
}

pub fn handle_input_events(tx: mpsc::Sender<EventApp>) {
    loop {
        if let Ok(event) = event::read() {
            match event {
                Event::Key(key_event) => {
                    if tx.send(EventApp::KeyInput(key_event)).is_err() {
                        warn!("Failed to send key event");
                        break;
                    }
                }
                Event::Resize(cols, rows) => {
                    if tx.send(EventApp::Resize(cols, rows)).is_err() {
                        warn!("Failed to send resize event");
                        break;
                    }
                }
                _ => {}
            }
        }
    }
}

pub async fn handle_synchronization(
    tx: mpsc::Sender<EventApp>,
    mut manager: Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
) {
    let _receiving_span = span!(Level::TRACE, "Receiving loop").entered();
    let mut previous_contacts: Vec<Box<DisplayContact>> = Vec::new();
    let mut previous_groups: Vec<Box<DisplayGroup>> = Vec::new();
    let mut initialized = false;
    info!("Start initial synchronization");
    loop {
        let messages_stream_result = manager.receive_messages().await;
        match messages_stream_result {
            Ok(messages_stream) => {
                pin_mut!(messages_stream);
                while let Some(received) = messages_stream.next().await {
                    _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                    match received {
                        Received::QueueEmpty => {
                            // NOTE: This is terrible solution but works for now, will have be
                            // changed to something more graceful in furure
                            debug!("Received queue empty");
                            if !initialized {
                                loop {
                                    info!("Synchronizing contacts");
                                    match manager.request_contacts().await {
                                        Ok(_) => {
                                            info!("Synchronized contacts.");
                                            break;
                                        }
                                        Err(e) => {
                                            error!(error = %e, "Failed to synchronize contacts");
                                            tokio::time::sleep(Duration::from_secs(3)).await;
                                        }
                                    }
                                }
                                initialized = true;
                            }
                        }
                        Received::Contacts => {
                            debug!("Received contact");
                        }
                        Received::Content(content) => {
                            debug!("Received content");
                            trace!("Received message: {content:#?}");

                            if initialized {
                                if let Some(formatted_msg) = format_message(&content)
                                    && !formatted_msg.sender
                                {
                                    handle_notification(
                                        &formatted_msg,
                                        &current_contacts_mutex,
                                        &manager,
                                    )
                                    .await;
                                }

                                if let Err(e) = tx.send(EventApp::ReceiveMessage) {
                                    error!(channel_error = %e);
                                }
                            }
                        }
                    }
                    _ = check_contacts(&mut manager, current_contacts_mutex.clone()).await;

                    let contacts_result = contacts::list_contacts_tui(&mut manager).await;
                    let groups_result = groups::list_groups_tui(&mut manager).await;

                    let contacts = match contacts_result {
                        Ok(list) => list,
                        Err(e) => {
                            error!("{e}");
                            continue;
                        }
                    };

                    let groups = match groups_result {
                        Ok(list) => list,
                        Err(e) => {
                            error!("{e}");
                            continue;
                        }
                    };

                    let contact_displays: Vec<Box<DisplayContact>> = contacts
                        .into_iter()
                        .filter_map(|contact_res| {
                            let contact = contact_res.ok()?;

                            let uuid_str = contact.uuid.to_string();

                            let display_name = if !contact.name.is_empty() {
                                contact.name
                            } else if let Some(phone) = contact.phone_number {
                                phone.to_string()
                            } else {
                                uuid_str.clone()
                            };

                            let display_contact =
                                Box::new(DisplayContact::new(display_name, contact.uuid));

                            Some(display_contact)
                        })
                        .collect();

                    let group_displays: Vec<Box<DisplayGroup>> = groups
                        .into_iter()
                        .filter_map(|groups_res| {
                            let (group_master_key, group) = groups_res.ok()?;

                            let display_name = group.title;
                            let display_group =
                                Box::new(DisplayGroup::new(display_name, group_master_key));

                            Some(display_group)
                        })
                        .collect();

                    let contacts_differ = contact_displays != previous_contacts;
                    let groups_differ = group_displays != previous_groups;

                    let display_recipients: Vec<Box<dyn DisplayRecipient>> = contact_displays
                        .iter()
                        .cloned()
                        .map(|c| c as Box<dyn DisplayRecipient>)
                        .chain(
                            group_displays
                                .iter()
                                .cloned()
                                .map(|g| g as Box<dyn DisplayRecipient>),
                        )
                        .collect();

                    if contacts_differ || groups_differ {
                        if initialized
                            && tx.send(EventApp::ContactsList(display_recipients)).is_err()
                        {
                            break;
                        }
                        if contacts_differ {
                            previous_contacts = contact_displays;
                        }
                        if groups_differ {
                            previous_groups = group_displays;
                        }
                    }
                }

                error!("Lost connection to stream, reconnecting in 3 seconds");
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
            Err(e) => {
                error!(error = %e, "Stream failed, retry in 3 seconds");
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

async fn handle_notification(
    formatted_msg: &MessageDto,
    current_contacts_mutex: &AsyncContactsMap,
    manager: &Manager<SqliteStore, Registered>,
) {
    let config = Config::load();
    if !config.notifications_enabled {
        return;
    }

    let (sender_name, group_name) = {
        let contacts = current_contacts_mutex.lock().await;
        let sender = contacts
            .get(&formatted_msg.uuid)
            .map(|c| {
                if !c.name.is_empty() {
                    c.name.clone()
                } else if let Some(phone) = &c.phone_number {
                    phone.to_string()
                } else {
                    formatted_msg.uuid.to_string()
                }
            })
            .unwrap_or_else(|| formatted_msg.uuid.to_string());

        debug!(
            "Sender UUID: {}, Resolved name: {}",
            formatted_msg.uuid, sender
        );

        let group = if let Some(group_context) = &formatted_msg.group_context {
            if let Some(master_key_bytes) = &group_context.master_key {
                let mut master_key = [0u8; 32];
                master_key.copy_from_slice(&master_key_bytes[..32]);

                manager
                    .store()
                    .group(master_key)
                    .await
                    .ok()
                    .flatten()
                    .map(|g| g.title)
            } else {
                None
            }
        } else {
            None
        };

        (sender, group)
    };

    let title = if let Some(group) = group_name {
        format!("{sender_name} → {group}")
    } else {
        sender_name
    };

    info!("Attempting notification for message from: {}", title);
    if let Err(e) = send_notification(&title, &formatted_msg.text) {
        error!("Failed to send notification: {}", e);
    } else {
        info!("Notification sent successfully");
    }
}

pub async fn handle_background_events(
    rx: Receiver<EventSend>,
    manager: Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
    tx_status: mpsc::Sender<EventApp>,
    retry_manager: Arc<Mutex<RetryManager>>,
) {
    let local_pool = LocalPoolHandle::new(4);

    let mut retry_interval = interval(Duration::from_secs(30));
    let mut cleanup_interval = interval(Duration::from_secs(3600));

    loop {
        tokio::select! {
            _ = retry_interval.tick() => {
                handle_retry_tick(
                    &manager,
                    &tx_status,
                    &retry_manager
                ).await;
            }

            _ = cleanup_interval.tick() => {
                handle_cleanup_tick(&retry_manager).await;
            }

            event = async {
                rx.recv().ok()
            } => {
                if let Some(event) = event {
                    handle_incoming_event(
                        event,
                        &manager,
                        &current_contacts_mutex,
                        &tx_status,
                        &retry_manager,
                        &local_pool
                    ).await;
                } else {
                    break;
                }
            }
        }
    }
}

async fn handle_retry_tick(
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
) {
    let mut retry_mgr = retry_manager.lock().await;
    let messages_to_retry = retry_mgr.messages_to_retry();
    drop(retry_mgr);

    let token_re = Regex::new(r"^.* token ([a-f0-9-]+)$").expect("Failed to compile RegEx");
    for msg in messages_to_retry {
        let result = if let Some(attachment_path) = &msg.attachment_path {
            // Fix: Handle RecipientId properly
            match &msg.recipient {
                RecipientId::Contact(uuid) => {
                    send::contact::send_attachment_tui(
                        uuid.to_string(),
                        msg.text.clone(),
                        attachment_path.clone(),
                        msg.quoted_message.clone(),
                        manager.clone(),
                    )
                    .await
                }
                RecipientId::Group(master_key) => {
                    send::group::send_attachment_tui(
                        master_key,
                        msg.text.clone(),
                        attachment_path.clone(),
                        manager.clone(),
                    )
                    .await
                }
            }
        } else {
            match &msg.recipient {
                RecipientId::Contact(uuid) => {
                    send::contact::send_message_tui(
                        uuid.to_string(),
                        msg.text.clone(),
                        msg.quoted_message.clone(),
                        manager.clone(),
                    )
                    .await
                }
                RecipientId::Group(master_key) => {
                    send::group::send_message_tui(
                        *master_key,
                        msg.text.clone(),
                        manager.clone(),
                        msg.quoted_message.clone(),
                    )
                    .await
                }
            }
        };

        let mut retry_mgr = retry_manager.lock().await;
        match result {
            Ok(_) => {
                retry_mgr.mark_sent(&msg.id);
                let _ = tx_status.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
            }
            Err(e) if is_captcha_error(&e) => {
                warn!(error = %e);

                match token_re.captures(e.to_string().as_str()) {
                    Some(caps) => match caps.get(1) {
                        Some(token) => {
                            let token = token.as_str();
                            if let Err(error) =
                                tx_status.send(EventApp::CaptchaError(token.to_string()))
                            {
                                error!(%error, "Failed to send event");
                            }
                        }
                        None => error!("Failed to extract token from error message."),
                    },
                    None => error!("Failed to extract token from error message."),
                }
                let mut retry_mgr = retry_manager.lock().await;
                // Even though not send this message is marked as sent so there is no retry for it.
                retry_mgr.mark_sent(&msg.id);
                drop(retry_mgr);

                _ = tx_status.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
            }
            Err(e) => {
                let error_msg = e.to_string();
                retry_mgr.mark_failed(&msg.id, error_msg.clone());

                if is_connection_error(&e) {
                    let _ = tx_status.send(EventApp::NetworkStatusChanged(
                        NetworkStatus::Disconnected("Cannot send: WiFi disconnected".to_string()),
                    ));
                }
            }
        }
        drop(retry_mgr);
    }
}

async fn handle_cleanup_tick(retry_manager: &Arc<Mutex<RetryManager>>) {
    let mut retry_mgr = retry_manager.lock().await;
    retry_mgr.cleanup_old_messages();
    drop(retry_mgr);
}

async fn handle_incoming_event(
    event: EventSend,
    manager: &Manager<SqliteStore, Registered>,
    current_contacts_mutex: &AsyncContactsMap,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
    local_pool: &LocalPoolHandle,
) {
    match event {
        EventSend::SendText(recipient, text, quoted_message) => {
            handle_send_text_event(
                recipient,
                text,
                quoted_message,
                manager,
                tx_status,
                retry_manager,
                local_pool,
            )
            .await;
        }
        EventSend::SendAttachment(recipient, text, attachment_path, quoted_message) => {
            handle_send_attachment_event(
                recipient,
                text,
                attachment_path,
                quoted_message,
                manager,
                // current_contacts_mutex,
                tx_status,
                retry_manager,
                local_pool,
            )
            .await;
        }
        EventSend::GetMessagesForContact(uuid_str) => {
            handle_get_contact_messages_event(uuid_str, manager, tx_status, local_pool).await;
        }
        EventSend::GetMessagesForGroup(master_key) => {
            handle_get_group_messages_event(master_key, manager, tx_status, local_pool).await;
        }
        EventSend::GetContactInfo(uuid_str) => {
            handle_get_contact_info_event(uuid_str, current_contacts_mutex, tx_status).await;
        }
        EventSend::SaveAttachment(attachment_pointer, attachment_save_dir) => {
            handle_save_attachment_event(
                *attachment_pointer,
                attachment_save_dir,
                manager,
                tx_status,
            )
            .await;
        }
    }
}

async fn handle_save_attachment_event(
    attachment_pointer: AttachmentPointer,
    attachment_save_dir: PathBuf,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
) {
    match save_attachment(attachment_pointer, manager.clone(), attachment_save_dir).await {
        Ok(path) => {
            let _ = tx_status.send(EventApp::UiStatus(UiStatusMessage::Info(format!(
                "Attachment saved to {}",
                path.display()
            ))));
        }
        Err(e) => {
            let _ = tx_status.send(EventApp::UiStatus(UiStatusMessage::Error(format!(
                "Failed to save attachment: {e}"
            ))));
            error!("Failed to save attachment: {:?}", e);
        }
    }
}
async fn handle_send_text_event(
    recipient: RecipientId,
    text: String,
    quoted_message: Option<MessageDto>,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
    local_pool: &LocalPoolHandle,
) {
    let outgoing_msg = OutgoingMessage::new(
        recipient.clone(),
        text.clone(),
        None,
        quoted_message.clone(),
    );
    let message_id = {
        let mut retry_mgr = retry_manager.lock().await;
        retry_mgr.add_message(outgoing_msg)
    };

    let recipient_clone = recipient.clone();
    let text_clone = text.clone();
    let quoted_message_clone = quoted_message.clone();
    let tx_status_clone = tx_status.clone();
    let retry_manager_clone = Arc::clone(retry_manager);
    let manager_clone = manager.clone();

    local_pool.spawn_pinned(move || async move {
        let send_result = match recipient_clone {
            RecipientId::Contact(uuid) => {
                send::contact::send_message_tui(
                    uuid.to_string(),
                    text_clone,
                    quoted_message_clone,
                    manager_clone,
                )
                .await
            }
            RecipientId::Group(master_key) => {
                send::group::send_message_tui(
                    master_key,
                    text_clone,
                    manager_clone,
                    quoted_message_clone,
                )
                .await
            }
        };

        match send_result {
            Ok(_) => {
                // Mark as sent in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;
                retry_mgr.mark_sent(&message_id);
                drop(retry_mgr);

                let _ =
                    tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                let _ = tx_status_clone.send(EventApp::ReceiveMessage);
            }
            Err(e) if is_captcha_error(&e) => {
                // Ok(_) => {
                // let e = "dummy token aaaa-aaaa-aaaa";
                warn!(error = %e);
                let token_re =
                    Regex::new(r"^.* token ([a-f0-9-]+)$").expect("Failed to compile RegEx");

                match token_re.captures(e.to_string().as_str()) {
                    Some(caps) => match caps.get(1) {
                        Some(token) => {
                            let token = token.as_str();
                            if let Err(error) =
                                tx_status_clone.send(EventApp::CaptchaError(token.to_string()))
                            {
                                error!(%error, "Failed to send event");
                            }
                        }
                        None => error!("Failed to extract token from error message."),
                    },
                    None => error!("Failed to extract token from error message."),
                }
                let mut retry_mgr = retry_manager_clone.lock().await;
                retry_mgr.mark_sent(&message_id);
                drop(retry_mgr);
                _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
            }
            Err(e) => {
                // Mark as failed in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;

                if is_delivery_confirmation_timeout(&e) {
                    retry_mgr.mark_sent(&message_id);
                    warn!("Message likely delivered despite confirmation timeout");
                } else {
                    retry_mgr.mark_failed(&message_id, e.to_string());

                    if is_connection_error(&e) {
                        let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                            NetworkStatus::Disconnected(
                                "Cannot send: WiFi disconnected".to_string(),
                            ),
                        ));
                    } else {
                        error!("Error sending message: {e:?}");
                    }
                }
                drop(retry_mgr);
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
async fn handle_send_attachment_event(
    recipient: RecipientId,
    text: String,
    attachment_path: String,
    quoted_message: Option<MessageDto>,
    manager: &Manager<SqliteStore, Registered>,
    // current_contacts_mutex: &AsyncContactsMap,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
    local_pool: &LocalPoolHandle,
) {
    let outgoing_msg = OutgoingMessage::new(
        recipient.clone(),
        text.clone(),
        Some(attachment_path.clone()),
        quoted_message.clone(),
    );
    let message_id = {
        let mut retry_mgr = retry_manager.lock().await;
        retry_mgr.add_message(outgoing_msg)
    };

    let recipient_clone = recipient.clone();
    let text_clone = text.clone();
    let attachment_path_clone = attachment_path.clone();
    let quoted_message_clone = quoted_message.clone();
    let tx_status_clone = tx_status.clone();
    let retry_manager_clone = Arc::clone(retry_manager);
    let manager_clone = manager.clone();

    local_pool.spawn_pinned(move || async move {
        // Handle recipient type - for now, only Contact is implemented
        let send_result = match recipient_clone {
            RecipientId::Contact(uuid) => {
                send::contact::send_attachment_tui(
                    uuid.to_string(),
                    text_clone,
                    attachment_path_clone,
                    quoted_message_clone,
                    manager_clone,
                )
                .await
            }
            RecipientId::Group(master_key) => {
                send::group::send_attachment_tui(
                    &master_key,
                    text_clone,
                    attachment_path_clone,
                    manager_clone,
                )
                .await
            }
        };

        match send_result {
            Ok(_) => {
                // Mark as sent in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;
                retry_mgr.mark_sent(&message_id);
                drop(retry_mgr);

                _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                _ = tx_status_clone.send(EventApp::ReceiveMessage);
            }
            Err(e) if is_captcha_error(&e) => {
                warn!(error = %e);
                let token_re =
                    Regex::new(r"^.* token ([a-f0-9-]+)$").expect("Failed to compile RegEx");

                match token_re.captures(e.to_string().as_str()) {
                    Some(caps) => match caps.get(1) {
                        Some(token) => {
                            let token = token.as_str();
                            if let Err(error) =
                                tx_status_clone.send(EventApp::CaptchaError(token.to_string()))
                            {
                                error!(%error, "Failed to send event");
                            }
                        }
                        None => error!("Failed to extract token from error message."),
                    },
                    None => error!("Failed to extract token from error message."),
                }
                let mut retry_mgr = retry_manager_clone.lock().await;
                // Even though not send this message is marked as sent so there is no retry for it.
                retry_mgr.mark_sent(&message_id);
                drop(retry_mgr);

                _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
            }
            Err(e) => {
                // Mark as failed in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;

                if is_delivery_confirmation_timeout(&e) {
                    retry_mgr.mark_sent(&message_id);
                    warn!("Message likely delivered despite confirmation timeout");
                } else {
                    retry_mgr.mark_failed(&message_id, e.to_string());
                    if is_connection_error(&e) {
                        _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                            NetworkStatus::Disconnected(
                                "Cannot send: WiFi disconnected".to_string(),
                            ),
                        ));
                    } else {
                        error!("Error sending attachment: {e:?}");
                    }
                }
                drop(retry_mgr);
            }
        }
    });
}

async fn handle_get_contact_messages_event(
    uuid_str: String,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    local_pool: &LocalPoolHandle,
) {
    let manager_clone = manager.clone();
    let tx_status_clone = tx_status.clone();

    local_pool.spawn_pinned(move || async move {
        let result =
            contact::list_messages_tui(uuid_str.clone(), "0".to_string(), manager_clone).await;

        let messages = match result {
            Ok(list) => {
                let _ =
                    tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                list
            }
            Err(e) => {
                error!("Failed to list messages: {}", e);
                if is_connection_error(&e) {
                    let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                        NetworkStatus::Disconnected(
                            "Cannot get messages from store: WiFi disconnected".to_string(),
                        ),
                    ));
                }
                Vec::new()
            }
        };

        if !messages.is_empty()
            && let Err(e) =
                tx_status_clone.send(EventApp::GetContactMessageHistory(uuid_str, messages))
        {
            error!("Failed to send contact message history event: {}", e);
        }
    });
}

async fn handle_get_group_messages_event(
    master_key: GroupMasterKeyBytes,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    local_pool: &LocalPoolHandle,
) {
    let manager_clone = manager.clone();
    let tx_status_clone = tx_status.clone();

    local_pool.spawn_pinned(move || async move {
        let result = receive::group::list_messages_tui(manager_clone, master_key, None).await;

        let messages = match result {
            Ok(list) => {
                let _ =
                    tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                list
            }
            Err(e) => {
                error!("Failed to list group messages: {}", e);
                if is_connection_error(&e) {
                    let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                        NetworkStatus::Disconnected(
                            "Cannot get messages from store: WiFi disconnected".to_string(),
                        ),
                    ));
                }
                Vec::new()
            }
        };

        if !messages.is_empty()
            && let Err(e) =
                tx_status_clone.send(EventApp::GetGroupMessageHistory(master_key, messages))
        {
            error!("Failed to send group message history event: {}", e);
        }
    });
}

async fn handle_get_contact_info_event(
    uuid_str: String,
    current_contacts_mutex: &AsyncContactsMap,
    tx_status: &mpsc::Sender<EventApp>,
) {
    let contacts_mutex = Arc::clone(current_contacts_mutex);
    let contacts = contacts_mutex.lock().await;

    if let Ok(uuid) = uuid_str.parse()
        && let Some(contact) = contacts.get(&uuid)
    {
        let contact_info = ContactInfo {
            uuid: contact.uuid.to_string(),
            name: contact.name.clone(),
            phone_number: contact.phone_number.as_ref().map(|p| p.to_string()),
            verified_state: contact.verified.state,
            expire_timer: contact.expire_timer,
            has_avatar: contact.avatar.is_some(),
        };
        if let Err(e) = tx_status.send(EventApp::ContactInfoReceived(contact_info)) {
            error!("Failed to send ContactInfoReceived event: {}", e);
        }

        if let Some(ref avatar_attachment) = contact.avatar {
            let avatar_bytes: Vec<u8> = avatar_attachment.reader.to_vec();
            if let Err(e) = tx_status.send(EventApp::ContactAvatarReceived(avatar_bytes)) {
                error!("Failed to send ContactAvatarReceived event: {}", e);
            }
        }
    }
}

pub fn handle_checking_qr_code(tx: mpsc::Sender<EventApp>) {
    loop {
        if Path::new(QRCODE).exists() {
            if tx.send(EventApp::QrCodeGenerated).is_err() {
                error!("Failed to send QrCodeGenerated event");
            }
            break;
        }
    }
}
