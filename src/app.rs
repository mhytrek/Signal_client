use crate::contacts;
use crate::ui::ui;
use crossterm::event;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::io::Stderr;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

pub enum CurrentScreen {
    Main,
    Writing,
    Options,
    Exiting,
}

pub struct App {
    pub contacts: Vec<String>,
    pub selected: usize,
    pub current_screen: CurrentScreen,
}

impl App {
    pub fn new() -> App {
        App {
            contacts: vec![],
            selected: 0,
            current_screen: CurrentScreen::Main,
        }
    }

    pub(crate) fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
        rx: Receiver<EventApp>,
    ) -> io::Result<bool> {
        loop {
            terminal.draw(|f| ui(f, self))?;

            if let Ok(event) = rx.recv() {
                if self.handle_event(event)? {
                    return Ok(true);
                }
            }
        }
    }

    fn handle_event(&mut self, event: EventApp) -> io::Result<bool> {
        match event {
            EventApp::KeyInput(key) => {
                if key.kind == KeyEventKind::Release {
                    return Ok(false);
                }
                self.handle_key_event(key)
            }
            EventApp::ContactsList(contacts) => {
                self.contacts = contacts;
                Ok(false)
            }
        }
    }

    fn handle_key_event(&mut self, key: event::KeyEvent) -> io::Result<bool> {
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
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('a') => self.current_screen = Main,
                _ => {}
            },
            Options => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.current_screen = Main,
                _ => {}
            },
        }
        Ok(false)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<String>),
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

pub async fn handle_contacts(tx: mpsc::Sender<EventApp>) {
    let mut previous_contacts: Vec<String> = Vec::new();

    loop {
        thread::sleep(std::time::Duration::from_secs(1));

        let result = contacts::list_contacts().await;

        let contacts = match result {
            Ok(list) => list,
            Err(_) => continue,
        };

        let contact_names: Vec<String> = contacts
            .into_iter()
            .filter_map(|contact| match contact {
                Ok(contact) => {
                    let name = contact.name.trim().to_string();
                    if name.is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                }
                Err(_) => None,
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
    }
}
