use eframe::egui;

use super::state::ImposeState;
use crate::ui_components::{LeafMarginsEditor, SheetMarginsEditor};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("📏 Margins")
        .default_open(false)
        .show(ui, |ui| {
            let mut changed = false;

            ui.label("Sheet margins (printer-safe area):")
                .on_hover_text("Margins inside the printable area of the physical sheet");
            ui.indent("sheet_margins", |ui| {
                changed |= SheetMarginsEditor::new(
                    &mut state.options.margins.sheet.top_mm,
                    &mut state.options.margins.sheet.bottom_mm,
                    &mut state.options.margins.sheet.left_mm,
                    &mut state.options.margins.sheet.right_mm,
                    25.0,
                )
                .show(ui);
            });

            ui.add_space(8.0);

            ui.label("Leaf margins (trim & gutter):")
                .on_hover_text("Margins around each book page within its cell on the sheet");
            ui.indent("leaf_margins", |ui| {
                changed |= LeafMarginsEditor::new(
                    &mut state.options.margins.leaf.top_mm,
                    &mut state.options.margins.leaf.bottom_mm,
                    &mut state.options.margins.leaf.fore_edge_mm,
                    &mut state.options.margins.leaf.spine_mm,
                    &mut state.options.margins.leaf.trim_allowance_mm,
                    50.0,
                )
                .show(ui);
            });

            if changed {
                state.needs_regeneration = true;
            }
        });
}
