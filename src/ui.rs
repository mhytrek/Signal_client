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
    paths::QRCODE,
};

/// Main UI rendering function.
pub fn ui(frame: &mut Frame, app: &mut App) {
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

/// Renders the contact list in the left chunk of the screen
fn render_contact_list(frame: &mut Frame, app: &App, area: Rect) {
    let list_items: Vec<ListItem> = app
        .contacts
        .iter()
        .enumerate()
        .map(|(i, (_, name, _id))| {
            let mut style = Style::default().fg(app.config.get_primary_color());
            if i == app.contact_selected {
                style = style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED)
                    .fg(app.config.get_accent_color());
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
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.config.get_primary_color())),
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

// Renders the chat window and input box in the right chunk of the screen
fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let messages = match app
        .contact_messages
        .get(&app.contacts[app.contact_selected].0)
    {
        Some(msgs) => msgs,
        None => &Vec::<MessageDto>::new(),
    };

    if !messages.is_empty() {
        let msg_padding = 2;
        let margin = vertical_chunks[0].width.saturating_div(4);
        let available_height = vertical_chunks[0].height;
        let max_width = vertical_chunks[0]
            .width
            .saturating_sub(msg_padding * 2 + 2 + margin) as usize;
        let min_width = 21; // hardcoded date format width

        let (heights, widths) =
            calculate_message_dimensions(messages, max_width, msg_padding, min_width);
        let last_visible_start = calculate_last_visible_start(&heights, available_height);
        let start_index = app.message_selected.min(last_visible_start);
        let visible_msgs = get_visible_messages(
            &heights,
            available_height,
            start_index,
            vertical_chunks[0].y,
        );

        render_messages(
            frame,
            messages,
            &visible_msgs,
            &heights,
            &widths,
            &vertical_chunks,
            app,
        );

        render_scrollbar(frame, app, messages.len(), &vertical_chunks);
    }

    render_input_and_attachment(frame, app, &vertical_chunks);
}

fn render_input_and_attachment(frame: &mut Frame, app: &App, vertical_chunks: &[Rect]) {
    let input_area_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)])
        .split(vertical_chunks[1]);

    let input_window = Paragraph::new(app.contacts[app.contact_selected].2.clone())
        .block(Block::default().title("Input").borders(Borders::ALL));

    let attachment_title = match &app.attachment_error {
        Some(error) => format!("Attachment Path - ERROR: {error}"),
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

    frame.render_widget(input_window, input_area_chunks[0]);
    frame.render_widget(attachment_window, input_area_chunks[1]);

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

fn render_scrollbar(frame: &mut Frame, app: &App, total_messages: usize, vertical_chunks: &[Rect]) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

    let mut scrollbar_state =
        ScrollbarState::new(total_messages).position(total_messages - app.message_selected);

    frame.render_stateful_widget(
        scrollbar,
        vertical_chunks[0].inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
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

    let styled_text = Text::styled(text, Style::default().fg(Color::default()));

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
                "(q) to quit | (↑ ↓) to navigate | (→) to select chat | (i) for contact info | (e) for options",
                Style::default().fg(app.config.get_primary_color()),
            ),
            CurrentScreen::Writing => {
                let retry_info = if let Ok(manager) = app.retry_manager.try_lock() {
                    let failed_count = manager.get_failed_count();

                    if failed_count > 0 {
                        format!(" | {} failed msgs", failed_count)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let base_text = if app.attachment_error.is_some() {
                    "(q) to exit | (ENTER) to send | (TAB) to switch input/attachment | Fix attachment path to send"
                } else {
                    "(q) to exit | (ENTER) to send | (TAB) to switch input/attachment"
                };

                Span::styled(
                    format!("{}{}", base_text, retry_info),
                    if app.attachment_error.is_some() {
                        Style::default().fg(app.config.get_error_color())
                    } else {
                        Style::default().fg(app.config.get_primary_color())
                    }
                )
            },
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
            let text = format!(
                "Terminal too small to show QRcode.\nMinimum window size 50x25 \n Current window size {}x{}",
                area.width, area.height
            );
            render_popup(frame, area, &text);
        }
    } else {
        let text = "Generating QR Code...";
        render_popup(frame, area, text);
    }
}

/// renders avatar image
fn render_avatar(frame: &mut Frame, app: &mut App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15), Constraint::Min(1)])
        .split(area);

    let avatar_block = Block::default()
        .title("Avatar")
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let avatar_area = avatar_block.inner(layout[0]);

    let centered_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Length(30),
            Constraint::Percentage(5),
        ])
        .split(avatar_area);
    let centered_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Length(30),
            Constraint::Percentage(5),
        ])
        .split(centered_layout[1]);

    if app.config.show_images {
        if let Some(avatar_image) = app.avatar_image.as_mut() {
            frame.render_stateful_widget(
                StatefulImage::new().resize(Resize::Fit(None)),
                centered_area[1],
                avatar_image,
            );
        } else {
            let placeholder_text = if app.avatar_cache.is_some() {
                "Loading avatar..."
            } else {
                "No avatar set"
            };

            let placeholder = Paragraph::new(placeholder_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Gray));

            frame.render_widget(placeholder, centered_area[1]);
        }
        frame.render_widget(avatar_block, area);
    }

    let profile_block = Block::default()
        .title("Profile Data")
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let mut profile_text = String::new();
    if let Some(profile) = &app.profile {
        profile_text.push_str(&format!(
            "\n NAME:            {}\n\
          \n ABOUT:           {}\n\
          \n EMOJI:           {}\n\
          \n AVATAR:          {}\n\n",
            profile
                .name
                .as_ref()
                .map_or("Not set", |n| n.given_name.as_str()),
            profile.about.as_ref().map_or("Not set", String::as_str),
            profile
                .about_emoji
                .as_ref()
                .map_or("Not set", String::as_str),
            if app.avatar_cache.is_some() {
                "Set"
            } else {
                "Not set"
            },
        ));
    } else {
        profile_text.push_str("Profile data not loaded...");
    }

    let profile_paragraph = Paragraph::new(profile_text)
        .block(profile_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::default()));

    if app.config.show_images {
        frame.render_widget(profile_paragraph, layout[1]);
    } else {
        frame.render_widget(profile_paragraph, area);
    }
}

/// Renders the enhanced options screen with improved layout
fn render_options(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(40), Constraint::Min(1)])
        .split(centered_rect(90, 80, frame.area()));

    if app.avatar_cache.is_some() && app.avatar_image.is_none() {
        app.load_avatar();
    }
    render_avatar(frame, app, main_layout[0]);

    // Configuration options panel
    let config_block = Block::default()
        .title("Configuration")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.config.get_primary_color()));

    let config_options = [
        format!(
            "Color Mode: {}",
            if app.config.color_mode {
                "Colorful"
            } else {
                "Black & White"
            }
        ),
        format!(
            "Show Images: {}",
            if app.config.show_images {
                "Enabled"
            } else {
                "Disabled"
            }
        ),
    ];

    let config_items: Vec<ListItem> = config_options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            let mut style = Style::default().fg(app.config.get_primary_color());
            if i == app.config_selected {
                style = style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(app.config.get_accent_color());
            }
            ListItem::new(option.clone()).style(style)
        })
        .collect();

    let config_list = List::new(config_items)
        .block(config_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ListState::default();
    list_state.select(Some(app.config_selected));

    frame.render_stateful_widget(config_list, main_layout[1], &mut list_state);
}

/// Renders contact information screen
fn render_contact_info_compact(frame: &mut Frame, app: &mut App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15), Constraint::Min(1)])
        .split(area);

    let avatar_block = Block::default()
        .title("Avatar")
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let avatar_area = avatar_block.inner(layout[0]);

    let centered_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Length(30),
            Constraint::Percentage(5),
        ])
        .split(avatar_area);
    let centered_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Length(30),
            Constraint::Percentage(5),
        ])
        .split(centered_layout[1]);

    if app.config.show_images {
        if let Some(contact_avatar) = app.contact_avatar_image.as_mut() {
            frame.render_stateful_widget(
                StatefulImage::new().resize(Resize::Fit(None)),
                centered_area[1],
                contact_avatar,
            );
        } else {
            let placeholder_text = if app.contact_avatar_cache.is_some() {
                "Loading..."
            } else if app
                .selected_contact_info
                .as_ref()
                .is_some_and(|c| c.has_avatar)
            {
                "Avatar available but not loaded"
            } else {
                "No avatar"
            };

            let placeholder = Paragraph::new(placeholder_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Gray));

            frame.render_widget(placeholder, avatar_area);
        }
        frame.render_widget(avatar_block, layout[0]);
    }

    let info_block = Block::default()
        .title("Contact Info")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.config.get_primary_color()));

    let mut info_text = String::new();

    if let Some(contact) = &app.selected_contact_info {
        info_text.push_str(&format!(
            "\nNAME:\n{}\n\nPHONE:\n{}\n\nVERIFIED:\n{}\n\nAVATAR:\n{}\n",
            contact.name,
            contact.phone_number.as_deref().unwrap_or("Not set"),
            match contact.verified_state {
                Some(state) if state > 0 => "Yes",
                _ => "No",
            },
            if contact.has_avatar { "Set" } else { "Not set" },
        ));
    } else {
        info_text.push_str("Loading...");
    }

    let info_paragraph = Paragraph::new(info_text)
        .block(info_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left)
        .style(Style::default().fg(app.config.get_primary_color()));

    if app.config.show_images {
        frame.render_widget(info_paragraph, layout[1]);
    } else {
        frame.render_widget(info_paragraph, area);
    }
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

// Takes a timestamp and transforms it to Date in a local timezone
fn get_local_timestamp(millis: u64) -> DateTime<Local> {
    let secs = (millis / 1000) as i64;
    let datetime_utc: DateTime<Utc> = DateTime::from_timestamp(secs, 0).expect("Invalid timestamp");

    datetime_utc.with_timezone(&Local)
}

fn calculate_message_dimensions(
    messages: &[MessageDto],
    max_width: usize,
    msg_padding: u16,
    min_width: usize,
) -> (Vec<u16>, Vec<u16>) {
    let mut heights = Vec::new();
    let mut widths = Vec::new();

    for msg in messages {
        let lines = msg.text.split('\n');
        let mut total_lines = 0;
        let mut longest_line_len = 0;

        for line in lines {
            let len = line.chars().count();
            longest_line_len = longest_line_len.max(len);
            total_lines += (len / max_width + 1) as u16;
        }

        heights.push(total_lines + 2);

        let actual_width = longest_line_len
            .min(max_width)
            .saturating_add((msg_padding * 2 + 2) as usize)
            .max(min_width) as u16;

        widths.push(actual_width);
    }

    (heights, widths)
}

// calculates which message would be the last visible
fn calculate_last_visible_start(heights: &[u16], available_height: u16) -> usize {
    let mut used_height = 0;
    let mut last_visible_start = 0;

    for (idx, &h) in heights.iter().enumerate().rev() {
        if used_height + h > available_height {
            break;
        }
        last_visible_start = idx;
        used_height += h;
    }

    last_visible_start
}

#[derive(Debug)]
enum Visibility {
    Full,         // fully visible
    Partial(u16), // only last N lines visible
}

fn get_visible_messages(
    heights: &[u16],
    available_height: u16,
    start_index: usize,
    y_start: u16,
) -> Vec<(usize, Visibility)> {
    let mut y_cursor = y_start;
    let mut visible_msgs = Vec::new();

    for (idx, &h) in heights.iter().enumerate().skip(start_index) {
        if y_cursor >= available_height {
            break;
        } else if y_cursor + h > available_height {
            let remaining = available_height.saturating_sub(y_cursor);
            visible_msgs.push((idx, Visibility::Partial(remaining)));
            break;
        } else {
            visible_msgs.push((idx, Visibility::Full));
            y_cursor += h;
        }
    }

    visible_msgs.reverse();
    visible_msgs
}

fn render_messages(
    frame: &mut Frame,
    messages: &[MessageDto],
    visible_msgs: &[(usize, Visibility)],
    heights: &[u16],
    widths: &[u16],
    vertical_chunks: &[Rect],
    app: &App,
) {
    let mut y_pos = 0;

    for &(idx, ref visibility) in visible_msgs {
        let msg = &messages[idx];
        let datetime_local = get_local_timestamp(msg.timestamp);
        let width = widths[idx];
        let mut height = heights[idx];
        let para: Paragraph;

        match visibility {
            Visibility::Full => {
                para = Paragraph::new(msg.text.clone())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title(datetime_local.format("%Y-%m-%d %H:%M:%S").to_string()),
                    )
                    .wrap(Wrap { trim: false });
            }
            Visibility::Partial(remaining_height) => {
                let max_text_lines = remaining_height.saturating_sub(1);
                let mut buffer = Vec::new();
                let mut used_lines = 0;

                for line in msg.text.lines().rev() {
                    let mut current_line = line;
                    while !current_line.is_empty() {
                        let take = current_line
                            .chars()
                            .take(width as usize - 4)
                            .collect::<String>();

                        if used_lines >= max_text_lines {
                            break;
                        }

                        buffer.push(take.clone());
                        used_lines += 1;
                        current_line = &current_line[take.len()..];
                    }
                    if used_lines >= max_text_lines {
                        break;
                    }
                }

                buffer.reverse();
                let visible_text = buffer.join("\n");

                height = *remaining_height;
                para = Paragraph::new(visible_text)
                    .block(
                        Block::default()
                            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                            .border_type(BorderType::Rounded),
                    )
                    .wrap(Wrap { trim: false });
            }
        }

        let mut x_pos = vertical_chunks[0].x;
        if app.contacts[app.contact_selected].0 != msg.uuid.to_string() {
            x_pos = vertical_chunks[0].x + vertical_chunks[0].width - width;
        }

        let msg_area = Rect {
            x: x_pos,
            y: y_pos,
            width,
            height,
        };
        frame.render_widget(para, msg_area);

        y_pos += height;
    }
}
