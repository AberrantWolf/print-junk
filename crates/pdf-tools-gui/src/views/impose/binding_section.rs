use eframe::egui;
use pdf_impose::{BindingType, PageArrangement};

use super::state::ImposeState;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("📖 Binding & Arrangement")
        .default_open(true)
        .show(ui, |ui| {
            ui.label("Binding type:");
            if show_binding_type_selector(ui, &mut state.options.binding_type) {
                log::info!("Binding type changed to: {:?}", state.options.binding_type);
                state.needs_regeneration = true;
            }

            ui.add_space(5.0);

            if state.options.binding_type.uses_signatures() {
                if show_arrangement_selector(ui, &mut state.options.page_arrangement) {
                    state.needs_regeneration = true;
                }
            }
        });
}

fn show_binding_type_selector(ui: &mut egui::Ui, binding_type: &mut BindingType) -> bool {
    let mut changed = false;
    let options: &[(BindingType, &str, &str)] = &[
        (BindingType::Signature, "Signature", "Folded and sewn signatures, the traditional bookbinding method"),
        (BindingType::PerfectBinding, "Perfect", "Pages glued directly to the spine"),
        (BindingType::SideStitch, "Side Stitch", "Pages stapled or sewn through the side near the spine"),
        (BindingType::Spiral, "Spiral", "Pages bound with a spiral or comb through punched holes"),
        (BindingType::CaseBinding, "Case", "Signature binding with a rigid cover (hardcover)"),
    ];
    ui.horizontal(|ui| {
        for (value, label, tooltip) in options {
            if ui
                .selectable_value(binding_type, value.clone(), *label)
                .on_hover_text(*tooltip)
                .changed()
            {
                changed = true;
            }
        }
    });
    changed
}

fn show_arrangement_selector(ui: &mut egui::Ui, arrangement: &mut PageArrangement) -> bool {
    let mut changed = false;

    ui.label("Page arrangement:");
    ui.horizontal(|ui| {
        let arrangements: &[(PageArrangement, &str, &str)] = &[
            (PageArrangement::Folio, "Folio (4pp)", "1 fold, 4 pages per signature (2 leaves)"),
            (PageArrangement::Quarto, "Quarto (8pp)", "2 folds, 8 pages per signature (4 leaves)"),
            (PageArrangement::Octavo, "Octavo (16pp)", "3 folds, 16 pages per signature (8 leaves)"),
        ];
        for (value, label, tooltip) in arrangements {
            if ui
                .selectable_value(arrangement, *value, *label)
                .on_hover_text(*tooltip)
                .changed()
            {
                changed = true;
            }
        }

        // Custom as inline selectable label (matches on variant, not inner value)
        let is_custom = matches!(arrangement, PageArrangement::Custom { .. });
        if ui.selectable_label(is_custom, "Custom")
            .on_hover_text("Specify a custom number of pages per signature")
            .clicked() && !is_custom {
            *arrangement = PageArrangement::Custom {
                pages_per_signature: 12,
            };
            changed = true;
        }
    });

    // Show pages-per-signature editor when Custom is active
    if let PageArrangement::Custom {
        pages_per_signature,
    } = arrangement
    {
        ui.horizontal(|ui| {
            ui.label("Pages per signature:");
            let mut val = *pages_per_signature as i32;
            if ui
                .add(egui::DragValue::new(&mut val).range(4..=256).speed(4.0))
                .changed()
            {
                // Snap to nearest multiple of 4
                let snapped = ((val + 2) / 4 * 4).clamp(4, 256);
                *pages_per_signature = snapped as usize;
                changed = true;
            }
        });
    }

    changed
}
