//! Shared live-preview pane used by every mode that renders a PDF preview.
//!
//! Each mode owns an `Option<ViewerState>`; this pane shows it through the common
//! [`show_viewer`](super::show_viewer) widget — so scroll/page/zoom preservation
//! across re-renders is shared, not re-implemented per mode — and falls back to a
//! centered placeholder when there is nothing to preview yet.

use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

use super::{ViewerState, show_viewer};

/// Render the central preview area for a mode.
///
/// - `viewer`: the mode's preview viewer; when `Some`, it is shown via [`show_viewer`].
/// - `overlay`: an optional status line drawn above the preview (e.g. "showing N of M").
/// - `placeholder`: content shown (centered) when there is no preview yet.
pub fn show_preview_pane(
    ui: &mut egui::Ui,
    viewer: &mut Option<ViewerState>,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
    overlay: Option<String>,
    placeholder: impl FnOnce(&mut egui::Ui),
) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        if viewer.is_some() {
            if let Some(text) = overlay {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(140, 180, 255), text);
                });
            }
            show_viewer(ui, viewer, command_tx);
        } else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(placeholder);
            });
        }
    });
}
