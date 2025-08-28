use anyhow::Result;
use mime_guess::mime::APPLICATION_OCTET_STREAM;
use presage::libsignal_service::sender::AttachmentSpec;
use std::fs;
use std::path::PathBuf;

pub mod contact;
pub mod group;

/// Create attachment spec from file path
async fn create_attachment(attachment_path: String) -> Result<(AttachmentSpec, Vec<u8>)> {
    // Resolve absolute path
    let path: PathBuf = fs::canonicalize(&attachment_path)
        .map_err(|_| anyhow::anyhow!("Failed to resolve path: {}", attachment_path))?;

    if !path.exists() {
        return Err(anyhow::anyhow!(
            "Attachment file not found: {}",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(anyhow::anyhow!(
            "Attachment path is not a file: {}",
            path.display()
        ));
    }

    let file_data = fs::read(&path)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file name for path: {}", path.display()))?
        .to_string_lossy()
        .to_string();

    let attachment_spec = AttachmentSpec {
        content_type: mime_guess::from_path(&path)
            .first()
            .unwrap_or(APPLICATION_OCTET_STREAM)
            .to_string(),
        length: file_data.len(),
        file_name: Some(file_name),
        preview: None,
        voice_note: None,
        borderless: None,
        width: None,
        height: None,
        caption: None,
        blur_hash: None,
    };

    Ok((attachment_spec, file_data))
}
