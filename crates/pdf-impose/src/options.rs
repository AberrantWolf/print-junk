use crate::constants::mm_to_pt;
use crate::layout::Rect;
use crate::layout::arrangement::{calculate_cut_edges, calculate_spread_positions};
use crate::layout::spread::calculate_spread_content;
use crate::types::{
    BindingType, CascadeConfig, CreepConfig, ImposeError, Margins, MarksAppearance, Orientation,
    OutputFormat, PageArrangement, PaperSize, PrinterMarks, Result, Rotation, ScalingMode,
    SewingConfig, SplitMode,
};
use std::path::PathBuf;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Comprehensive imposition configuration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImpositionOptions {
    // Input
    pub input_files: Vec<PathBuf>,

    // Binding and arrangement
    pub binding_type: BindingType,
    pub page_arrangement: PageArrangement,
    /// Number of sheets nested together per signature (default: 1)
    pub sheets_per_signature: usize,

    // Output configuration
    pub output_paper_size: PaperSize,
    pub output_orientation: Orientation,
    pub output_format: OutputFormat,
    pub scaling_mode: ScalingMode,

    // Margins
    pub margins: Margins,

    // Printer's marks
    pub marks: PrinterMarks,

    // Marks appearance
    /// Appearance for interior marks (fold lines, trim marks, sewing marks) —
    /// near trim/fold edges, potentially visible in the finished book.
    #[cfg_attr(feature = "serde", serde(default))]
    pub interior_marks_appearance: MarksAppearance,
    /// Appearance for exterior marks (crop marks, registration marks, collation marks,
    /// cascade cut lines) — at sheet edges, reliably trimmed or covered by binding.
    #[cfg_attr(feature = "serde", serde(default))]
    pub exterior_marks_appearance: MarksAppearance,

    // Sewing configuration (for sewing station marks)
    pub sewing_config: SewingConfig,

    // Page numbering
    pub add_page_numbers: bool,
    pub page_number_start: usize,

    // Flyleaves
    pub front_flyleaves: usize,
    pub back_flyleaves: usize,

    // Output splitting
    pub split_mode: SplitMode,

    // Rotation for source pages
    pub source_rotation: Rotation,

    // Cascade (tile multiple imposed sheets on a larger output page)
    pub cascade: Option<CascadeConfig>,

    // Creep (shingling) compensation for folded signatures
    #[cfg_attr(feature = "serde", serde(default))]
    pub creep: CreepConfig,
}

impl Default for ImpositionOptions {
    fn default() -> Self {
        Self {
            input_files: Vec::new(),
            binding_type: BindingType::Signature,
            page_arrangement: PageArrangement::Quarto,
            sheets_per_signature: 1,
            output_paper_size: PaperSize::Letter,
            output_orientation: Orientation::Landscape,
            output_format: OutputFormat::DoubleSided,
            scaling_mode: ScalingMode::Fit,
            margins: Margins::default(),
            marks: PrinterMarks::default(),
            interior_marks_appearance: MarksAppearance::default(),
            exterior_marks_appearance: MarksAppearance::default(),
            sewing_config: SewingConfig::default(),
            add_page_numbers: false,
            page_number_start: 1,
            front_flyleaves: 0,
            back_flyleaves: 0,
            split_mode: SplitMode::None,
            source_rotation: Rotation::None,
            cascade: None,
            creep: CreepConfig::default(),
        }
    }
}

impl ImpositionOptions {
    /// Total pages per signature (`pages_per_sheet` × `sheets_per_signature`)
    pub fn pages_per_signature(&self) -> usize {
        self.page_arrangement.pages_per_sheet() * self.sheets_per_signature
    }

    /// Load options from JSON file
    #[cfg(feature = "serde")]
    pub async fn load(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let bytes = tokio::fs::read(path).await?;
        let options = serde_json::from_slice(&bytes)
            .map_err(|e| ImposeError::Config(format!("Failed to parse config: {e}")))?;
        Ok(options)
    }

    /// Save options to JSON file
    #[cfg(feature = "serde")]
    pub async fn save(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ImposeError::Config(format!("Failed to serialize config: {e}")))?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    /// Raw paper dimensions in points (before cascade cell derivation)
    fn raw_paper_dimensions_pt(&self) -> (f32, f32) {
        let (width_mm, height_mm) = self
            .output_paper_size
            .dimensions_with_orientation(self.output_orientation);
        (mm_to_pt(width_mm), mm_to_pt(height_mm))
    }

    /// Calculate the cell dimensions when cascade is active.
    ///
    /// The cell size is derived by dividing the available cascade sheet area
    /// (after subtracting sheet margins and inter-cell gaps) by the grid dimensions.
    pub fn cell_dimensions_pt(&self) -> Option<(f32, f32)> {
        let cascade = self.cascade.as_ref()?;
        if cascade.is_trivial() {
            return None;
        }
        let (sheet_w, sheet_h) = self.raw_paper_dimensions_pt();
        let margins = &self.margins.sheet;
        let gap = mm_to_pt(cascade.margin_mm);

        let avail_w = sheet_w
            - mm_to_pt(margins.left_mm)
            - mm_to_pt(margins.right_mm)
            - gap * (cascade.cols as f32 - 1.0);
        let avail_h = sheet_h
            - mm_to_pt(margins.top_mm)
            - mm_to_pt(margins.bottom_mm)
            - gap * (cascade.rows as f32 - 1.0);

        Some((avail_w / cascade.cols as f32, avail_h / cascade.rows as f32))
    }

    /// Calculate the dimensions of a single imposed sheet (cell) in points.
    ///
    /// When cascade is active, returns the derived cell size.
    /// When cascade is inactive, returns the output paper dimensions.
    pub fn sheet_dimensions_pt(&self) -> (f32, f32) {
        self.cell_dimensions_pt()
            .unwrap_or_else(|| self.raw_paper_dimensions_pt())
    }

    /// Calculate the full output page dimensions in points.
    ///
    /// When cascade is active, returns the cascade sheet size (the large output page).
    /// When cascade is inactive, returns the same as `sheet_dimensions_pt()`.
    pub fn cascade_sheet_dimensions_pt(&self) -> (f32, f32) {
        if self.cascade.as_ref().is_some_and(|c| !c.is_trivial()) {
            self.raw_paper_dimensions_pt()
        } else {
            self.sheet_dimensions_pt()
        }
    }

    /// Calculate the leaf area bounds (inside sheet margins) in points.
    ///
    /// This is relative to the cell (imposed sheet), not the cascade output page.
    /// When cascade is inactive, the cell *is* the output page.
    pub fn leaf_bounds_pt(&self) -> Rect {
        let (width_pt, height_pt) = self.sheet_dimensions_pt();
        if self.cascade.as_ref().is_some_and(|c| !c.is_trivial()) {
            // In cascade mode, cell has no sheet margins — the cascade sheet has them.
            // The entire cell area is the leaf area.
            Rect::new(0.0, 0.0, width_pt, height_pt)
        } else {
            let margins = &self.margins.sheet;
            Rect::new(
                mm_to_pt(margins.left_mm),
                mm_to_pt(margins.bottom_mm),
                width_pt - mm_to_pt(margins.left_mm) - mm_to_pt(margins.right_mm),
                height_pt - mm_to_pt(margins.top_mm) - mm_to_pt(margins.bottom_mm),
            )
        }
    }

    /// Validate the options
    pub fn validate(&self) -> Result<()> {
        if self.input_files.is_empty() {
            return Err(ImposeError::Config("No input files specified".to_string()));
        }

        if self.sheets_per_signature == 0 {
            return Err(ImposeError::Config(
                "Sheets per signature must be at least 1".to_string(),
            ));
        }

        // Validate non-negative margins
        let sm = &self.margins.sheet;
        if sm.top_mm < 0.0 || sm.bottom_mm < 0.0 || sm.left_mm < 0.0 || sm.right_mm < 0.0 {
            return Err(ImposeError::Config(
                "Sheet margins must not be negative".to_string(),
            ));
        }
        let lm = &self.margins.leaf;
        if lm.top_mm < 0.0
            || lm.bottom_mm < 0.0
            || lm.fore_edge_mm < 0.0
            || lm.spine_mm < 0.0
            || lm.trim_allowance_mm < 0.0
        {
            return Err(ImposeError::Config(
                "Leaf margins and trim allowance must not be negative".to_string(),
            ));
        }

        // Validate marks appearance
        for (name, appearance) in [
            ("Interior", &self.interior_marks_appearance),
            ("Exterior", &self.exterior_marks_appearance),
        ] {
            if !(0.0..=1.0).contains(&appearance.gray) {
                return Err(ImposeError::Config(format!(
                    "{name} marks gray must be between 0.0 and 1.0"
                )));
            }
            if appearance.line_width_scale <= 0.0 {
                return Err(ImposeError::Config(format!(
                    "{name} marks line width scale must be positive"
                )));
            }
        }

        if self.sewing_config.kettle_offset_mm < 0.0 {
            return Err(ImposeError::Config(
                "Kettle stitch offset must not be negative".to_string(),
            ));
        }

        // Validate creep configuration
        match self.creep {
            CreepConfig::PerLayer { creep_per_layer_mm } if creep_per_layer_mm < 0.0 => {
                return Err(ImposeError::Config(
                    "Creep per layer must not be negative".to_string(),
                ));
            }
            CreepConfig::FromCaliper { paper_thickness_mm } if paper_thickness_mm < 0.0 => {
                return Err(ImposeError::Config(
                    "Paper thickness must not be negative".to_string(),
                ));
            }
            _ => {}
        }

        // Validate output format compatibility with binding type
        if let (
            BindingType::PerfectBinding | BindingType::SideStitch | BindingType::Spiral,
            OutputFormat::TwoSided,
        ) = (self.binding_type, self.output_format)
        {
            // TwoSided (separate front/back PDFs) doesn't make sense for these bindings
            return Err(ImposeError::Config(format!(
                "{:?} binding does not support TwoSided output format. Use DoubleSided or SingleSidedSequence.",
                self.binding_type
            )));
        }

        // Validate custom paper size minimum
        if let PaperSize::Custom {
            width_mm,
            height_mm,
        } = self.output_paper_size
            && (width_mm < 10.0 || height_mm < 10.0)
        {
            return Err(ImposeError::Config(
                "Custom paper size must be at least 10mm in each dimension".into(),
            ));
        }

        // Validate cascade configuration
        if let Some(cascade) = &self.cascade {
            if cascade.cols == 0 || cascade.rows == 0 {
                return Err(ImposeError::Config(
                    "Cascade columns and rows must be at least 1".into(),
                ));
            }
            if cascade.margin_mm < 0.0 {
                return Err(ImposeError::Config(
                    "Cascade margin must not be negative".into(),
                ));
            }
            if !cascade.is_trivial() {
                let (cell_w, cell_h) = self.sheet_dimensions_pt();
                if cell_w <= 0.0 || cell_h <= 0.0 {
                    return Err(ImposeError::Config(
                        "Cascade grid and margins leave no space for individual imposed sheets"
                            .into(),
                    ));
                }
            }
        }

        // Validate that sheet margins don't consume all space
        let leaf_bounds = self.leaf_bounds_pt();
        if !leaf_bounds.is_valid() {
            return Err(ImposeError::Config(
                "Sheet margins are too large for the paper size".into(),
            ));
        }

        // Validate effective book page area through the full layout pipeline
        let spread_positions =
            calculate_spread_positions(self.page_arrangement, leaf_bounds, &self.margins.leaf);
        let cut_edges = calculate_cut_edges(self.page_arrangement);
        for (spread, cuts) in spread_positions.iter().zip(cut_edges.iter()) {
            let content = calculate_spread_content(spread, &self.margins.leaf, *cuts);
            if !content.verso.is_valid() || !content.recto.is_valid() {
                return Err(ImposeError::Config(
                    "Margins are too large for the paper size and page arrangement".into(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::{
        BindingType, OutputFormat, PageArrangement, PaperSize, Rotation, ScalingMode, SplitMode,
    };
    use serde::{Deserialize, Serialize};

    // Manual implementations for types that don't derive Serialize/Deserialize
    impl Serialize for BindingType {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s = match self {
                BindingType::Signature => "Signature",
                BindingType::PerfectBinding => "PerfectBinding",
                BindingType::SideStitch => "SideStitch",
                BindingType::Spiral => "Spiral",
                BindingType::CaseBinding => "CaseBinding",
            };
            serializer.serialize_str(s)
        }
    }

    #[cfg(feature = "serde")]
    impl<'de> Deserialize<'de> for BindingType {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "Signature" => Ok(BindingType::Signature),
                "PerfectBinding" => Ok(BindingType::PerfectBinding),
                "SideStitch" => Ok(BindingType::SideStitch),
                "Spiral" => Ok(BindingType::Spiral),
                "CaseBinding" => Ok(BindingType::CaseBinding),
                _ => Err(serde::de::Error::custom("Unknown binding type")),
            }
        }
    }

    impl Serialize for PageArrangement {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s = match self {
                PageArrangement::Folio => "Folio",
                PageArrangement::Quarto => "Quarto",
                PageArrangement::Octavo => "Octavo",
            };
            serializer.serialize_str(s)
        }
    }

    impl<'de> Deserialize<'de> for PageArrangement {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "Folio" => Ok(PageArrangement::Folio),
                "Quarto" => Ok(PageArrangement::Quarto),
                "Octavo" => Ok(PageArrangement::Octavo),
                _ => Err(serde::de::Error::unknown_variant(
                    &s,
                    &["Folio", "Quarto", "Octavo"],
                )),
            }
        }
    }

    impl Serialize for PaperSize {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeStruct;
            match self {
                PaperSize::A3 => serializer.serialize_str("A3"),
                PaperSize::A4 => serializer.serialize_str("A4"),
                PaperSize::A5 => serializer.serialize_str("A5"),
                PaperSize::Letter => serializer.serialize_str("Letter"),
                PaperSize::Legal => serializer.serialize_str("Legal"),
                PaperSize::Tabloid => serializer.serialize_str("Tabloid"),
                PaperSize::Custom {
                    width_mm,
                    height_mm,
                } => {
                    let mut s = serializer.serialize_struct("Custom", 2)?;
                    s.serialize_field("width_mm", width_mm)?;
                    s.serialize_field("height_mm", height_mm)?;
                    s.end()
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for PaperSize {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::{self, MapAccess, Visitor};
            use std::fmt;

            struct PaperSizeVisitor;

            impl<'de> Visitor<'de> for PaperSizeVisitor {
                type Value = PaperSize;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a paper size")
                }

                fn visit_str<E>(self, value: &str) -> std::result::Result<PaperSize, E>
                where
                    E: de::Error,
                {
                    match value {
                        "A3" => Ok(PaperSize::A3),
                        "A4" => Ok(PaperSize::A4),
                        "A5" => Ok(PaperSize::A5),
                        "Letter" => Ok(PaperSize::Letter),
                        "Legal" => Ok(PaperSize::Legal),
                        "Tabloid" => Ok(PaperSize::Tabloid),
                        _ => Err(de::Error::unknown_variant(
                            value,
                            &["A3", "A4", "A5", "Letter", "Legal", "Tabloid", "Custom"],
                        )),
                    }
                }

                fn visit_map<M>(self, mut map: M) -> std::result::Result<PaperSize, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut width_mm = None;
                    let mut height_mm = None;

                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "width_mm" => width_mm = Some(map.next_value()?),
                            "height_mm" => height_mm = Some(map.next_value()?),
                            _ => {
                                let _: serde::de::IgnoredAny = map.next_value()?;
                            }
                        }
                    }

                    match (width_mm, height_mm) {
                        (Some(w), Some(h)) => Ok(PaperSize::Custom {
                            width_mm: w,
                            height_mm: h,
                        }),
                        _ => Err(de::Error::missing_field("width_mm or height_mm")),
                    }
                }
            }

            deserializer.deserialize_any(PaperSizeVisitor)
        }
    }

    // Simple derive-based implementations for remaining types
    impl Serialize for OutputFormat {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(match self {
                OutputFormat::DoubleSided => "DoubleSided",
                OutputFormat::TwoSided => "TwoSided",
                OutputFormat::SingleSidedSequence => "SingleSidedSequence",
            })
        }
    }

    impl<'de> Deserialize<'de> for OutputFormat {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "DoubleSided" => Ok(OutputFormat::DoubleSided),
                "TwoSided" => Ok(OutputFormat::TwoSided),
                "SingleSidedSequence" => Ok(OutputFormat::SingleSidedSequence),
                _ => Err(serde::de::Error::custom("Unknown output format")),
            }
        }
    }

    impl Serialize for ScalingMode {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(match self {
                ScalingMode::Fit => "Fit",
                ScalingMode::Fill => "Fill",
                ScalingMode::None => "None",
                ScalingMode::Stretch => "Stretch",
            })
        }
    }

    impl<'de> Deserialize<'de> for ScalingMode {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "Fit" => Ok(ScalingMode::Fit),
                "Fill" => Ok(ScalingMode::Fill),
                "None" => Ok(ScalingMode::None),
                "Stretch" => Ok(ScalingMode::Stretch),
                _ => Err(serde::de::Error::custom("Unknown scaling mode")),
            }
        }
    }

    impl Serialize for Rotation {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(match self {
                Rotation::None => "None",
                Rotation::Clockwise90 => "Clockwise90",
                Rotation::Clockwise180 => "Clockwise180",
                Rotation::Clockwise270 => "Clockwise270",
            })
        }
    }

    impl<'de> Deserialize<'de> for Rotation {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "None" => Ok(Rotation::None),
                "Clockwise90" => Ok(Rotation::Clockwise90),
                "Clockwise180" => Ok(Rotation::Clockwise180),
                "Clockwise270" => Ok(Rotation::Clockwise270),
                _ => Err(serde::de::Error::custom("Unknown rotation")),
            }
        }
    }

    impl Serialize for SplitMode {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeStruct;
            match self {
                SplitMode::None => serializer.serialize_str("None"),
                SplitMode::ByPages(n) => {
                    let mut s = serializer.serialize_struct("ByPages", 1)?;
                    s.serialize_field("pages", n)?;
                    s.end()
                }
                SplitMode::BySheets(n) => {
                    let mut s = serializer.serialize_struct("BySheets", 1)?;
                    s.serialize_field("sheets", n)?;
                    s.end()
                }
                SplitMode::BySignatures(n) => {
                    let mut s = serializer.serialize_struct("BySignatures", 1)?;
                    s.serialize_field("signatures", n)?;
                    s.end()
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for SplitMode {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::{self, MapAccess, Visitor};
            use std::fmt;

            struct SplitModeVisitor;

            impl<'de> Visitor<'de> for SplitModeVisitor {
                type Value = SplitMode;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a split mode")
                }

                fn visit_str<E>(self, value: &str) -> std::result::Result<SplitMode, E>
                where
                    E: de::Error,
                {
                    match value {
                        "None" => Ok(SplitMode::None),
                        _ => Err(de::Error::custom("Unknown split mode")),
                    }
                }

                fn visit_map<M>(self, mut map: M) -> std::result::Result<SplitMode, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut pages = None;
                    let mut sheets = None;
                    let mut signatures = None;

                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "pages" => pages = Some(map.next_value()?),
                            "sheets" => sheets = Some(map.next_value()?),
                            "signatures" => signatures = Some(map.next_value()?),
                            _ => {
                                let _: serde::de::IgnoredAny = map.next_value()?;
                            }
                        }
                    }

                    if let Some(p) = pages {
                        Ok(SplitMode::ByPages(p))
                    } else if let Some(s) = sheets {
                        Ok(SplitMode::BySheets(s))
                    } else if let Some(sig) = signatures {
                        Ok(SplitMode::BySignatures(sig))
                    } else {
                        Err(de::Error::missing_field("pages, sheets, or signatures"))
                    }
                }
            }

            deserializer.deserialize_any(SplitModeVisitor)
        }
    }
} // end of serde_impls module
