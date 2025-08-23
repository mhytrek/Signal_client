use std::{
    fs::{self},
    path::Path,
};

use chrono::{DateTime, Local, Utc};
use qrcode::QrCode;
use ratatui::layout::Alignment;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};
use ratatui_image::{Resize, StatefulImage};
use tui_qrcode::{Colors, QrCodeWidget};

use crate::{
    app::{App, CurrentScreen, InputFocus, LinkingStatus, NetworkStatus},
    messages::receive::MessageDto,
    paths::QRCODE, ui::{chat::render_chat, contact_list::{render_contact_info_compact, render_contact_list}, linking::{render_qrcode, render_textarea}, options::render_options, utils::{render_paragraph, render_popup}},
};

/// Main UI rendering function.
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)])
        .split(chunks[0]);

    match app.current_screen {
        CurrentScreen::Main => {
            render_contact_list(frame, app, main_chunks[0]);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::Writing => {
            render_contact_list(frame, app, main_chunks[0]);
            render_chat(frame, app, main_chunks[1]);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::Options => {
            render_options(frame, app); // app jest już &mut
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::Exiting => {
            render_popup(frame, frame.area(), "Would you like to quit? \n (y/n)");
        }
        CurrentScreen::LinkingNewDevice => match app.linking_status {
            LinkingStatus::Unlinked => {
                render_textarea(frame, app, frame.area());
            }
            LinkingStatus::InProgress => {
                render_qrcode(frame, chunks[0]);
                let text = "Scan the QR code to link new device...";
                render_paragraph(frame, chunks[1], text);
            }
            LinkingStatus::Linked => {}
            LinkingStatus::Error(ref _error_msg) => {
                render_popup(
                    frame,
                    frame.area(),
                    "Error linking device, check if you have Internet connection.\n PRESS ANY KEY TO RETRY",
                );
            }
        },
        CurrentScreen::Syncing => {
            render_popup(frame, frame.area(), "Syncing contacts and messages...");
        }
        CurrentScreen::ContactInfo => {
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Ratio(1, 4),
                    Constraint::Ratio(1, 4),
                    Constraint::Ratio(2, 4),
                ])
                .split(chunks[0]);

            render_contact_list(frame, app, horizontal_chunks[0]);
            render_contact_info_compact(frame, app, horizontal_chunks[1]);
            render_footer(frame, app, chunks[1]);
        }
    }
}

/// Renders the footer section at the bottom of the screen.
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let current_keys_hint = {
        match app.current_screen {
            CurrentScreen::Main => Span::styled(
                "(q) to quit | (↑ ↓) to navigate | (→) to select chat | (i) for contact info | (e) for options",
                Style::default().fg(app.config.get_primary_color()),
            ),
            CurrentScreen::Writing => {
                if app.attachment_error.is_some() {
                    Span::styled(
                        "(q) to exit | (ENTER) to send | (TAB) to switch input/attachment | Fix attachment path to send",
                        Style::default().fg(app.config.get_error_color()),
                    )
                } else {
                    Span::styled(
                        "(q) to exit | (ENTER) to send | (TAB) to switch input/attachment",
                        Style::default().fg(app.config.get_primary_color()),
                    )
                }
            }
            CurrentScreen::Options => Span::styled(
                "(q) to exit | (↑ ↓) to navigate | (ENTER/SPACE) to toggle option",
                Style::default().fg(app.config.get_primary_color()),
            ),
            CurrentScreen::ContactInfo => Span::styled(
                "(q) to exit | (← or ESC) to go back to main",
                Style::default().fg(app.config.get_primary_color()),
            ),
            _ => Span::default(),
        }
    };

    let network_status = match &app.network_status {
        NetworkStatus::Connected => Span::styled(
            "⚡ Online",
            Style::default().fg(app.config.get_success_color()),
        ),
        NetworkStatus::Disconnected(msg) => Span::styled(
            format!("⚠ {msg}"),
            Style::default().fg(app.config.get_error_color()),
        ),
    };

    let footer_text = Line::from(vec![current_keys_hint, Span::raw(" | "), network_status]);

    let key_notes_footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.config.get_primary_color())),
    );

    frame.render_widget(key_notes_footer, area);
}







