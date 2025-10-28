use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, ListItem, Paragraph, Wrap},
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::App;

pub fn render_group_info(frame: &mut Frame, app: &mut App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15), Constraint::Min(1)])
        .split(area);
    let info_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(layout[1]);

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
        .title("Group Info")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.config.get_primary_color()));

    let members_block = Block::default()
        .title("Group Members")
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

    let contacts_list: Vec<ListItem> = match &app.selected_group_info {
        Some(group_info) => group_info
            .members
            .iter()
            .enumerate()
            .map(|(idx, member)| {
                let mut style = Style::default().fg(app.config.get_primary_color());
                if idx == app.selected_group_member {
                    style = style
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::UNDERLINED)
                        .fg(app.config.get_accent_color());
                }
                let display_name = match &member.name {
                    Some(name) => name.clone(),
                    None => match &member.phone_number {
                        Some(phone) => phone.clone(),
                        None => member.uuid.to_string(),
                    },
                };
                ListItem::new(display_name).style(style)
            })
            .collect(),
        None => vec![],
    };

    if app.config.show_images {
        frame.render_widget(info_paragraph, info_layout[0]);
    } else {
        frame.render_widget(info_paragraph, area);
    }
}
