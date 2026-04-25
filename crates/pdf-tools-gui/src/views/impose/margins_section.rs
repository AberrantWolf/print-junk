use eframe::egui;
use pdf_impose::CreepConfig;

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

            // Creep compensation (only for signature-based bindings)
            if state.options.binding_type.uses_signatures() {
                ui.add_space(8.0);
                changed |= show_creep_section(ui, state);
            }

            if changed {
                state.needs_regeneration = true;
            }
        });
}

fn show_creep_section(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    let mut changed = false;

    ui.label("Creep compensation:").on_hover_text(
        "Compensates for paper thickness in folded signatures. \
             Inner sheets protrude at the fore edge; creep shifts \
             their content toward the spine so margins stay even after trimming.",
    );

    ui.indent("creep_config", |ui| {
        // Mode selector
        ui.horizontal(|ui| {
            if ui
                .selectable_label(matches!(state.options.creep, CreepConfig::None), "None")
                .clicked()
            {
                state.options.creep = CreepConfig::None;
                changed = true;
            }

            if ui
                .selectable_label(
                    matches!(state.options.creep, CreepConfig::PerLayer { .. }),
                    "Per layer",
                )
                .on_hover_text("Fixed offset per nested leaf layer")
                .clicked()
            {
                state.options.creep = CreepConfig::PerLayer {
                    creep_per_layer_mm: 0.1,
                };
                changed = true;
            }

            if ui
                .selectable_label(
                    matches!(state.options.creep, CreepConfig::FromCaliper { .. }),
                    "From caliper",
                )
                .on_hover_text(
                    "Computed from paper caliper using fold geometry \
                     (e.g., 80gsm ≈ 0.10 mm, 120gsm ≈ 0.14 mm)",
                )
                .clicked()
            {
                state.options.creep = CreepConfig::FromCaliper {
                    paper_thickness_mm: 0.1,
                };
                changed = true;
            }
        });

        // Mode-specific controls
        match &mut state.options.creep {
            CreepConfig::None => {}
            CreepConfig::PerLayer { creep_per_layer_mm } => {
                changed |= ui
                    .horizontal(|ui| {
                        ui.label("Per layer:");
                        ui.add(
                            egui::DragValue::new(creep_per_layer_mm)
                                .range(0.01..=2.0)
                                .speed(0.01)
                                .suffix(" mm"),
                        )
                        .changed()
                    })
                    .inner;
            }
            CreepConfig::FromCaliper { paper_thickness_mm } => {
                changed |= ui
                    .horizontal(|ui| {
                        ui.label("Paper caliper:");
                        ui.add(
                            egui::DragValue::new(paper_thickness_mm)
                                .range(0.01..=0.5)
                                .speed(0.01)
                                .suffix(" mm"),
                        )
                        .changed()
                    })
                    .inner;
            }
        }

        // Info line: max creep offset and spine margin check
        if state.options.creep.is_enabled() {
            let max_creep = pdf_impose::max_creep_offset_mm(
                state.options.creep,
                state.options.page_arrangement,
                state.options.sheets_per_signature,
            );
            let spine_mm = state.options.margins.leaf.spine_mm;

            ui.add_space(4.0);
            ui.label(format!("Max creep offset: {max_creep:.2} mm"));

            if max_creep > spine_mm {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 50),
                    format!(
                        "Spine margin ({spine_mm:.1} mm) is less than max creep ({max_creep:.2} mm)"
                    ),
                );

                if ui
                    .button(format!("Set spine margin to {max_creep:.2} mm"))
                    .on_hover_text(
                        "Increase the spine (gutter) margin so it absorbs the maximum \
                         creep shift. Without this, the innermost leaves' content may \
                         cross the spine fold.",
                    )
                    .clicked()
                {
                    state.options.margins.leaf.spine_mm = max_creep;
                    changed = true;
                }
            }
        }
    });

    changed
}
