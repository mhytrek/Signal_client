use std::{
    fs::{self},
    path::Path,
};

use chrono::{DateTime, Local, Utc};
use qrcode::QrCode;
use ratatui::layout::Alignment;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};
use tui_qrcode::{Colors, QrCodeWidget};

use crate::{
    app::{App, CurrentScreen, InputFocus, LinkingStatus, NetworkStatus},
    paths::QRCODE,
};

/// Main UI rendering function.
pub fn ui(frame: &mut Frame, app: &App) {
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
            render_chat_and_contact(frame, app, main_chunks[1]);
            render_footer(frame, app, chunks[1]);
        }
        CurrentScreen::Options => {
            render_options(frame, app);
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
                render_popup(frame, frame.area(), "Error linking device, check if you have Internet connection.\n PRESS ANY KEY TO RETRY");
            }
        },
        CurrentScreen::Syncing => {
            render_popup(frame, frame.area(), "Syncing contacts and messeges...");
        }
    }
}

/// Renders the contact list in the left chunk of the screen
fn render_contact_list(frame: &mut Frame, app: &App, area: Rect) {
    let list_items: Vec<ListItem> = app
        .contacts
        .iter()
        .enumerate()
        .map(|(i, (_, name, _id))| {
            let mut style = Style::default();
            if i == app.contact_selected {
                style = style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED);
            }
            ListItem::new(name.clone()).style(style)
        })
        .collect();

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

    let mut scrollbar_state = ScrollbarState::new(list_items.len()).position(app.contact_selected);

    let chat_list_widget = List::new(list_items).block(
        Block::default()
            .padding(Padding::new(1, 1, 1, 1))
            .title("Chats")
            .borders(Borders::ALL),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(app.contact_selected));

    frame.render_stateful_widget(chat_list_widget, area, &mut list_state);
    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

/// Renders the chat window and input box in the right chunk of the screen
fn render_chat_and_contact(frame: &mut Frame, app: &App, area: Rect) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let messages: Vec<ListItem> = match &app
        .contact_messages
        .get(&app.contacts[app.contact_selected].0)
    {
        Some(msgs) => msgs
            .iter()
            .map(|msg| {
                let mut style = Style::default();

                let millis = msg.timestamp;
                let secs = (millis / 1000) as i64;
                let datetime_utc: DateTime<Utc> =
                    DateTime::from_timestamp(secs, 0).expect("Invalid timestamp");

                let datetime_local = datetime_utc.with_timezone(&Local);

                let content = format!(
                    "[{}] {}",
                    datetime_local.format("%Y-%m-%d %H:%M:%S"),
                    msg.text
                );

                if app.contacts[app.contact_selected].0 != msg.uuid.to_string() {
                    style = style.add_modifier(Modifier::BOLD);
                    ListItem::new(
                        Line::from(format!(" {}", content))
                            .style(style)
                            .right_aligned(),
                    )
                } else {
                    ListItem::new(Line::from(format!("{} ", content)).style(style))
                }
            })
            .collect(),
        None => vec![],
    };

    let chat_window = List::new(messages.clone()).block(
        Block::default()
            .title(app.contacts[app.contact_selected].1.clone())
            .borders(Borders::ALL),
    );

    let input_area_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)])
        .split(vertical_chunks[1]);

    let input_window = Paragraph::new(app.contacts[app.contact_selected].2.clone())
        .block(Block::default().title("Input").borders(Borders::ALL));

    let attachment_title = match &app.attachment_error {
        Some(error) => format!("Attachment Path - ERROR: {}", error),
        None => "Attachment Path".to_string(),
    };

    let attachment_style = match &app.attachment_error {
        Some(_) => Style::default().fg(Color::Red),
        None => Style::default(),
    };

    let attachment_border_style = match &app.attachment_error {
        Some(_) => Style::default().fg(Color::Red),
        None => Style::default(),
    };

    let attachment_window =
        Paragraph::new(Text::styled(app.attachment_path.clone(), attachment_style)).block(
            Block::default()
                .title(attachment_title)
                .borders(Borders::ALL)
                .border_style(attachment_border_style),
        );

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

    let mut scrollbar_state = ScrollbarState::new(messages.len()).position(app.message_selected);

    let mut list_state = ListState::default();
    list_state.select(Some(app.message_selected));

    frame.render_stateful_widget(chat_window, vertical_chunks[0], &mut list_state);
    frame.render_widget(input_window, input_area_chunks[0]);
    frame.render_widget(attachment_window, input_area_chunks[1]);
    frame.render_stateful_widget(
        scrollbar,
        vertical_chunks[0].inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    if let CurrentScreen::Writing = app.current_screen {
        match app.input_focus {
            InputFocus::Message => {
                frame.set_cursor_position((
                    input_area_chunks[0].x + app.character_index as u16 + 1,
                    input_area_chunks[0].y + 1,
                ));
            }
            InputFocus::Attachment => {
                frame.set_cursor_position((
                    input_area_chunks[1].x + app.attachment_path.len() as u16 + 1,
                    input_area_chunks[1].y + 1,
                ));
            }
        }
    }
}

/// Renders a popup with given text
fn render_popup(frame: &mut Frame, area: Rect, text: &str) {
    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let styled_text = Text::styled(text, Style::default());

    let paragraph = Paragraph::new(styled_text)
        .block(popup_block)
        .centered()
        .wrap(Wrap { trim: false });

    let popup_area = centered_rect(60, 25, area);
    frame.render_widget(paragraph, popup_area);
}

/// Renders a paragraph with given text
fn render_paragraph(frame: &mut Frame, area: Rect, text: &str) {
    let block = Block::default().borders(Borders::ALL);

    let styled_text = Text::styled(text, Style::default().fg(Color::White));

    let paragraph = Paragraph::new(styled_text)
        .block(block)
        .centered()
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
/// Renders the footer section at the bottom of the screen.
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let current_keys_hint = {
        match app.current_screen {
            CurrentScreen::Main => Span::styled(
                "(q) to quit | (↑ ↓) to navigate | (→) to select chat | (e) to show more options",
                Style::default(),
            ),
            CurrentScreen::Writing => {
                if app.attachment_error.is_some() {
                    Span::styled(
                        "(q) to exit | (ENTER) to send | (TAB) to switch input/attachment | Fix attachment path to send",
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    Span::styled(
                        "(q) to exit | (ENTER) to send | (TAB) to switch input/attachment",
                        Style::default(),
                    )
                }
            }
            CurrentScreen::Options => Span::styled("(q) to exit | (e) to select", Style::default()),

            _ => Span::default(),
        }
    };

    let network_status = match &app.network_status {
        NetworkStatus::Connected => Span::styled("⚡ Online", Style::default().fg(Color::Green)),
        NetworkStatus::Disconnected(msg) => {
            Span::styled(format!("⚠ {}", msg), Style::default().fg(Color::Red))
        }
    };

    let footer_text = Line::from(vec![current_keys_hint, Span::raw(" | "), network_status]);

    let key_notes_footer =
        Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));

    frame.render_widget(key_notes_footer, area);
}

/// Renders a textarea for device name
fn render_textarea(frame: &mut Frame, app: &App, area: Rect) {
    let input_area = Block::default()
        .title("Input device name")
        .borders(Borders::ALL);
    let input_text = Paragraph::new(app.textarea.clone()).block(input_area);
    let area = centered_rect_fixed_size(50, 5, area);

    frame.render_widget(input_text, area);
}

/// Renders 50x50 QRCode if it exists in the QRCODE path
fn render_qrcode(frame: &mut Frame, area: Rect) {
    if Path::new(QRCODE).exists() {
        if area.width >= 50 && area.height >= 25 {
            let url = fs::read_to_string(QRCODE).expect("failed to read from file");
            let qr_code = QrCode::new(url).expect("Failed to generate QRcode");
            let widget = QrCodeWidget::new(qr_code).colors(Colors::Inverted);
            let qr_area = centered_rect_fixed_size(50, 25, area);
            frame.render_widget(widget, qr_area);
        } else {
            let text = format!("Terminal too small to show QRcode.\nMinimum window size 50x25 \n Current window size {}x{}", area.width, area.height);
            render_popup(frame, area, &text);
        }
    } else {
        let text = "Generating QR Code...";
        render_popup(frame, area, text);
    }
}

/// Renders the enhanced options screen with improved layout
fn render_options(frame: &mut Frame, app: &App) {
    let popup_block = Block::default()
        .title("Options")
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let mut options_text = String::new();

    options_text.push_str("PROFILE:\n");

    if let Some(profile) = &app.profile {
        options_text.push_str("  Name               : ");
        options_text.push_str(
            profile
                .name
                .as_ref()
                .map_or("Not set", |n| n.given_name.as_str()),
        );
        options_text.push('\n');

        options_text.push_str("  About              : ");
        options_text.push_str(profile.about.as_ref().map_or("Not set", String::as_str));
        options_text.push('\n');

        options_text.push_str("  Emoji              : ");
        options_text.push_str(
            profile
                .about_emoji
                .as_ref()
                .map_or("Not set", String::as_str),
        );
        options_text.push('\n');

        options_text.push_str("  Avatar             : ");
        options_text.push_str(if profile.avatar.is_some() {
            "Set"
        } else {
            "Not set"
        });
        options_text.push('\n');

        options_text.push_str("  Unrestricted Access: ");
        options_text.push_str(if profile.unrestricted_unidentified_access {
            "Enabled"
        } else {
            "Disabled"
        });
        options_text.push_str("\n\n");
    } else {
        options_text.push_str("  Profile data not loaded...\n\n");
    }

    let exit_paragraph = Paragraph::new(options_text)
        .block(popup_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    let area = centered_rect(60, 80, frame.area());

    frame.render_widget(exit_paragraph, area);
}

/// Creates a rectangular area centered within the given Rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Creates a rectangular area centered within the given Rect with fixed size
fn centered_rect_fixed_size(width: u16, height: u16, r: Rect) -> Rect {
    let rect_width = width.min(r.width);
    let rect_height = height.min(r.height);

    let x = r.x + (r.width - rect_width) / 2;
    let y = r.y + (r.height - rect_height) / 2;

    Rect {
        x,
        y,
        width: rect_width,
        height: rect_height,
    }
}
