use eframe::egui;

use super::state::ImposeState;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("✂ Printer's Marks")
        .default_open(false)
        .show(ui, |ui| {
            let mut changed = false;

            changed |= ui
                .checkbox(&mut state.options.marks.fold_lines, "Fold lines")
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.cut_lines,
                    "Cut lines (with scissors)",
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.crop_marks,
                    "Crop marks (sheet edges)",
                )
                .changed();
            changed |= ui
                .checkbox(&mut state.options.marks.trim_marks, "Trim marks (per leaf)")
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.registration_marks,
                    "Registration marks",
                )
                .changed();

            ui.separator();

            changed |= ui
                .checkbox(
                    &mut state.options.marks.sewing_marks,
                    "Sewing station marks",
                )
                .changed();

            // Show sewing configuration when sewing marks are enabled
            if state.options.marks.sewing_marks {
                ui.indent("sewing_config", |ui| {
                    let mut stations = state.options.sewing_config.station_count as i32;
                    ui.horizontal(|ui| {
                        ui.label("Stations:");
                        ui.add(egui::DragValue::new(&mut stations).range(1..=10));
                    });
                    if stations.max(1) as usize != state.options.sewing_config.station_count {
                        state.options.sewing_config.station_count = stations.max(1) as usize;
                        changed = true;
                    }

                    let prev_offset = state.options.sewing_config.kettle_offset_mm;
                    ui.horizontal(|ui| {
                        ui.label("Kettle offset (mm):");
                        ui.add(
                            egui::DragValue::new(
                                &mut state.options.sewing_config.kettle_offset_mm,
                            )
                            .range(5.0..=30.0)
                            .speed(0.5),
                        );
                    });
                    if state.options.sewing_config.kettle_offset_mm != prev_offset {
                        changed = true;
                    }
                });
            }

            changed |= ui
                .checkbox(
                    &mut state.options.marks.collation_marks,
                    "Collation marks (back marks)",
                )
                .changed();

            if changed {
                state.needs_regeneration = true;
            }
        });
}
