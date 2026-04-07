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

/// Paper orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    /// Portrait: height > width
    Portrait,
    /// Landscape: width > height (default for imposition — pages are arranged side by side)
    #[default]
    Landscape,
}

impl Orientation {
    /// Returns true if landscape orientation
    pub fn is_landscape(self) -> bool {
        matches!(self, Orientation::Landscape)
    }

    /// Returns the opposite orientation
    pub fn flip(self) -> Self {
        match self {
            Orientation::Portrait => Orientation::Landscape,
            Orientation::Landscape => Orientation::Portrait,
        }
    }
}

/// Standard paper sizes
///
/// All dimensions are stored in portrait orientation (width < height).
/// Use `dimensions_with_orientation` to get landscape dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperSize {
    /// ISO A3 (297mm × 420mm)
    A3,
    /// ISO A4 (210mm × 297mm)
    A4,
    /// ISO A5 (148mm × 210mm)
    A5,
    /// US Letter (8.5" × 11")
    Letter,
    /// US Legal (8.5" × 14")
    Legal,
    /// US Tabloid (11" × 17")
    Tabloid,
    /// Custom dimensions in millimeters
    Custom { width_mm: f32, height_mm: f32 },
}

impl Default for PaperSize {
    fn default() -> Self {
        PaperSize::Letter
    }
}

impl PaperSize {
    /// Get base dimensions in millimeters (always portrait: width < height for standard sizes)
    pub fn dimensions_mm(self) -> (f32, f32) {
        match self {
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Tabloid => (279.4, 431.8),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
        }
    }

    /// Get dimensions with orientation applied
    pub fn dimensions_with_orientation(self, orientation: Orientation) -> (f32, f32) {
        let (w, h) = self.dimensions_mm();
        match orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    /// Get dimensions in points (1/72 inch)
    pub fn dimensions_pt(self) -> (f32, f32) {
        let (w, h) = self.dimensions_mm();
        (crate::constants::mm_to_pt(w), crate::constants::mm_to_pt(h))
    }

    /// Get dimensions in points with orientation applied
    pub fn dimensions_pt_with_orientation(self, orientation: Orientation) -> (f32, f32) {
        let (w, h) = self.dimensions_with_orientation(orientation);
        (crate::constants::mm_to_pt(w), crate::constants::mm_to_pt(h))
    }
}

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
            0 => Rotation::None,
            90 => Rotation::Clockwise90,
            180 => Rotation::Clockwise180,
            270 => Rotation::Clockwise270,
            _ => Rotation::None, // Snap to nearest 90°
        }
    }
}

// =============================================================================
// Margins
// =============================================================================

/// Sheet margins - printer-safe area around the entire output sheet.
///
/// These margins ensure content stays within the printer's printable area.
/// 10mm default ensures printer's marks (crop marks, registration marks) remain visible
/// even on consumer printers that can't print to the edge.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SheetMargins {
    pub top_mm: f32,
    pub bottom_mm: f32,
    pub left_mm: f32,
    pub right_mm: f32,
}

impl Default for SheetMargins {
    fn default() -> Self {
        Self::uniform(10.0)
    }
}

impl SheetMargins {
    /// Create uniform margins on all sides
    pub fn uniform(margin_mm: f32) -> Self {
        Self {
            top_mm: margin_mm,
            bottom_mm: margin_mm,
            left_mm: margin_mm,
            right_mm: margin_mm,
        }
    }

    /// Create with no margins (borderless)
    pub fn none() -> Self {
        Self::uniform(0.0)
    }

    /// Total horizontal margin (left + right)
    pub fn horizontal_mm(&self) -> f32 {
        self.left_mm + self.right_mm
    }

    /// Total vertical margin (top + bottom)
    pub fn vertical_mm(&self) -> f32 {
        self.top_mm + self.bottom_mm
    }
}

/// Leaf margins - applied to each logical page within the imposed sheet.
///
/// These provide:
/// - Trim space for cutting after folding
/// - Spine gutter for readability when bound
/// - Consistent page margins in the final book
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct LeafMargins {
    /// Top margin (head) of each leaf
    pub top_mm: f32,
    /// Bottom margin (tail) of each leaf
    pub bottom_mm: f32,
    /// Outer margin (fore edge) - the edge opposite the spine
    pub fore_edge_mm: f32,
    /// Inner margin (spine/gutter) - extra space near the binding
    pub spine_mm: f32,
    /// Trim allowance - extra material around fold edges, trimmed away after binding (3mm standard)
    pub trim_allowance_mm: f32,
}

impl Default for LeafMargins {
    fn default() -> Self {
        Self {
            top_mm: 0.0,
            bottom_mm: 0.0,
            fore_edge_mm: 0.0,
            spine_mm: 0.0,
            trim_allowance_mm: 3.0,
        }
    }
}

impl LeafMargins {
    /// Create uniform margins (except spine and trim allowance)
    pub fn uniform(margin_mm: f32) -> Self {
        Self {
            top_mm: margin_mm,
            bottom_mm: margin_mm,
            fore_edge_mm: margin_mm,
            spine_mm: margin_mm,
            trim_allowance_mm: 3.0,
        }
    }
}

/// Combined margins for imposition
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Margins {
    /// Printer-safe margins around the entire output sheet
    pub sheet: SheetMargins,
    /// Margins for each logical page/leaf
    pub leaf: LeafMargins,
}

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
// Output Splitting
// =============================================================================

/// How to split the output PDF
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SplitMode {
    /// No splitting - single output file
    #[default]
    None,
    /// Split by number of pages
    ByPages(usize),
    /// Split by number of sheets
    BySheets(usize),
    /// Split by number of signatures
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
    /// Source page MediaBox could not be parsed; using default Letter dimensions
    DefaultDimensionsUsed { page_index: usize },
    /// Flyleaves requested on a document with no pages (no effect)
    FlyleavesOnEmptyDocument,
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
    /// Total output page count (usually output_sheets × 2)
    pub output_pages: usize,
    /// Number of blank pages added for padding
    pub blank_pages_added: usize,
    /// Warnings about potential issues
    pub warnings: Vec<Warning>,
}

impl ImpositionStatistics {
    /// Returns true if any blank pages were added
    pub fn has_blank_pages(&self) -> bool {
        self.blank_pages_added > 0
    }
}
