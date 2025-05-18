use crate::contacts::get_contacts_tui;
use crate::messages::send::{send_attachment_tui, send_message_tui};
use crate::messages::receive::{self, list_messages_tui, MessageDto};
use crate::paths::QRCODE;
use crate::profile::get_profile_tui;
use crate::ui::ui;
use crate::{
    contacts, create_registered_manager, devices, AsyncContactsMap, AsyncRegisteredManager,
};
use anyhow::Result;
use crossterm::event;
use crossterm::event::{KeyCode, KeyEventKind};
use presage::libsignal_service::Profile;
use presage::libsignal_service::prelude::Uuid;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::collections::HashMap;
use std::io::Stderr;
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};
use tokio::runtime::Builder;
use tokio::sync::{Mutex, RwLock};

use std::thread;

#[derive(PartialEq)]
pub enum CurrentScreen {
    Main,
    Syncing,
    LinkingNewDevice,
    Writing,
    Options,
    Exiting,
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

pub struct App {
    pub contacts: Vec<(String, String, String)>, // contact_uuid, contact_name, input for this contact

    pub contact_selected: usize,
    pub message_selected: usize,

    pub current_screen: CurrentScreen,
    pub linking_status: LinkingStatus,
    pub network_status: NetworkStatus,

    pub character_index: usize,
    pub textarea: String,
    pub attachment_path: String,

    pub input_focus: InputFocus,

    pub profile: Option<Profile>,
    pub contact_messages: HashMap<String,Vec<MessageDto>>,

    pub manager: Option<AsyncRegisteredManager>,

    pub tx_thread: mpsc::Sender<EventApp>,
    pub rx_tui: mpsc::Receiver<EventApp>,

    pub tx_tui: mpsc::Sender<EventSend>,
    pub rx_thread: Option<mpsc::Receiver<EventSend>>,
}

#[derive(PartialEq, Clone)]
pub enum NetworkStatus {
    Connected,
    Disconnected(String),
}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<(String, String)>),
    LinkingFinished(bool),
    LinkingError(String),
    NetworkStatusChanged(NetworkStatus),
    ProfileReceived(Profile),
    GetMessageHistory(HashMap<String,Vec<MessageDto>>),
    ReceiveMessage(HashMap<String,Vec<MessageDto>>)
}
pub enum EventSend {
    SendText(String, String),
    SendAttachment(String, String, String),
}

impl App {
    pub fn new(linking_status: LinkingStatus) -> App {
        let (tx_thread, rx_tui) = mpsc::channel();
        let (tx_tui, rx_thread) = mpsc::channel();
        App {
            linking_status,
            contacts: vec![],
            contact_selected: 0,
            message_selected: 0,
            character_index: 0,
            current_screen: CurrentScreen::LinkingNewDevice,
            textarea: String::new(),
            contact_messages: HashMap::new(),
            network_status: NetworkStatus::Connected,
            attachment_path: String::new(),
            input_focus: InputFocus::Message,

            profile: None,

            manager: None,

            tx_thread,
            rx_tui,
            tx_tui,
            rx_thread: Some(rx_thread),
        }
    }

    pub(crate) async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    ) -> io::Result<bool> {
        if self.linking_status == LinkingStatus::Linked {
            if let Some(rx) = self.rx_thread.take() {
                let new_manager: AsyncRegisteredManager = match create_registered_manager().await {
                    Ok(manager) => Arc::new(RwLock::new(manager)),
                    Err(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            "Failed to create manager",
                        ));
                    }
                };
                let new_manager_mutex = Arc::clone(&new_manager);

                self.manager = Some(new_manager);

                if let Err(e) =
                    init_background_threads(self.tx_thread.clone(), rx, new_manager_mutex).await
                {
                    eprintln!("Failed to init threads: {:?}", e);
                }
            }
            self.current_screen = CurrentScreen::Syncing;
        }

        let tx_key_events = self.tx_thread.clone();
        thread::spawn(move || {
            handle_key_input_events(tx_key_events);
        });

        loop {
            terminal.draw(|f| ui(f, self))?;

            match self.rx_tui.try_recv() {
                Ok(event) => {
                    if self.handle_event(event, &self.tx_tui.clone()).await? {
                        return Ok(true);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Ok(false);
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
                self.handle_key_event(key, tx)
            }
            EventApp::NetworkStatusChanged(status) => {
                self.network_status = status;
                Ok(false)
            }
            EventApp::LinkingError(error_msg) => {
                self.linking_status = LinkingStatus::Error(error_msg);
                Ok(false)
            }
            EventApp::ContactsList(contacts) => {
                if self.current_screen == CurrentScreen::Syncing {
                    self.get_messages(contacts.clone());
                }
                self.contacts = contacts
                    .into_iter()
                    .map(|(uuid, name)| (uuid, name, String::new()))
                    .collect();
                Ok(false)
            }
            EventApp::LinkingFinished(result) => {
                match result {
                    true => {
                        self.linking_status = LinkingStatus::Linked;
                        if let Some(rx) = self.rx_thread.take() {
                            let new_manager: AsyncRegisteredManager =
                                match create_registered_manager().await {
                                    Ok(manager) => Arc::new(RwLock::new(manager)),
                                    Err(_) => {
                                        return Err(io::Error::new(
                                            io::ErrorKind::Other,
                                            "Failed to create manager",
                                        ));
                                    }
                                };
                            let new_manager_mutex = Arc::clone(&new_manager);

                            self.manager = Some(new_manager);

                            if let Err(e) = init_background_threads(
                                self.tx_thread.clone(),
                                rx,
                                new_manager_mutex,
                            )
                            .await
                            {
                                eprintln!("Failed to init threads: {:?}", e);
                            }
                        }
                        self.current_screen = CurrentScreen::Syncing;
                    }
                    false => self.linking_status = LinkingStatus::Unlinked,
                }
                Ok(false)
            },
            EventApp::ProfileReceived(profile) => {
                self.profile = Some(profile);
                Ok(false)
            },
            
            EventApp::GetMessageHistory(messeges_map) => {
                if self.current_screen == CurrentScreen::Syncing {
                    self.current_screen = CurrentScreen::Main;
                }
                self.contact_messages = messeges_map;
                Ok(false)
            }
            EventApp::ReceiveMessage(messages_map) => {
                for (uuid, messages) in messages_map {
                    self.contact_messages
                        .entry(uuid)
                        .or_default()
                        .extend(messages);
                }
                Ok(false)
            }
        }
    }
    

    fn enter_char(&mut self, new_char: char) {
        if let Some((_, _, input)) = self.contacts.get_mut(self.contact_selected) {
            input.push(new_char);
            self.character_index += 1;
        }
    }

    fn delete_char(&mut self) {
        if let Some((_, _, input)) = self.contacts.get_mut(self.contact_selected) {
            if self.character_index > 0 {
                input.pop();
                self.character_index -= 1;
            }
        }
    }
    fn submit_message(&mut self, tx: &Sender<EventSend>) {
        if let Some((uuid,name, input)) = self.contacts.get_mut(self.contact_selected) {
                if !input.trim().is_empty() {
                let message = if input.trim().is_empty() {
                    "".to_string()
                } else {
                    input.clone()
                };

                if !self.attachment_path.trim().is_empty() {
                    tx.send(EventSend::SendAttachment(
                        name.clone(),
                        message,
                        self.attachment_path.clone(),
                    ))
                    .unwrap();
                    self.attachment_path.clear();
                } else if !message.is_empty() {
                    tx.send(EventSend::SendText(name.clone(), message)).unwrap();
                }

                self.character_index = 0;
                let uuid_raw = Uuid::from_str(uuid).expect("contact does not exist");
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("cannot make timestamp")
                    .as_millis() as u64;
                self.contact_messages
                    .entry(uuid.clone())
                    .or_default()
                    .push(MessageDto {
                        uuid: uuid_raw,
                        timestamp,
                        text: input.clone(),
                        sender: true,
                    });
                input.clear();

            }
        }
    }


    fn get_messages(&mut self, contacts: Vec<(String, String)>) {
        //spawn thread to get messages
        let tx_get_messages_event = self.tx_thread.clone();
        let new_manager_mutex: AsyncRegisteredManager = Arc::clone(&self.manager.clone().unwrap());
        let contacts_uuids = contacts.iter().map(|(uuid, _)| uuid.clone()).collect();

        thread::Builder::new()
            .name(String::from("getting_messages_thread"))
            .stack_size(1024 * 1024 * 8)
            .spawn(move || {
                let runtime = Builder::new_multi_thread()
                    .thread_name("getting_messages_runtime")
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    handle_getting_messages(
                        tx_get_messages_event,
                        contacts_uuids,
                        new_manager_mutex,
                    )
                    .await;
                })
            })
            .unwrap();
    }

    fn handle_key_event(
        &mut self,
        key: event::KeyEvent,
        tx: &Sender<EventSend>,
    ) -> io::Result<bool> {
        use CurrentScreen::*;
        match self.current_screen {
            Main => match key.code {
                KeyCode::Right | KeyCode::Char('d') => {
                    self.message_selected = match self
                        .contact_messages
                        .get(&self.contacts[self.contact_selected].0)
                    {
                        Some(msgs) => msgs.len().max(0),
                        None => 0,
                    };
                    self.current_screen = Writing;
                }
                KeyCode::Char('q') | KeyCode::Esc => self.current_screen = Exiting,
                KeyCode::Char('e') => self.current_screen = Options,
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.contact_selected < self.contacts.len() - 1 {
                        self.contact_selected += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.contact_selected > 0 {
                        self.contact_selected -= 1;
                    }
                }
                _ => {}
            },
            Exiting => match key.code {
                KeyCode::Char('y') | KeyCode::Esc | KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('n') => self.current_screen = Main,
                _ => {}
            },
            Writing => match key.code {
                KeyCode::Esc | KeyCode::Left => self.current_screen = Main,
                KeyCode::Tab => {
                    self.input_focus = match self.input_focus {
                        InputFocus::Message => InputFocus::Attachment,
                        InputFocus::Attachment => InputFocus::Message,
                    };
                }
                KeyCode::Enter => self.submit_message(tx),
                KeyCode::Char(to_insert) => match self.input_focus {
                    InputFocus::Message => self.enter_char(to_insert),
                    InputFocus::Attachment => self.attachment_path.push(to_insert),
                },
                KeyCode::Backspace => match self.input_focus {
                    InputFocus::Message => self.delete_char(),
                    InputFocus::Attachment => {
                        self.attachment_path.pop();
                    }
                },

                KeyCode::Down  => {
                    let last_message= match self.contact_messages.get(&self.contacts[self.contact_selected].0) {
                        Some(msgs) => msgs.len(),
                        None => 0,
                    };

                    if self.message_selected < last_message - 1 {
                        self.message_selected += 1;
                    }
                }
                KeyCode::Up => {
                    if self.message_selected > 0 {
                        self.message_selected -= 1;
                    }
                }
                _ => {}
            },
            Options => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.current_screen = Main,
                _ => {}
            },
            LinkingNewDevice => {
                match self.linking_status {
                    LinkingStatus::Linked => self.current_screen = Syncing,
                    LinkingStatus::Unlinked => {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    if Path::new(QRCODE).exists() {
                                        fs::remove_file(QRCODE)?;
                                    }

                                    //spawn thread to link device
                                    let device_name = self.textarea.clone();
                                    let tx_link_device_event = self.tx_thread.clone();
                                    thread::Builder::new()
                                        .name(String::from("linking_device_thread"))
                                        .stack_size(1024 * 1024 * 8)
                                        .spawn(move || {
                                            let runtime = Builder::new_multi_thread()
                                                .thread_name("linking_device_runtime")
                                                .enable_all()
                                                .build()
                                                .unwrap();
                                            runtime.block_on(async move {
                                                handle_linking_device(
                                                    tx_link_device_event,
                                                    device_name,
                                                )
                                                .await;
                                            })
                                        })
                                        .unwrap();

                                    self.linking_status = LinkingStatus::InProgress
                                }
                                KeyCode::Backspace => {
                                    self.textarea.pop();
                                }

                                KeyCode::Esc => {
                                    self.current_screen = LinkingNewDevice;
                                }

                                KeyCode::Char(value) => self.textarea.push(value),

                                _ => {}
                            }
                        }
                    }
                    LinkingStatus::InProgress => {}
                    LinkingStatus::Error(_) => {
                        if key.kind == KeyEventKind::Press {
                            self.linking_status = LinkingStatus::Unlinked;
                        }
                    }
                }
            }
            Syncing => {}
        }
        Ok(false)
    }
}

/// spawn thread to sync contacts and to send messeges
pub async fn init_background_threads(
    tx_thread: mpsc::Sender<EventApp>,
    rx_thread: mpsc::Receiver<EventSend>,
    manager: AsyncRegisteredManager,
) -> Result<()> {
    let new_manager_mutex = Arc::clone(&manager);
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_tui(new_manager_mutex).await?));

    //spawn thread to sync contacts
    let tx_contacts_events = tx_thread.clone();
    let new_manager = Arc::clone(&manager);
    let new_contacts = Arc::clone(&current_contacts_mutex);
    thread::Builder::new()
        .name(String::from("contacts_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("contacts_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_contacts(tx_contacts_events, new_manager, new_contacts).await;
            })
        })
        .unwrap();

    //spawn thread to send messeges
    let new_manager = Arc::clone(&manager);
    let rx_sending_thread = rx_thread;
    let new_contacts = Arc::clone(&current_contacts_mutex);
    // thread::spawn(move || {
    let tx_status_clone = tx_thread.clone();
    thread::Builder::new()
        .name(String::from("sending_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("sending_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_sending_messages(
                    rx_sending_thread,
                    new_manager,
                    new_contacts,
                    tx_status_clone,
                )
                .await;
            })
        })
        .unwrap();

    // Add profile fetching
    let profile_manager = Arc::clone(&manager);
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
                if let Ok(profile) = get_profile_tui(Arc::from(profile_manager)).await {
                    let _ = tx_profile.send(EventApp::ProfileReceived(profile));
                }
                tokio::time::sleep(std::time::Duration::from_secs(100)).await;
            })
        })
        .unwrap();

    //spawn thread to receive new messages
    let tx_receive_events = tx_thread.clone();
    let new_manager = Arc::clone(&manager);
    let new_contacts = Arc::clone(&current_contacts_mutex);
    thread::Builder::new()
        .name(String::from("receive_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("receive_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_receiving_new_messages(tx_receive_events, new_manager, new_contacts).await;
            })
        })
        .unwrap();

    Ok(())
}

pub fn handle_key_input_events(tx: mpsc::Sender<EventApp>) {
    loop {
        if let Ok(event::Event::Key(key_event)) = crossterm::event::read() {
            if tx.send(EventApp::KeyInput(key_event)).is_err() {
                eprintln!("Failed to send key event");
                break;
            }
        }
    }
}

pub async fn handle_contacts(
    tx: mpsc::Sender<EventApp>,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) {
    let mut previous_contacts: Vec<(String, String)> = Vec::new();

    loop {
        let new_mutex = Arc::clone(&manager_mutex);
        let new_contacts_mutex = Arc::clone(&current_contacts_mutex);
        match contacts::sync_contacts_tui(new_mutex, new_contacts_mutex).await {
            Ok(_) => {
                let _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
            }
            Err(e) => {
                if e.to_string().contains("connection")
                    || e.to_string().contains("network")
                    || e.to_string().contains("Websocket")
                    || e.to_string().contains("timeout")
                {
                    let _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Disconnected(
                        "WiFi connection lost".to_string(),
                    )));
                }
            }
        };

        let new_mutex = Arc::clone(&manager_mutex);
        let result = contacts::list_contacts_tui(new_mutex).await;

        let contacts = match result {
            Ok(list) => list,
            Err(_) => continue,
        };

        let contact_names: Vec<(String, String)> = contacts
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

                Some((uuid_str, display_name))
            })
            .collect();

        if contact_names != previous_contacts {
            if tx
                .send(EventApp::ContactsList(contact_names.clone()))
                .is_err()
            {
                break;
            }

            previous_contacts = contact_names;
        }

        tokio::time::sleep(std::time::Duration::from_secs(200)).await;
    }
}

pub async fn handle_receiving_new_messages(
    tx: mpsc::Sender<EventApp>,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) {
    loop {
        let new_contacts_mutex = Arc::clone(&current_contacts_mutex);

        let new_mutex = Arc::clone(&manager_mutex);
        let result = receive::receive_messages_tui(new_mutex, new_contacts_mutex).await;

        let messages = match result {
            Ok(list) => list,
            Err(_) => continue,
        };

        if !messages.is_empty() {
            let mut message_map: HashMap<String, Vec<MessageDto>> = HashMap::new();

            for msg in messages {
                let key = msg.uuid.to_string();
                message_map.entry(key).or_default().push(msg);
            }

            if tx.send(EventApp::ReceiveMessage(message_map)).is_err() {
                break;
            }
        }
    }
}

pub async fn handle_sending_messages(
    rx: Receiver<EventSend>,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
    tx_status: mpsc::Sender<EventApp>,
) {
    loop {
        if let Ok(event) = rx.recv() {
            match event {
                EventSend::SendText(recipient, text) => {
                    match send_message_tui(
                        recipient.clone(),
                        text.clone(),
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await
                    {
                        Ok(_) => {
                            let _ = tx_status
                                .send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                        }
                        Err(err) => {
                            if err.to_string().contains("connection")
                                || err.to_string().contains("network")
                                || err.to_string().contains("Websocket")
                                || err.to_string().contains("timeout")
                            {
                                let _ = tx_status.send(EventApp::NetworkStatusChanged(
                                    NetworkStatus::Disconnected(
                                        "Cannot send: WiFi disconnected".to_string(),
                                    ),
                                ));
                            } else {
                                println!("Error sending message: {:?}", err);
                            }
                        }
                    }

                    let _ = contacts::sync_contacts_tui(
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await;
                }
                EventSend::SendAttachment(recipient, text, attachment_path) => {
                    match send_attachment_tui(
                        recipient.clone(),
                        text.clone(),
                        attachment_path,
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await
                    {
                        Ok(_) => {
                            let _ = tx_status
                                .send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                        }
                        Err(err) => {
                            if err.to_string().contains("connection")
                                || err.to_string().contains("network")
                                || err.to_string().contains("Websocket")
                                || err.to_string().contains("timeout")
                            {
                                let _ = tx_status.send(EventApp::NetworkStatusChanged(
                                    NetworkStatus::Disconnected(
                                        "Cannot send: WiFi disconnected".to_string(),
                                    ),
                                ));
                            } else {
                                println!("Error sending attachment: {:?}", err);
                            }
                        }
                    }

                    let _ = contacts::sync_contacts_tui(
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await;
                }
            }
        }
    }
}

pub async fn handle_getting_messages(
    tx: mpsc::Sender<EventApp>,
    contacts: Vec<String>,
    manager: AsyncRegisteredManager,
) {
    let mut messages_hashmap = HashMap::new();
    for contact in contacts {
        let new_mutex = Arc::clone(&manager);
        let result = list_messages_tui(contact.clone(), "0".to_string(), new_mutex).await;
        let messages = result.unwrap_or_default();
        messages_hashmap.insert(contact.clone(), messages);
    }

    if tx
        .send(EventApp::GetMessageHistory(messages_hashmap))
        .is_err()
    {}
}

pub async fn handle_linking_device(tx: mpsc::Sender<EventApp>, device_name: String) {
    let result = devices::link_new_device_tui(device_name).await;

    match result {
        Ok(_) => {
            if tx.send(EventApp::LinkingFinished(true)).is_err() {
                eprintln!("Failed to send linking status");
            }
        }
        Err(e) => {
            let error_msg = if e.to_string().contains("connection")
                || e.to_string().contains("network")
                || e.to_string().contains("unreachable")
                || e.to_string().contains("timeout")
            {
                "Network error: Please check your WiFi connection".to_string()
            } else {
                e.to_string()
            };

            if tx.send(EventApp::LinkingError(error_msg)).is_err() {
                eprintln!("Failed to send linking error");
            }
        }
    }
}
