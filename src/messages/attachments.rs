use mime_guess::mime::APPLICATION_OCTET_STREAM;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Local;
use presage::libsignal_service::sender::AttachmentSpec;
use presage::manager::{Manager, Registered};
use presage::proto::AttachmentPointer;
use presage_store_sqlite::SqliteStore;
use tracing::info;

// save attachment to given directory
pub async fn save_attachment(
    attachment_pointer: AttachmentPointer,
    manager: Manager<SqliteStore, Registered>,
    save_dir: PathBuf,
) -> Result<PathBuf> {
    let attachment_data = match manager.get_attachment(&attachment_pointer).await {
        Ok(data) => data,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to get attachment data from database: {}",
                e
            ));
        }
    };

    if attachment_data.is_empty() {
        return Err(anyhow::anyhow!("Attachment data is empty"));
    }

    let base_name = attachment_pointer.file_name.clone().unwrap_or_else(|| {
        let extension = mime_guess::get_mime_extensions_str(attachment_pointer.content_type())
            .and_then(|exts| exts.first().map(|e| e.to_string()))
            .unwrap_or_else(|| "bin".to_string());
        let local_date = Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
        format!("{local_date}.{extension}")
    });

    let file_path = get_unique_file_path(&save_dir, &base_name);

    fs::write(&file_path, &attachment_data).map_err(|e| {
        anyhow::anyhow!(
            "Failed to save the attachment to {} : {}",
            file_path.display(),
            e
        )
    })?;

    info!("Saved attachment to : {}", file_path.display());
    Ok(file_path)
}

// Takes the file name and search for the files with the same name in given directory.
// It returns the unique file path by inreasing the counter at the end of the file name
fn get_unique_file_path(dir: &Path, file_name: &str) -> PathBuf {
    let mut candidate = dir.join(file_name);

    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());

    let mut counter = 1;
    loop {
        let new_name = match ext {
            Some(ext) => format!("{stem}-{counter}.{ext}"),
            None => format!("{stem}-{counter}"),
        };
        candidate = dir.join(new_name);
        if !candidate.exists() {
            break candidate;
        }
        counter += 1;
    }
}

/// Create attachment spec from file path
pub async fn create_attachment(attachment_path: String) -> Result<(AttachmentSpec, Vec<u8>)> {
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
