use crate::contacts::get_contacts_tui;
use crate::messages::send::send_message_tui;
use crate::paths::QRCODE;
use crate::ui::ui;
use crate::{
    contacts, create_registered_manager, devices, AsyncContactsMap, AsyncRegisteredManager,
};
use anyhow::Result;
use crossterm::event;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stderr;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
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
}

pub struct App {
    pub contacts: Vec<(String, String)>, // contact_name, input for this contact
    pub selected: usize,
    pub current_screen: CurrentScreen,
    pub linking_status: LinkingStatus,
    pub character_index: usize,
    pub textarea: String,

    pub tx_thread: mpsc::Sender<EventApp>,
    pub rx_tui: mpsc::Receiver<EventApp>,

    pub tx_tui: mpsc::Sender<EventSend>,
    pub rx_thread: Option<mpsc::Receiver<EventSend>>,
}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<String>),
    LinkingFinished(bool),
}
pub enum EventSend {
    SendText(String, String),
}

impl App {
    pub fn new(linking_status: LinkingStatus) -> App {
        let (tx_thread, rx_tui) = mpsc::channel();
        let (tx_tui, rx_thread) = mpsc::channel();
        App {
            linking_status,
            contacts: vec![],
            selected: 0,
            character_index: 0,
            current_screen: CurrentScreen::LinkingNewDevice,
            textarea: String::new(),

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
                if let Err(e) = init_background_threadss(self.tx_thread.clone(), rx).await {
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
            EventApp::ContactsList(contacts) => {
                if self.current_screen == CurrentScreen::Syncing {
                    self.current_screen = CurrentScreen::Main;
                }
                self.contacts = contacts
                    .into_iter()
                    .map(|name| (name, String::new()))
                    .collect();
                Ok(false)
            }
            EventApp::LinkingFinished(result) => {
                match result {
                    true => {
                        self.linking_status = LinkingStatus::Linked;
                        if let Some(rx) = self.rx_thread.take() {
                            let _ = init_background_threadss(self.tx_thread.clone(), rx).await;
                        }
                        self.current_screen = CurrentScreen::Syncing;
                    }
                    false => self.linking_status = LinkingStatus::Unlinked,
                }
                Ok(false)
            }
        }
    }

    fn enter_char(&mut self, new_char: char) {
        if let Some((_, input)) = self.contacts.get_mut(self.selected) {
            input.push(new_char);
            self.character_index += 1;
        }
    }

    fn delete_char(&mut self) {
        if let Some((_, input)) = self.contacts.get_mut(self.selected) {
            input.pop();
            self.character_index -= 1;
        }
    }
    fn submit_message(&mut self, tx: &Sender<EventSend>) {
        if let Some((name, input)) = self.contacts.get_mut(self.selected) {
            if !input.trim().is_empty() {
                let message = input.clone();
                tx.send(EventSend::SendText(name.clone(), message)).unwrap();
                input.clear();
                self.character_index = 0;
            }
        }
    }

    fn handle_key_event(
        &mut self,
        key: event::KeyEvent,
        tx: &Sender<EventSend>,
    ) -> io::Result<bool> {
        use CurrentScreen::*;
        match self.current_screen {
            Main => match key.code {
                KeyCode::Right | KeyCode::Char('d') => self.current_screen = Writing,
                KeyCode::Char('q') | KeyCode::Esc => self.current_screen = Exiting,
                KeyCode::Char('e') => self.current_screen = Options,
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.selected < self.contacts.len() - 1 {
                        self.selected += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.selected > 0 {
                        self.selected -= 1;
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
                KeyCode::Enter => self.submit_message(tx),
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Backspace => self.delete_char(),
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
                }
            }
            Syncing => {}
        }
        Ok(false)
    }
}

/// spawn thread to sync contacts and to send messeges
pub async fn init_background_threadss(
    tx_thread: mpsc::Sender<EventApp>,
    rx_thread: mpsc::Receiver<EventSend>,
) -> Result<()> {
    let manager: AsyncRegisteredManager = Arc::new(RwLock::new(create_registered_manager().await?));

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
                handle_sending_messages(rx_sending_thread, new_manager, new_contacts).await;
            })
            // let x = rx_thread;
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
    let mut previous_contacts: Vec<String> = Vec::new();

    loop {
        let new_mutex = Arc::clone(&manager_mutex);
        let new_contacts_mutex = Arc::clone(&current_contacts_mutex);
        contacts::sync_contacts_tui(new_mutex, new_contacts_mutex)
            .await
            .unwrap();

        let new_mutex = Arc::clone(&manager_mutex);
        let result = contacts::list_contacts_tui(new_mutex).await;

        let contacts = match result {
            Ok(list) => list,
            Err(_) => continue,
        };

        let contact_names: Vec<String> = contacts
            .into_iter()
            .filter_map(|contact_res| {
                let contact = contact_res.ok()?;
                let mut info = contact.name;
                if info.is_empty() {
                    if contact.phone_number.is_some() {
                        info = contact.phone_number.unwrap().to_string();
                    } else {
                        info = contact.uuid.to_string();
                    }
                }
                if info.is_empty() {
                    None
                } else {
                    Some(info)
                }
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

        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    }
}

pub async fn handle_sending_messages(
    rx: Receiver<EventSend>,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) {
    loop {
        if let Ok(event) = rx.recv() {
            match event {
                EventSend::SendText(recipient, text) => {
                    if let Result::Err(err_mess) = send_message_tui(
                        recipient,
                        text,
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await
                    {
                        println!("{:?}", err_mess)
                    }
                    contacts::sync_contacts_tui(
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await
                    .unwrap();
                    // Need to add error handling
                }
            }
        }
    }
}

pub async fn handle_linking_device(tx: mpsc::Sender<EventApp>, device_name: String) {
    let result = devices::link_new_device_tui(device_name).await;

    let success = result.is_ok();

    if tx.send(EventApp::LinkingFinished(success)).is_err() {
        eprintln!("Failed to send linking status");
    }
}
