use ratatui::layout::Alignment;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
};
use ratatui_image::{Resize, StatefulImage};

use crate::{app::App, ui::utils::render_scrollbar};

/// Renders the contact list in the left chunk of the screen
pub fn render_contact_list(frame: &mut Frame, app: &App, area: Rect) {
    let list_items: Vec<ListItem> = app
        .recipients
        .iter()
        .enumerate()
        .map(|(i, (recipient, _))| {
            let mut style = Style::default().fg(app.config.get_primary_color());
            if i == app.selected_recipient {
                style = style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED)
                    .fg(app.config.get_accent_color());
            }
            let name = recipient.display_name().to_string();
            ListItem::new(name).style(style)
        })
        .collect();

    let chat_list_widget = List::new(list_items.clone()).block(
        Block::default()
            .padding(Padding::new(1, 1, 1, 1))
            .title("Chats")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.config.get_primary_color())),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_recipient));

    frame.render_stateful_widget(chat_list_widget, area, &mut list_state);
    render_scrollbar(frame, app.selected_recipient, list_items.len(), area);
}

/// Renders contact information screen
pub fn render_contact_info_compact(frame: &mut Frame, app: &mut App, area: Rect) {
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
            "\nNAME:\n{}\n\nPHONE:\n{}\n\nABOUT:\n{}\n\nVERIFIED:\n{}\n\nAVATAR:\n{}\n",
            contact.name,
            contact.phone_number.as_deref().unwrap_or("Not set"),
            contact.description.as_deref().unwrap_or(""),
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
