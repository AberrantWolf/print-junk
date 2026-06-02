mod actions_section;
mod additional_section;
mod binding_section;
mod cascade_section;
mod input_section;
mod margins_section;
mod marks_section;
mod output_section;
mod state;
mod statistics_section;

pub use state::ImposeState;

use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

pub fn show_impose(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::Panel::left("impose_controls")
        .min_size(300.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("PDF Imposition");
                ui.separator();

                input_section::show(ui, state, command_tx);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                binding_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                output_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                cascade_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                margins_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                marks_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                additional_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                statistics_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                actions_section::show(ui, state, command_tx);
            });
        });

    show_preview_area(ui, state, command_tx);
}

fn show_preview_area(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let overlay = match (
        state.preview_signatures_shown,
        state.preview_total_signatures,
    ) {
        (Some(shown), Some(total)) if shown < total => {
            Some(format!("Preview: showing {shown} of {total} signatures"))
        }
        _ => None,
    };
    let has_files = !state.options.input_files.is_empty();

    super::preview::show_preview_pane(ui, &mut state.preview_viewer, command_tx, overlay, |ui| {
        if has_files {
            ui.heading("Ready to Generate");
            ui.label("Click 'Generate Preview' to see the imposed layout");
        } else {
            ui.heading("No Input Files");
            ui.label("Add PDF files to begin");
        }
    });
}
