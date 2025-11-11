use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::UiStatusMessage,
    ui::{
        captcha::render_captcha,
        group_info::{render_group_info, render_member_info},
        render_account_creation, render_account_selector,
    },
};
use crate::{
    app::{App, CurrentScreen, LinkingStatus, NetworkStatus, RecipientId},
    ui::{
        chat::render_chat,
        contact_list::{render_contact_info_compact, render_contact_list},
        linking::{render_qrcode, render_textarea},
        options::render_options,
        utils::{render_paragraph, render_popup},
    },
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
            render_options(frame, app);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::Exiting => {
            let status_message =
                UiStatusMessage::Info("Would you like to quit? \n (y/n)".to_string());
            render_popup(frame, frame.area(), &status_message);
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
            LinkingStatus::Error(ref error_msg) => {
                use crate::ui::linking::render_linking_error;
                render_linking_error(
                    frame,
                    frame.area(),
                    error_msg,
                    app.creating_account_name.is_some(),
                );
            }
        },
        CurrentScreen::Syncing => {
            let status_message =
                UiStatusMessage::Info("Syncing contacts and messages...".to_string());
            render_popup(frame, frame.area(), &status_message);
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
        CurrentScreen::GroupInfo => {
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Ratio(1, 4),
                    Constraint::Ratio(2, 4),
                    Constraint::Ratio(1, 4),
                ])
                .split(chunks[0]);

            render_contact_list(frame, app, horizontal_chunks[0]);
            render_group_info(frame, app, horizontal_chunks[1]);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::MemberInfo => {
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Ratio(1, 4),
                    Constraint::Ratio(2, 4),
                    Constraint::Ratio(1, 4),
                ])
                .split(chunks[0]);

            render_contact_list(frame, app, horizontal_chunks[0]);
            render_group_info(frame, app, horizontal_chunks[1]);
            render_member_info(frame, app, horizontal_chunks[2]);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::AccountSelector => {
            render_account_selector(frame, app, frame.area());
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::CreatingAccount => {
            render_account_creation(frame, app, frame.area());
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::ConfirmDelete => {
            if let Some(account_name) = &app.deleting_account {
                let text = format!(
                    "Are you sure you want to delete account '{account_name}'?\n\
            This action cannot be undone!\n\n\
            All messages and data for this account will be lost.\n\n\
            Press 'y' to confirm deletion\n\
            Press 'n' or ESC to cancel",
                );
                let status_message = UiStatusMessage::Info(text);
                render_popup(frame, frame.area(), &status_message);
            }
        }
        CurrentScreen::InspectMesseges => {
            render_contact_list(frame, app, main_chunks[0]);
            render_chat(frame, app, main_chunks[1]);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::Popup => {
            let status_message = match app.ui_status_info.clone() {
                Some(message) => message.status_message,
                None => UiStatusMessage::Info("".to_string()),
            };
            render_popup(frame, frame.area(), &status_message);
        }
        CurrentScreen::Recaptcha => {
            render_captcha(frame, frame.area(), app);
        }
    }
}

/// Renders the footer section at the bottom of the screen.
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let current_keys_hint = match app.current_screen {
        CurrentScreen::Main => Span::styled(
            "(q) to quit | (↑ ↓) to navigate | (→) to select chat | (i) for contact info | (a) for accounts panel | (e) for options",
            Style::default().fg(app.config.get_primary_color()),
        ),
        CurrentScreen::Writing => {
            let retry_info = if let Ok(manager) = app.retry_manager.try_lock() {
                let failed_count = manager.failed_count();

                if failed_count > 0 {
                    format!(" | {failed_count} failed msgs")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let base_text = if app.attachment_error.is_some() {
                "(q) to exit | Fix attachment path to send | (CTRL+t) to switch input/attachment | (TAB) autocomplete path | (CTRL+e) to inspect messages"
            } else {
                "(q) to exit | (ENTER) to send | (CTRL+t) to switch input/attachment | (TAB) autocomplete path | (CTRL+e) to inspect messages"
            };

            let mut reply_info = "";

            if app.quoted_message.is_some() {
                reply_info = " | (CTRL+r) to stop replying"
            }

            Span::styled(
                format!("{base_text}{reply_info}{retry_info}"),
                if app.attachment_error.is_some() {
                    Style::default().fg(app.config.get_error_color())
                } else {
                    Style::default().fg(app.config.get_primary_color())
                },
            )
        }
        CurrentScreen::Options => Span::styled(
            "(q) to exit | (↑ ↓) to navigate | (ENTER/SPACE) to toggle option",
            Style::default().fg(app.config.get_primary_color()),
        ),
        CurrentScreen::ContactInfo => Span::styled(
            "(q) to exit | (← or ESC) to go back",
            Style::default().fg(app.config.get_primary_color()),
        ),
        CurrentScreen::GroupInfo => Span::styled(
            "(q) to exit | (← or ESC) to go back",
            Style::default().fg(app.config.get_primary_color()),
        ),
        CurrentScreen::InspectMesseges => {
            let selected_recipient_id = app.recipients[app.selected_recipient].0.id();
            let optional_messages = match selected_recipient_id {
                RecipientId::Contact(uuid) => app.contact_messages.get(&uuid.to_string()),
                RecipientId::Group(group_key) => app.group_messages.get(&group_key),
            };

            let mut save_attachment_info = "";
            if let Some(messages) = optional_messages
                && let Some(Some(_att)) = messages
                    .get(app.message_selected)
                    .map(|msg| msg.attachment.as_ref())
            {
                save_attachment_info = " | (s) to save attachment"
            }

            Span::styled(
                format!(
                    "(q) to exit inspection mode | (← or ESC) to go back | (r) to reply | (d) to delete{save_attachment_info}"
                ),
                Style::default().fg(app.config.get_primary_color()),
            )
        }
        _ => Span::default(),
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
