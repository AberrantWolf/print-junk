//! Typesetting mode: lay out a text file (Plaintext / Markdown / HTML) into a
//! typeset PDF, with a live preview. Desktop-only (Typst is a native dependency).

use std::path::PathBuf;

use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_typeset::{
    BreakPosition, Color, HAlign, InputFormat, PageBreakRule, TableBorder, TypesetConfig,
    TypesetInput, available_font_families,
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

    /// Which heading level (1..=6) the headings section is currently editing.
    #[serde(skip)]
    pub heading_edit_level: u8,

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
            heading_edit_level: 1,
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
                headings_section(ui, state);
                section_gap(ui);
                spacing_section(ui, state);
                section_gap(ui);
                tables_section(ui, state);
                section_gap(ui);
                page_breaks_section(ui, state);
                section_gap(ui);
                document_section(ui, state);
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
    ui.label("Body text:");
    let mut changed = false;
    changed |= font_family_picker(
        ui,
        "ts_body_font",
        "Font",
        &mut state.config.body_font.family,
        &state.available_fonts,
    );
    changed |= labeled_drag_clamped(
        ui,
        "Size",
        &mut state.config.body_font.size_pt,
        5.0..=32.0,
        " pt",
    );
    changed |= color_row(ui, "Color", &mut state.config.body_color);
    changed |= color_row(ui, "Link color", &mut state.config.link_color);
    changed |= color_row(ui, "Code color", &mut state.config.code_color);
    changed |= optional_color_row(
        ui,
        "Code background",
        &mut state.config.code_background,
        Color::new(244, 244, 244),
    );
    if changed {
        state.needs_regeneration = true;
    }
}

/// Per-level heading editor: pick a level, then edit its style. Only the
/// selected level's controls are shown to keep the panel compact.
fn headings_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Headings:");

    // Clamp first — a skipped/zeroed value on restore must not underflow below.
    state.heading_edit_level = state.heading_edit_level.clamp(1, 6);
    let mut changed = false;

    ui.horizontal(|ui| {
        for lvl in 1..=6u8 {
            if ui
                .selectable_label(state.heading_edit_level == lvl, format!("H{lvl}"))
                .clicked()
            {
                state.heading_edit_level = lvl;
            }
        }
    });

    let lvl = state.heading_edit_level;
    let available = &state.available_fonts;
    let st = &mut state.config.heading_styles[usize::from(lvl - 1)];

    changed |= font_family_picker(ui, "ts_heading_font", "Font", &mut st.family, available);
    changed |= labeled_drag_clamped(ui, "Size", &mut st.size_pt, 6.0..=72.0, " pt");
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut st.bold, "Bold").changed();
        changed |= ui.checkbox(&mut st.italic, "Italic").changed();
    });
    changed |= color_row(ui, "Color", &mut st.color);

    let aligns = [
        (HAlign::Left, "Left"),
        (HAlign::Center, "Center"),
        (HAlign::Right, "Right"),
    ];
    changed |= enum_selector(ui, "ts_heading_align", "Align", &mut st.align, &aligns);
    changed |= labeled_drag_clamped(ui, "Space above", &mut st.space_above_mm, 0.0..=40.0, " mm");
    changed |= labeled_drag_clamped(ui, "Space below", &mut st.space_below_mm, 0.0..=40.0, " mm");
    changed |= ui
        .checkbox(&mut st.start_new_page, "Start on new page (chapter)")
        .changed();

    if changed {
        state.needs_regeneration = true;
    }
}

fn tables_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Tables:");
    let t = &mut state.config.table;
    let mut changed = false;

    changed |= ui.checkbox(&mut t.header_bold, "Bold header row").changed();
    changed |= optional_color_row(ui, "Header fill", &mut t.header_fill, Color::new(230, 230, 230));
    changed |= optional_color_row(ui, "Zebra striping", &mut t.zebra_fill, Color::new(244, 244, 244));

    let borders = [
        (TableBorder::All, "All"),
        (TableBorder::Horizontal, "Horizontal"),
        (TableBorder::None, "None"),
    ];
    changed |= enum_selector(ui, "ts_table_border", "Borders", &mut t.border, &borders);
    changed |= labeled_drag_clamped(ui, "Border width", &mut t.border_width_pt, 0.0..=3.0, " pt");
    changed |= color_row(ui, "Border color", &mut t.border_color);
    changed |= labeled_drag_clamped(ui, "Cell padding", &mut t.cell_padding_mm, 0.0..=8.0, " mm");

    if changed {
        state.needs_regeneration = true;
    }
}

fn document_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    ui.label("Document & front matter:");
    let c = &mut state.config;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label("Title");
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut c.doc_title)
                    .desired_width(180.0)
                    .hint_text("(none)"),
            )
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Author");
        changed |= ui
            .add(egui::TextEdit::singleline(&mut c.doc_author).desired_width(180.0))
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Keywords");
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut c.doc_keywords)
                    .desired_width(180.0)
                    .hint_text("comma, separated"),
            )
            .changed();
    });
    ui.label(
        egui::RichText::new("A title page is added when a title is set.")
            .small()
            .weak(),
    );

    changed |= ui
        .checkbox(&mut c.generate_toc, "Table of contents")
        .changed();
    if c.generate_toc {
        changed |= labeled_drag_clamped(ui, "TOC depth", &mut c.toc_depth, 1..=6, "");
    }
    changed |= ui
        .checkbox(&mut c.smart_punctuation, "Smart punctuation")
        .on_hover_text("Curly quotes and typographic dashes")
        .changed();
    ui.horizontal(|ui| {
        ui.label("Language");
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut c.lang)
                    .desired_width(60.0)
                    .hint_text("en"),
            )
            .on_hover_text("BCP-47 code; controls hyphenation and quotes")
            .changed();
    });

    if changed {
        state.needs_regeneration = true;
    }
}

/// A labeled sRGB color swatch button. Returns `true` if the color changed.
fn color_row(ui: &mut egui::Ui, label: &str, color: &mut Color) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut rgb = [color.r, color.g, color.b];
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            *color = Color::new(rgb[0], rgb[1], rgb[2]);
            true
        } else {
            false
        }
    })
    .inner
}

/// A checkbox that toggles an optional color on/off, with a swatch when enabled.
/// `default` seeds the color when first enabled.
fn optional_color_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<Color>,
    default: Color,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *value = enabled.then_some(default);
            changed = true;
        }
        if let Some(color) = value.as_mut() {
            let mut rgb = [color.r, color.g, color.b];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *color = Color::new(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }
        }
    });
    changed
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
