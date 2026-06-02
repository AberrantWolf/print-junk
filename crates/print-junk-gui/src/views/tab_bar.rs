//! The top tab strip that switches between feature modes.
//!
//! Driven by [`Mode::ALL`] so the tabs stay in sync with the startup selector.

use eframe::egui;

use crate::startup::Mode;

/// Height of a tab button. Larger than a default button so the strip reads as
/// a row of tabs rather than a toolbar.
const TAB_HEIGHT: f32 = 34.0;

/// Render the mode tabs into the current horizontal layout, mutating `mode` when
/// a tab is clicked.
pub fn show_tabs(ui: &mut egui::Ui, mode: &mut Mode) {
    for (m, icon, label) in Mode::ALL {
        if tab_button(ui, icon, label, *mode == m).clicked() {
            *mode = m;
        }
    }
}

/// A single tab: a wide, top-rounded button that fills with the accent colour
/// when selected so it reads as the active tab joined to the content below.
fn tab_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    let visuals = ui.visuals();
    let text_color = if selected {
        visuals.strong_text_color()
    } else {
        visuals.weak_text_color()
    };
    let text = egui::RichText::new(format!("{icon}  {label}"))
        .size(15.0)
        .color(text_color);

    // Rounded only on the top so the active tab visually connects to the panel.
    let corner = egui::CornerRadius {
        nw: 8,
        ne: 8,
        sw: 0,
        se: 0,
    };

    let mut button = egui::Button::new(text)
        .min_size(egui::vec2(0.0, TAB_HEIGHT))
        .corner_radius(corner);

    if selected {
        button = button.fill(visuals.selection.bg_fill);
    } else {
        button = button.fill(egui::Color32::TRANSPARENT);
    }

    ui.add(button)
}
