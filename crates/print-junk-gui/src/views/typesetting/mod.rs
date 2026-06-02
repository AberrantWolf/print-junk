//! Typesetting mode: lay out a text file (Plaintext / Markdown / HTML) into a
//! typeset PDF, with a live preview. Desktop-only (Typst is a native dependency).

use std::path::PathBuf;

use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_typeset::{
    BreakPosition, InputFormat, PageBreakRule, TypesetConfig, TypesetInput, available_font_families,
};
use pdf_units::Orientation;
use tokio::sync::mpsc;

use super::ViewerState;
use crate::ui_components::{enum_selector, labeled_drag_clamped, paper_size_picker};

/// UI state for the typesetting mode.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TypesettingState {
    pub source_path: Option<PathBuf>,
    /// Document content (re-loaded from `source_path`; not persisted).
    #[serde(skip)]
    pub source_text: String,
    pub format: InputFormat,
    pub config: TypesetConfig,

    /// Installed font families, for the font pickers (re-enumerated; not persisted).
    #[serde(skip)]
    pub available_fonts: Vec<String>,

    #[serde(skip)]
    pub preview_viewer: Option<ViewerState>,
    #[serde(skip)]
    pub needs_regeneration: bool,
    #[serde(skip)]
    pub preview_page_count: usize,
}

impl Default for TypesettingState {
    fn default() -> Self {
        Self {
            source_path: None,
            source_text: String::new(),
            format: InputFormat::Markdown,
            config: TypesetConfig::default(),
            available_fonts: available_font_families(),
            preview_viewer: None,
            needs_regeneration: false,
            preview_page_count: 0,
        }
    }
}

impl TypesettingState {
    fn to_input(&self) -> TypesetInput {
        TypesetInput {
            text: self.source_text.clone(),
            format: self.format,
        }
    }

    /// Prompt for a source file and load it, detecting the format from the
    /// extension. Shared by the "Open..." button and the Cmd+O shortcut.
    pub fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Text documents",
                &["md", "markdown", "txt", "text", "html", "htm"],
            )
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    if let Some(fmt) = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .and_then(InputFormat::from_extension)
                    {
                        self.format = fmt;
                    }
                    self.source_text = text;
                    self.source_path = Some(path);
                    self.needs_regeneration = true;
                }
                Err(e) => log::error!("Failed to read {}: {e}", path.display()),
            }
        }
    }
}

pub fn show_typesetting(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::Panel::left("typesetting_controls")
        .min_size(320.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Typesetting");
                ui.separator();

                source_section(ui, state);
                section_gap(ui);
                page_section(ui, state);
                section_gap(ui);
                margins_section(ui, state);
                section_gap(ui);
                fonts_section(ui, state);
                section_gap(ui);
                spacing_section(ui, state);
                section_gap(ui);
                page_breaks_section(ui, state);
                section_gap(ui);
                actions_section(ui, state, command_tx);

                // Regenerate the preview once per frame, AFTER every section has
                // had a chance to flag a change. This must run last — if it ran
                // earlier, changes made by later sections (fonts, margins, …)
                // would be missed until some future frame.
                maybe_regenerate(state, command_tx);
            });
        });

    let page_count = state.preview_page_count;
    let has_text = !state.source_text.trim().is_empty();
    let overlay = (page_count > 0).then(|| format!("Preview: {page_count} page(s)"));
    super::preview::show_preview_pane(ui, &mut state.preview_viewer, command_tx, overlay, |ui| {
        if has_text {
            ui.heading("Ready to Typeset");
            ui.label("Adjust settings to update the preview");
        } else {
            ui.heading("No Document");
            ui.label("Open a text, Markdown, or HTML file to begin");
        }
    });
}

fn section_gap(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(4.0);
}

// =============================================================================
// Sections
// =============================================================================

fn source_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Source document:");
    ui.horizontal(|ui| {
        let name = state
            .source_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| "(untitled)".to_string(), |n| n.to_string_lossy().into());
        ui.label(name);
        if ui.button("Open...").clicked() {
            state.open_file_dialog();
        }
    });

    let formats = [
        (InputFormat::Markdown, "Markdown"),
        (InputFormat::Plaintext, "Plain text"),
        (InputFormat::Html, "HTML"),
    ];
    if enum_selector(ui, "ts_format", "Format:", &mut state.format, &formats) {
        state.needs_regeneration = true;
    }

    ui.add_space(4.0);
    ui.label("Content (editable):");
    let edited = ui
        .add(
            egui::TextEdit::multiline(&mut state.source_text)
                .desired_rows(6)
                .desired_width(f32::INFINITY)
                .code_editor(),
        )
        .changed();
    if edited {
        state.needs_regeneration = true;
    }
}

/// Send a fresh preview request if any setting changed this frame. Call once,
/// after all sections, so no section's change is missed.
fn maybe_regenerate(state: &mut TypesettingState, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    if state.needs_regeneration && !state.source_text.trim().is_empty() {
        state.needs_regeneration = false;
        let _ = command_tx.send(PdfCommand::TypesetGeneratePreview {
            input: state.to_input(),
            config: state.config.clone(),
        });
    }
}

fn page_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let mut changed = false;
    changed |= paper_size_picker(ui, "ts_paper", "Page size:", &mut state.config.page_size);

    let orientations = [
        (Orientation::Portrait, "Portrait"),
        (Orientation::Landscape, "Landscape"),
    ];
    changed |= enum_selector(
        ui,
        "ts_orientation",
        "Orientation:",
        &mut state.config.orientation,
        &orientations,
    );
    changed |= ui
        .checkbox(&mut state.config.page_numbers, "Page numbers")
        .changed();

    if changed {
        state.needs_regeneration = true;
    }
}

fn margins_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Margins:");
    let m = &mut state.config;
    let mut changed = false;
    changed |= labeled_drag_clamped(ui, "Top", &mut m.margin_top_mm, 0.0..=80.0, " mm");
    changed |= labeled_drag_clamped(ui, "Bottom", &mut m.margin_bottom_mm, 0.0..=80.0, " mm");
    changed |= labeled_drag_clamped(
        ui,
        "Inner (spine)",
        &mut m.margin_inner_mm,
        0.0..=80.0,
        " mm",
    );
    changed |= labeled_drag_clamped(ui, "Outer (fore-edge)", &mut m.margin_outer_mm, 0.0..=80.0, " mm");
    if changed {
        state.needs_regeneration = true;
    }
}

fn fonts_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Fonts:");
    let mut changed = false;
    changed |= font_family_picker(ui, "ts_body_font", "Body", &mut state.config.body_font.family, &state.available_fonts);
    changed |= labeled_drag_clamped(ui, "Body size", &mut state.config.body_font.size_pt, 5.0..=32.0, " pt");
    changed |= font_family_picker(
        ui,
        "ts_heading_font",
        "Heading",
        &mut state.config.heading_font.family,
        &state.available_fonts,
    );
    changed |= labeled_drag_clamped(ui, "Heading size", &mut state.config.heading_font.size_pt, 6.0..=48.0, " pt");
    if changed {
        state.needs_regeneration = true;
    }
}

/// A combo box over installed font families, with a leading "(default)" entry
/// that maps to an empty family (engine picks a serif).
fn font_family_picker(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    family: &mut String,
    available: &[String],
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let selected = if family.is_empty() {
            "(default)".to_string()
        } else {
            family.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(family.is_empty(), "(default)")
                    .clicked()
                    && !family.is_empty()
                {
                    family.clear();
                    changed = true;
                }
                for fam in available {
                    if ui
                        .selectable_label(family == fam, fam)
                        .clicked()
                        && family != fam
                    {
                        family.clone_from(fam);
                        changed = true;
                    }
                }
            });
    });
    changed
}

fn spacing_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Spacing:");
    let c = &mut state.config;
    let mut changed = false;
    changed |= labeled_drag_clamped(ui, "Line leading", &mut c.line_spacing_em, 0.0..=2.0, " em");
    changed |= labeled_drag_clamped(ui, "Paragraph gap", &mut c.paragraph_spacing_mm, 0.0..=20.0, " mm");
    changed |= labeled_drag_clamped(ui, "First-line indent", &mut c.paragraph_indent_mm, 0.0..=30.0, " mm");
    changed |= ui.checkbox(&mut c.justify, "Justify text").changed();
    changed |= ui.checkbox(&mut c.hyphenate, "Hyphenate").changed();
    if changed {
        state.needs_regeneration = true;
    }
}

fn page_breaks_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Page-break rules:");
    ui.label(
        egui::RichText::new("Insert a page break at lines matching a pattern.")
            .small()
            .weak(),
    );

    let mut changed = false;
    let mut remove: Option<usize> = None;
    let positions = [
        (BreakPosition::After, "after"),
        (BreakPosition::Before, "before"),
        (BreakPosition::Replace, "replace"),
    ];

    for (i, rule) in state.config.page_breaks.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut rule.pattern)
                        .desired_width(90.0)
                        .hint_text("e.g. -----"),
                )
                .changed();
            changed |= enum_selector(ui, &format!("ts_break_{i}"), "", &mut rule.position, &positions);
            if ui.button("✖").clicked() {
                remove = Some(i);
            }
        });
    }

    if let Some(i) = remove {
        state.config.page_breaks.remove(i);
        changed = true;
    }
    if ui.button("➕ Add rule").clicked() {
        state
            .config
            .page_breaks
            .push(PageBreakRule::new("", BreakPosition::Replace));
        changed = true;
    }

    if changed {
        state.needs_regeneration = true;
    }
}

fn actions_section(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let has_text = !state.source_text.trim().is_empty();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(has_text, egui::Button::new("💾 Save PDF…"))
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name("typeset.pdf")
                .save_file()
        {
            let _ = command_tx.send(PdfCommand::TypesetGenerate {
                input: state.to_input(),
                config: state.config.clone(),
                output_path: path,
            });
        }

        if ui
            .add_enabled(has_text, egui::Button::new("📑 Send to Impose"))
            .on_hover_text("Typeset and load the result into the imposition mode")
            .clicked()
        {
            let _ = command_tx.send(PdfCommand::TypesetSendToImpose {
                input: state.to_input(),
                config: state.config.clone(),
            });
        }
    });
}
