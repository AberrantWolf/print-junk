//! Worker-side handlers for the typesetting mode. Typst compilation is CPU-bound,
//! so it runs on a blocking task rather than the async worker thread.

use std::path::PathBuf;

use pdf_async_runtime::{PdfUpdate, TypesetConfig, TypesetInput};
use tokio::sync::mpsc;

fn count_pdf_pages(bytes: &[u8]) -> usize {
    lopdf::Document::load_mem(bytes).map_or(0, |doc| doc.get_pages().len())
}

fn send_error(update_tx: &mpsc::UnboundedSender<PdfUpdate>, message: String) {
    let _ = update_tx.send(PdfUpdate::Error { message });
}

pub async fn handle_generate_preview(
    input: TypesetInput,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match tokio::task::spawn_blocking(move || pdf_typeset::typeset(&input, &config)).await {
        Ok(Ok(pdf_bytes)) => {
            let page_count = count_pdf_pages(&pdf_bytes);
            let _ = update_tx.send(PdfUpdate::TypesetPreviewGenerated {
                pdf_bytes,
                page_count,
            });
        }
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

pub async fn handle_generate(
    input: TypesetInput,
    config: TypesetConfig,
    output_path: PathBuf,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match tokio::task::spawn_blocking(move || pdf_typeset::typeset(&input, &config)).await {
        Ok(Ok(pdf_bytes)) => match tokio::fs::write(&output_path, &pdf_bytes).await {
            Ok(()) => {
                let _ = update_tx.send(PdfUpdate::TypesetComplete { path: output_path });
            }
            Err(e) => send_error(update_tx, format!("Failed to write PDF: {e}")),
        },
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

/// Typeset to a unique temp PDF and signal the imposition mode to pick it up.
/// The imposition pipeline is path-based, so a temp file is the cleanest handoff.
pub async fn handle_send_to_impose(
    input: TypesetInput,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match tokio::task::spawn_blocking(move || pdf_typeset::typeset(&input, &config)).await {
        Ok(Ok(pdf_bytes)) => {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let path = std::env::temp_dir().join(format!("print-junk-typeset-{unique}.pdf"));
            match tokio::fs::write(&path, &pdf_bytes).await {
                Ok(()) => {
                    let _ = update_tx.send(PdfUpdate::TypesetReadyForImpose { path });
                }
                Err(e) => send_error(update_tx, format!("Failed to write PDF: {e}")),
            }
        }
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}
