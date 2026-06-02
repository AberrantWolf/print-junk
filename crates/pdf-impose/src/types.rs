//! Core types for PDF imposition
//!
//! This module defines the fundamental types used throughout the imposition process:
//! - Error types and Result alias
//! - Paper sizes and orientation
//! - Binding and arrangement options
//! - Margin configurations
//! - Printer's marks settings

use thiserror::Error;

// =============================================================================
// Error Handling
// =============================================================================

/// Errors that can occur during imposition
#[derive(Error, Debug)]
pub enum ImposeError {
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("No pages to impose")]
    NoPages,
}

/// Result type alias for imposition operations
pub type Result<T> = std::result::Result<T, ImposeError>;

// =============================================================================
// Paper Configuration
// =============================================================================

// Orientation, paper sizes, and margins live in the shared `pdf-units` crate so
// every feature (imposition, flashcards, typesetting) shares one definition.
// Re-exported here so existing `crate::types::{PaperSize, Margins, …}` paths and
// the `pdf_impose::*` public surface keep working unchanged.
pub use pdf_units::{LeafMargins, Margins, Orientation, PaperSize, SheetMargins};

// =============================================================================
// Binding Configuration
// =============================================================================

/// Binding methods for the finished book
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum BindingType {
    /// Saddle-stitch binding (folded sheets, stapled at spine)
    /// Best for booklets up to ~64 pages
    #[default]
    Signature,
    /// Perfect binding (pages glued to spine)
    /// Best for thicker books
    PerfectBinding,
    /// Side stitch binding (staples through side margin)
    SideStitch,
    /// Spiral/coil binding
    Spiral,
    /// Case binding (sewn signatures in hardcover)
    CaseBinding,
}

impl BindingType {
    /// Returns true if this binding type uses signatures (folded sheets)
    pub fn uses_signatures(self) -> bool {
        matches!(self, BindingType::Signature | BindingType::CaseBinding)
    }
}

/// Page arrangement within a signature
///
/// Determines how each sheet is folded. The number of sheets nested
/// together per signature is configured separately via `sheets_per_signature`
/// on `ImpositionOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum PageArrangement {
    /// Folio: 4 pages per sheet (1 fold)
    /// Grid: 2 columns × 1 row
    Folio,
    /// Quarto: 8 pages per sheet (2 folds)
    /// Grid: 2 columns × 2 rows
    #[default]
    Quarto,
    /// Octavo: 16 pages per sheet (3 folds)
    /// Grid: 4 columns × 2 rows
    Octavo,
}

impl PageArrangement {
    /// Number of pages produced by folding a single sheet
    pub fn pages_per_sheet(self) -> usize {
        match self {
            PageArrangement::Folio => 4,
            PageArrangement::Quarto => 8,
            PageArrangement::Octavo => 16,
        }
    }

    /// Grid dimensions (columns, rows) for this arrangement
    pub fn grid_dimensions(self) -> (usize, usize) {
        match self {
            PageArrangement::Folio => (2, 1),
            PageArrangement::Quarto => (2, 2),
            PageArrangement::Octavo => (4, 2),
        }
    }
}

// =============================================================================
// Output Configuration
// =============================================================================

/// Output PDF format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum OutputFormat {
    /// Single PDF with both sides interleaved (page 1 front, page 1 back, page 2 front, ...)
    #[default]
    DoubleSided,
    /// Two separate PDFs (all fronts, then all backs)
    TwoSided,
    /// Single PDF with pages in print order for single-sided printing
    SingleSidedSequence,
}

/// Page scaling behavior when source pages don't match output cell size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ScalingMode {
    /// Fit entire page within available space (preserve aspect ratio, may have margins)
    #[default]
    Fit,
    /// Fill available space (preserve aspect ratio, may crop)
    Fill,
    /// No scaling (center at original size)
    None,
    /// Stretch to fill (ignore aspect ratio)
    Stretch,
}

/// Rotation to apply to source pages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Rotation {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl Rotation {
    /// Get rotation in degrees
    pub fn degrees(self) -> i32 {
        match self {
            Rotation::None => 0,
            Rotation::Clockwise90 => 90,
            Rotation::Clockwise180 => 180,
            Rotation::Clockwise270 => 270,
        }
    }

    /// Create from degrees (normalized to 0, 90, 180, 270)
    pub fn from_degrees(deg: i32) -> Self {
        match deg.rem_euclid(360) {
            90 => Rotation::Clockwise90,
            180 => Rotation::Clockwise180,
            270 => Rotation::Clockwise270,
            _ => Rotation::None, // 0 and snap non-90° to None
        }
    }
}

// Margins (`SheetMargins`, `LeafMargins`, `Margins`) are re-exported from
// `pdf-units` at the top of this module.

// =============================================================================
// Printer's Marks
// =============================================================================

/// Printer's marks configuration
///
/// These marks help with alignment, folding, and trimming during finishing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct PrinterMarks {
    /// Add fold lines (dashed) - where paper should be folded, including spine fold
    pub fold_lines: bool,
    /// Add trim marks (L-shaped marks at inter-spread fold edges for guillotine trimming)
    pub trim_marks: bool,
    /// Add crop marks (L-shaped corner marks at sheet edges)
    pub crop_marks: bool,
    /// Add registration marks (crosshairs for alignment)
    pub registration_marks: bool,
    /// Add sewing station marks along spine fold (signature/case binding only)
    pub sewing_marks: bool,
    /// Add collation marks on spine edge for signature ordering verification
    pub collation_marks: bool,
}

impl PrinterMarks {
    /// Enable all marks
    pub fn all() -> Self {
        Self {
            fold_lines: true,
            trim_marks: true,
            crop_marks: true,
            registration_marks: true,
            sewing_marks: true,
            collation_marks: true,
        }
    }

    /// Check if any marks are enabled
    pub fn any_enabled(&self) -> bool {
        self.fold_lines
            || self.trim_marks
            || self.crop_marks
            || self.registration_marks
            || self.sewing_marks
            || self.collation_marks
    }
}

// =============================================================================
// Marks Appearance
// =============================================================================

/// Visual appearance settings for a group of printer's marks.
///
/// Controls line thickness and gray level. Two instances are used:
/// one for interior marks (fold lines, trim marks, sewing marks — near
/// trim/fold edges, risk of showing in the finished book) and one for
/// exterior marks (crop marks, registration marks, collation marks,
/// cascade cut lines — at sheet edges, reliably trimmed or covered).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MarksAppearance {
    /// Gray value for mark color: 0.0 = black (most visible), 1.0 = white (invisible).
    /// Maps directly to the PDF `DeviceGray` colorspace operators (G/g).
    pub gray: f32,
    /// Multiplier applied to each mark type's base line width.
    /// 1.0 = default widths, 0.5 = half width, 2.0 = double width.
    pub line_width_scale: f32,
}

impl Default for MarksAppearance {
    fn default() -> Self {
        Self {
            gray: 0.0,
            line_width_scale: 1.0,
        }
    }
}

// =============================================================================
// Sewing Configuration
// =============================================================================

/// Configuration for sewing station marks along the spine fold
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct SewingConfig {
    /// Number of sewing stations between the two kettle stitch positions
    pub station_count: usize,
    /// Distance from head/tail edges to kettle stitch holes, in mm
    pub kettle_offset_mm: f32,
}

impl Default for SewingConfig {
    fn default() -> Self {
        Self {
            station_count: 3,
            kettle_offset_mm: 12.0,
        }
    }
}

// =============================================================================
// Cascade Configuration
// =============================================================================

/// Cascade (step-and-repeat) configuration for tiling multiple imposed sheets
/// onto a single larger output page.
///
/// When cascade is active, the `output_paper_size` on `ImpositionOptions`
/// represents the large cascade sheet. Each individual imposed layout (cell)
/// is sized by dividing the available area by the grid dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct CascadeConfig {
    /// Number of columns in the cascade grid
    pub cols: usize,
    /// Number of rows in the cascade grid
    pub rows: usize,
    /// Gap between cascade cells in mm
    pub margin_mm: f32,
    /// Whether to add cut lines between cascade cells
    pub cut_lines: bool,
    /// Flip axis for duplex alignment
    pub flip_axis: FlipAxis,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            cols: 1,
            rows: 1,
            margin_mm: 5.0,
            cut_lines: false,
            flip_axis: FlipAxis::default(),
        }
    }
}

impl CascadeConfig {
    /// Total number of cells in the cascade grid
    pub fn cells(&self) -> usize {
        self.cols * self.rows
    }

    /// Returns true if the cascade is trivial (1×1, equivalent to no cascade)
    pub fn is_trivial(&self) -> bool {
        self.cols <= 1 && self.rows <= 1
    }
}

/// Flip axis for duplex printing alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FlipAxis {
    /// Long-edge flip: columns reverse on back side
    #[default]
    LongEdge,
    /// Short-edge flip: rows reverse on back side
    ShortEdge,
}

// =============================================================================
// Creep Compensation
// =============================================================================

/// Creep (shingling) compensation configuration.
///
/// When multiple sheets are nested in a folded signature, inner sheets protrude
/// at the fore edge due to paper thickness. After trimming, inner pages are
/// narrower. Creep compensation shifts each page's content toward the spine
/// proportionally to its nesting depth, so that after trimming all pages have
/// visually consistent margins.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "mode"))]
pub enum CreepConfig {
    /// No creep compensation
    #[default]
    None,
    /// User provides a fixed creep offset per nested leaf (pair of pages)
    PerLayer {
        /// Creep shift per layer, in mm (typically 0.05–0.2mm)
        creep_per_layer_mm: f32,
    },
    /// Computed from paper caliper using fold geometry (π·t / 2 per layer)
    FromCaliper {
        /// Paper caliper (thickness of one sheet) in mm (e.g., 0.1 for 80gsm copy paper)
        paper_thickness_mm: f32,
    },
}

impl CreepConfig {
    /// Returns true if creep compensation is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }
}

// =============================================================================
// Output Splitting
// =============================================================================

/// How to split the output PDF
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SplitMode {
    /// No splitting - single output file
    #[default]
    None,
    /// Split by number of signatures per output file. Must be ≥ 1, and only
    /// valid when `binding_type.uses_signatures()`.
    BySignatures(usize),
}

// =============================================================================
// Warnings
// =============================================================================

/// Warnings about an imposition job that don't prevent it from completing
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// Blank pages added exceed 25% of the total signature capacity
    ExcessiveBlankPadding {
        blank_count: usize,
        total_pages: usize,
        percent: f32,
    },
    /// Kettle offset is too large for the spine length — sewing marks will be off-page
    KettleOffsetTooLarge { offset_mm: f32, max_mm: f32 },
    /// Source page `MediaBox` could not be parsed; using default Letter dimensions
    DefaultDimensionsUsed { page_index: usize },
    /// Flyleaves requested on a document with no pages (no effect)
    FlyleavesOnEmptyDocument,
    /// Maximum creep offset exceeds the configured spine margin
    CreepExceedsSpineMargin { max_creep_mm: f32, spine_mm: f32 },
    /// Generic warning
    Other(String),
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Warning::ExcessiveBlankPadding {
                blank_count,
                total_pages,
                percent,
            } => write!(
                f,
                "{blank_count} blank pages added ({percent:.0}% of {total_pages} total) \
                 — consider adjusting content length or page arrangement"
            ),
            Warning::KettleOffsetTooLarge { offset_mm, max_mm } => write!(
                f,
                "Kettle stitch offset ({offset_mm:.1}mm) exceeds half the spine length \
                 ({max_mm:.1}mm) — sewing marks will be positioned off-page"
            ),
            Warning::DefaultDimensionsUsed { page_index } => write!(
                f,
                "Page {page_index}: could not read page dimensions, using default Letter size (8.5×11in)"
            ),
            Warning::FlyleavesOnEmptyDocument => {
                write!(f, "Flyleaves requested but source document has no pages")
            }
            Warning::CreepExceedsSpineMargin {
                max_creep_mm,
                spine_mm,
            } => write!(
                f,
                "Maximum creep offset ({max_creep_mm:.2}mm) exceeds spine margin \
                 ({spine_mm:.1}mm) — inner pages may cross the spine fold"
            ),
            Warning::Other(msg) => write!(f, "{msg}"),
        }
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics about an imposition job
#[derive(Debug, Clone, PartialEq)]
pub struct ImpositionStatistics {
    /// Total number of source pages (including flyleaves)
    pub source_pages: usize,
    /// Total number of output sheets (physical pieces of paper)
    pub output_sheets: usize,
    /// Number of signatures (if using signature binding)
    pub signatures: Option<usize>,
    /// Pages per signature (if using signatures)
    pub pages_per_signature: Option<Vec<usize>>,
    /// Total output page count (usually `output_sheets` × 2)
    pub output_pages: usize,
    /// Number of blank pages added for padding
    pub blank_pages_added: usize,
    /// Number of imposed cells per cascade sheet (if cascade is active)
    pub cascade_cells_per_sheet: Option<usize>,
    /// Warnings about potential issues
    pub warnings: Vec<Warning>,
}

impl ImpositionStatistics {
    /// Returns true if any blank pages were added
    pub fn has_blank_pages(&self) -> bool {
        self.blank_pages_added > 0
    }
}
