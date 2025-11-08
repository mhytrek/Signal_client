use std::{
    fs::{self},
    path::Path,
};

use qrcode::QrCode;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};
use tui_qrcode::{Colors, QrCodeWidget};

use crate::{
    app::{App, UiStatusMessage},
    paths::{self},
    ui::utils::{centered_rect_fixed_size, render_popup},
};

/// Renders 50x50 QRCode if it exists in the QRCODE path
pub fn render_qrcode(frame: &mut Frame, area: Rect) {
    if Path::new(&paths::qrcode()).exists() {
        if area.width >= 50 && area.height >= 25 {
            let url = fs::read_to_string(paths::qrcode()).expect("failed to read from file");
            let qr_code = QrCode::new(url).expect("Failed to generate QRcode");
            let widget = QrCodeWidget::new(qr_code).colors(Colors::Inverted);
            let qr_area = centered_rect_fixed_size(50, 25, area);
            frame.render_widget(widget, qr_area);
        } else {
            let text = format!(
                "Terminal too small to show QRcode.\nMinimum window size 50x25 \n Current window size {}x{}",
                area.width, area.height
            );
            let status_message = UiStatusMessage::Info(text);
            render_popup(frame, area, &status_message);
        }
    } else {
        let text = "Generating QR Code...".to_string();
        let status_message = UiStatusMessage::Info(text);
        render_popup(frame, area, &status_message);
    }
}

/// Renders a textarea for device name
pub fn render_textarea(frame: &mut Frame, app: &App, area: Rect) {
    let input_area = Block::default()
        .title("Input device name")
        .borders(Borders::ALL);
    let input_text = Paragraph::new(app.textarea.clone()).block(input_area);
    let area = centered_rect_fixed_size(50, 5, area);

    frame.render_widget(input_text, area);
}

pub fn render_linking_error(
    frame: &mut Frame,
    area: Rect,
    error_msg: &str,
    is_account_creation: bool,
) {
    let retry_instruction = if is_account_creation {
        "Press ESC to go back to account selector\nPress any other key to retry"
    } else {
        "Press ESC to cancel\nPress any other key to retry"
    };

    let text = format!(
        "Error: {error_msg}\n\n{retry_instruction}\n\nNote: If your phone shows successful connection,\ntry waiting a moment and check your accounts list."
    );
    let status_message = UiStatusMessage::Error(text);

    render_popup(frame, area, &status_message);
}
