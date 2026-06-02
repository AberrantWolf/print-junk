use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_flashcards::MeasurementSystem;
use pdf_units::PaperSize;
use tokio::sync::mpsc;

use super::ViewerState;
use crate::ui_components::{
    MarginsEditor, SliderBuilder, SpacingEditor, enum_selector, paper_size_picker,
};

mod flashcard_layout;
use flashcard_layout::{FlashcardLayout, MaxValueType, convert_values, get_max_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SizingMode {
    Grid,     // Specify rows/columns, card size is calculated
    CardSize, // Specify card size, rows/columns are calculated
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FlashcardState {
    pub csv_path: String,
    pub paper_size: PaperSize,
    pub measurement_system: MeasurementSystem,
    pub sizing_mode: SizingMode,

    // Margins in current measurement system
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,

    // Card dimensions in current measurement system
    pub card_width: f32,
    pub card_height: f32,

    // Grid layout
    pub rows: usize,
    pub columns: usize,

    // Spacing in current measurement system
    pub row_spacing: f32,
    pub column_spacing: f32,

    pub font_size_pt: f32,

    // Loaded flashcards (re-loaded from `csv_path`; not persisted).
    #[serde(skip)]
    pub cards: Vec<pdf_flashcards::Flashcard>,

    // Preview state (transient).
    #[serde(skip)]
    pub preview_viewer: Option<ViewerState>,

    // Track if we need to regenerate (transient).
    #[serde(skip)]
    pub needs_regeneration: bool,
}

impl Default for FlashcardState {
    fn default() -> Self {
        let measurement_system = MeasurementSystem::Inches;
        Self {
            csv_path: String::new(),
            paper_size: PaperSize::Letter,
            measurement_system,
            sizing_mode: SizingMode::Grid,
            margin_top: 0.4,
            margin_bottom: 0.4,
            margin_left: 0.4,
            margin_right: 0.4,
            card_width: 2.5,
            card_height: 3.5,
            rows: 2,
            columns: 3,
            row_spacing: 0.2,
            column_spacing: 0.2,
            font_size_pt: 12.0,
            cards: Vec::new(),
            preview_viewer: None,
            needs_regeneration: false,
        }
    }
}

impl FlashcardState {
    pub fn to_options(&self) -> pdf_flashcards::FlashcardOptions {
        let (page_width_mm, page_height_mm) = self.paper_size.dimensions_mm();
        pdf_flashcards::FlashcardOptions {
            page_width_mm,
            page_height_mm,
            margin_top_mm: self.measurement_system.to_mm(self.margin_top),
            margin_bottom_mm: self.measurement_system.to_mm(self.margin_bottom),
            margin_left_mm: self.measurement_system.to_mm(self.margin_left),
            margin_right_mm: self.measurement_system.to_mm(self.margin_right),
            card_width_mm: self.measurement_system.to_mm(self.card_width),
            card_height_mm: self.measurement_system.to_mm(self.card_height),
            rows: self.rows,
            columns: self.columns,
            row_spacing_mm: self.measurement_system.to_mm(self.row_spacing),
            column_spacing_mm: self.measurement_system.to_mm(self.column_spacing),
            font_size_pt: self.font_size_pt,
        }
    }

    pub fn convert_all_values(&mut self, old_system: MeasurementSystem) {
        convert_values(
            &mut [
                &mut self.margin_top,
                &mut self.margin_bottom,
                &mut self.margin_left,
                &mut self.margin_right,
                &mut self.card_width,
                &mut self.card_height,
                &mut self.row_spacing,
                &mut self.column_spacing,
            ],
            old_system,
            self.measurement_system,
        );
    }

    pub fn recalculate_grid_from_card_size(&mut self) {
        let layout = self.to_layout();
        (self.rows, self.columns) = layout.calculate_grid_from_card_size();
    }

    pub fn recalculate_card_size_from_grid(&mut self) {
        let layout = self.to_layout();
        (self.card_width, self.card_height) = layout.calculate_card_size_from_grid();
    }

    fn to_layout(&self) -> FlashcardLayout {
        FlashcardLayout {
            paper_size: self.paper_size,
            measurement_system: self.measurement_system,
            margin_top: self.margin_top,
            margin_bottom: self.margin_bottom,
            margin_left: self.margin_left,
            margin_right: self.margin_right,
            card_width: self.card_width,
            card_height: self.card_height,
            rows: self.rows,
            columns: self.columns,
            row_spacing: self.row_spacing,
            column_spacing: self.column_spacing,
        }
    }
}

pub fn show_flashcards(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::Panel::left("flashcard_controls")
        .min_size(300.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Flashcard Settings");
                ui.separator();

                show_csv_section(ui, state, command_tx);
                ui.add_space(10.0);
                ui.separator();

                show_paper_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_margins_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_sizing_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_spacing_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_font_section(ui, state);
                ui.add_space(20.0);
                ui.separator();

                show_actions_section(ui, state, command_tx);
            });
        });

    show_preview_area(ui, state, command_tx);
}

fn show_csv_section(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.label("CSV File:");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.csv_path);
        if ui.button("Browse...").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
        {
            state.csv_path = path.display().to_string();
            log::info!("Loading CSV: {}", path.display());
            let _ = command_tx.send(PdfCommand::FlashcardsLoadCsv { input_path: path });
        }
    });

    if !state.cards.is_empty() {
        ui.label(format!("Loaded: {} cards", state.cards.len()));
    }
}

fn show_paper_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    if paper_size_picker(ui, "paper_size", "Paper Type:", &mut state.paper_size) {
        state.needs_regeneration = true;
    }

    ui.add_space(10.0);

    let measurement_systems = [
        (MeasurementSystem::Inches, "Inches (in)"),
        (MeasurementSystem::Millimeters, "Millimeters (mm)"),
        (MeasurementSystem::Points, "Points (pt)"),
    ];

    let old_system = state.measurement_system;
    enum_selector(
        ui,
        "measurement_system",
        "Measurement System:",
        &mut state.measurement_system,
        &measurement_systems,
    );

    if old_system != state.measurement_system {
        state.convert_all_values(old_system);
    }
}

fn show_margins_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Page Margins:");
    let max = get_max_value(MaxValueType::Margin, state.measurement_system);
    let unit = state.measurement_system.name();

    if MarginsEditor::new(
        &mut state.margin_top,
        &mut state.margin_bottom,
        &mut state.margin_left,
        &mut state.margin_right,
        max,
        unit,
    )
    .show(ui)
    {
        state.needs_regeneration = true;
    }
}

fn show_sizing_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Sizing Mode:");
    egui::ComboBox::from_id_salt("sizing_mode")
        .selected_text(match state.sizing_mode {
            SizingMode::Grid => "Specify Grid (rows/columns)",
            SizingMode::CardSize => "Specify Card Size",
        })
        .show_ui(ui, |ui| {
            if ui
                .selectable_value(
                    &mut state.sizing_mode,
                    SizingMode::Grid,
                    "Specify Grid (rows/columns)",
                )
                .changed()
            {
                state.recalculate_card_size_from_grid();
                state.needs_regeneration = true;
            }
            if ui
                .selectable_value(
                    &mut state.sizing_mode,
                    SizingMode::CardSize,
                    "Specify Card Size",
                )
                .changed()
            {
                state.recalculate_grid_from_card_size();
                state.needs_regeneration = true;
            }
        });

    ui.add_space(10.0);
    ui.separator();

    // Grid Layout
    ui.label("Grid Layout:");
    ui.add_enabled_ui(state.sizing_mode == SizingMode::Grid, |ui| {
        let mut changed = false;
        changed |= SliderBuilder::new(&mut state.rows, 1..=10)
            .text("Rows")
            .show(ui);
        changed |= SliderBuilder::new(&mut state.columns, 1..=10)
            .text("Columns")
            .show(ui);

        if changed {
            state.recalculate_card_size_from_grid();
            state.needs_regeneration = true;
        }
    });

    ui.add_space(10.0);
    ui.separator();

    // Card Size
    ui.label("Card Size:");
    ui.add_enabled_ui(state.sizing_mode == SizingMode::CardSize, |ui| {
        let max = get_max_value(MaxValueType::CardSize, state.measurement_system);
        let unit = state.measurement_system.name();
        let mut changed = false;

        changed |= SliderBuilder::new(&mut state.card_width, 0.0..=max)
            .text(format!("Width ({unit})"))
            .show(ui);

        changed |= SliderBuilder::new(&mut state.card_height, 0.0..=max)
            .text(format!("Height ({unit})"))
            .show(ui);

        if changed {
            state.recalculate_grid_from_card_size();
            state.needs_regeneration = true;
        }
    });
}

fn show_spacing_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Spacing:");
    let max = get_max_value(MaxValueType::Spacing, state.measurement_system);
    let unit = state.measurement_system.name();

    if SpacingEditor::new(
        &mut state.column_spacing,
        &mut state.row_spacing,
        "Column Spacing",
        "Row Spacing",
        max,
        unit,
    )
    .show(ui)
    {
        state.needs_regeneration = true;
    }
}

fn show_font_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Font Size:");
    if SliderBuilder::new(&mut state.font_size_pt, 6.0..=36.0)
        .text("Size (pt)")
        .show(ui)
    {
        state.needs_regeneration = true;
    }
}

fn show_actions_section(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let can_generate = !state.cards.is_empty();

    #[cfg(not(target_arch = "wasm32"))]
    if ui
        .add_enabled(can_generate, egui::Button::new("💾 Save PDF..."))
        .clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name("flashcards.pdf")
            .save_file()
    {
        log::info!("Saving flashcards to: {}", path.display());
        let options = state.to_options();
        let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
            cards: state.cards.clone(),
            options,
            output_path: path,
        });
    }

    // Auto-regenerate preview when settings change
    if state.needs_regeneration && can_generate {
        generate_preview(state, command_tx);
    }
}

fn generate_preview(state: &mut FlashcardState, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    state.needs_regeneration = false;
    log::info!("Generating flashcard preview");
    let options = state.to_options();
    let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
        cards: state.cards.clone(),
        options,
        output_path: std::env::temp_dir().join("flashcards_preview.pdf"),
    });
}

fn show_preview_area(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let card_count = state.cards.len();
    super::preview::show_preview_pane(ui, &mut state.preview_viewer, command_tx, None, |ui| {
        if card_count == 0 {
            ui.heading("No CSV Loaded");
            ui.label("Select a CSV file to begin");
        } else {
            ui.heading("Ready to Generate");
            ui.label(format!("{card_count} flashcards loaded"));
            ui.label("Click 'Generate Preview' to see the result");
        }
    });
}
