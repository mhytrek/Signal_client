use ratatui::layout::Alignment;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::App;

use super::utils::centered_rect;

/// renders avatar image
pub fn render_avatar(frame: &mut Frame, app: &mut App, area: Rect) {
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
pub fn render_options(frame: &mut Frame, app: &mut App) {
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
