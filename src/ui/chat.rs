use presage::proto::data_message::Quote;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::{
    app::{App, CurrentScreen, RecipientId},
    messages::receive::MessageDto,
    ui::{
        input::render_input_and_attachment,
        utils::{get_local_timestamp, render_scrollbar},
    },
};

#[derive(Debug)]
enum Visibility {
    Full,         // fully visible
    Partial(u16), // only last N lines visible
}

// Renders the chat window and input box in the right chunk of the screen
pub fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let recipient = &app.recipients[app.selected_recipient].0;
    let messages = match recipient.id() {
        RecipientId::Contact(uuid) => app.contact_messages.get(&uuid.to_string()),
        RecipientId::Group(master_key) => app.group_messages.get(&master_key),
    };
    let messages = match messages {
        Some(msgs) => msgs,
        None => &vec![],
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
            calculate_message_dimensions(messages, max_width, msg_padding, min_width, app);
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

        let content_lenght = messages.len();
        let scrollbar_position = content_lenght - app.message_selected;

        render_scrollbar(
            frame,
            scrollbar_position,
            content_lenght,
            vertical_chunks[0],
        );
    }

    render_input_and_attachment(frame, app, &vertical_chunks);
}

fn calculate_message_dimensions(
    messages: &[MessageDto],
    max_width: usize,
    msg_padding: u16,
    min_width: usize,
    app: &App,
) -> (Vec<u16>, Vec<u16>) {
    let mut heights = Vec::new();
    let mut widths = Vec::new();

    for msg in messages {
        let mut total_lines = 0;
        let mut longest_line_len = 0;

        if let Some(quote) = &msg.quote {
            let (quote_lines, quote_width) = calculate_quote_block(app, quote, max_width);
            total_lines += quote_lines;
            longest_line_len = longest_line_len.max(quote_width);
        }

        for line in msg.text.split('\n') {
            let len = line.chars().count();
            longest_line_len = longest_line_len.max(len);
            total_lines += (len / max_width + 1) as u16;
        }

        let message_height = total_lines + 3;

        let actual_width = longest_line_len
            .min(max_width)
            .saturating_add((msg_padding * 2 + 2) as usize)
            .max(min_width) as u16;

        heights.push(message_height);
        widths.push(actual_width);
    }

    (heights, widths)
}

fn calculate_quote_block(app: &App, quote: &Quote, max_width: usize) -> (u16, usize) {
    let author_name = get_display_name(app, quote.author_aci());
    let quote_time = get_local_timestamp(quote.id())
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let quoted_preview = quote
        .text
        .clone()
        .unwrap_or_else(|| "...".to_string())
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect::<String>();

    let info_line = format!("┆ {author_name} · {quote_time}");
    let max_line_len = info_line
        .chars()
        .count()
        .max(quoted_preview.chars().count());

    let quote_lines =
        (1 + (quoted_preview.len() / max_width + 1) + (info_line.len() / max_width + 1)) as u16;
    (quote_lines, max_line_len.min(max_width))
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

        let mut style = Style::default();
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        if let CurrentScreen::InspectMesseges = app.current_screen
            && idx == app.message_selected
        {
            style = style.add_modifier(Modifier::REVERSED);
        }

        if let Some(quoted) = &app.quoted_message
            && quoted.timestamp == msg.timestamp
        {
            style = style.add_modifier(Modifier::REVERSED);
        }

        let text_content = build_message_content(app, msg);

        let reactions_display = if !msg.reactions.is_empty() {
            let joined = msg.reactions.iter()
                .filter_map(|(_uuid,r)| r.emoji.clone())
                .collect::<Vec<_>>()
                .join(" ");
            Some(joined)
        } else {
            None
        };

        if let Some(reaction_text) = reactions_display {
            block = block.title_bottom(reaction_text);
        }

        let para: Paragraph = match visibility {
            Visibility::Full => Paragraph::new(text_content)
                .style(style)
                .block(
                    block
                        .clone()
                        .title_top(datetime_local.format("%Y-%m-%d %H:%M:%S").to_string())
                        .title_bottom(get_display_name(app, msg.uuid.to_string().as_str())),
                )
                .wrap(Wrap { trim: false }),

            Visibility::Partial(remaining_height) => {
                let max_text_lines = remaining_height.saturating_sub(1);
                let visible_lines: Vec<String> = text_content
                    .lines()
                    .rev()
                    .take(max_text_lines as usize)
                    .map(|s| s.to_string())
                    .collect();

                let visible_text = visible_lines
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                height = *remaining_height;

                Paragraph::new(visible_text)
                    .style(style)
                    .block(block.borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM))
                    .wrap(Wrap { trim: false })
            }
        };

        let mut x_pos = vertical_chunks[0].x;
        // let id = app.recipients[app.selected_recipient].0.id();
        let uuid_string = app.uuid.unwrap_or_default().to_string();
        if uuid_string == msg.uuid.to_string() {
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

fn build_message_content(app: &App, msg: &MessageDto) -> String {
    let mut text_content = String::new();

    if let Some(quote) = &msg.quote {
        text_content.push_str(&render_quote_block(app, quote));
        text_content.push('\n');
    }

    text_content.push_str(&msg.text);
    text_content
}

fn render_quote_block(app: &App, quote: &Quote) -> String {
    let author_name = get_display_name(app, quote.author_aci());
    let quote_time = get_local_timestamp(quote.id())
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let quoted_preview = quote
        .text
        .clone()
        .unwrap_or_else(|| "...".to_string())
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect::<String>();

    format!(
        "┆ {author_name} · {quote_time}\n┆ {}\n",
        quoted_preview.trim()
    )
}

fn get_display_name(app: &App, author_aci: &str) -> String {
    for (recipient, _) in &app.recipients {
        if let RecipientId::Contact(uuid) = recipient.id()
            && uuid.to_string() == author_aci
        {
            return recipient.display_name().to_string();
        }
    }
    "Unknown".to_string()
}
