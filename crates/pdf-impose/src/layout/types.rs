//! Layout data types for imposition
//!
//! The fundamental unit is a **Spread** (verso + recto page pair).
//! Arrangements are built by composition:
//! - Folio = 1 spread per sheet side
//! - Quarto = 2 spreads stacked (top rotated 180°)
//! - Octavo = 4 spreads in 2x2 (top row rotated 180°)
//!
//! Key types:
//! - `Spread` - A verso + recto page pair
//! - `SpreadPosition` - Where a spread is placed on the sheet
//! - `SpreadCutEdges` - Which edges have cut lines
//! - `PagePlacement` - Final rendering information for a single page
//! - `SpreadSheetLayout` - All spreads for one side of a sheet

// =============================================================================
// Point
// =============================================================================

/// A 2D point in PDF coordinate space (origin at bottom-left)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Create a new point
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Origin point (0, 0)
    pub const fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Offset this point by the given amounts
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

impl From<(f32, f32)> for Point {
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

impl From<Point> for (f32, f32) {
    fn from(p: Point) -> Self {
        (p.x, p.y)
    }
}

// =============================================================================
// Spread Types (New Compositional Model)
// =============================================================================

/// A spread is the fundamental unit of imposition: two facing pages.
///
/// After folding, a spread becomes two facing pages in the book:
/// - Verso (left): even page numbers (2, 4, 6, ...)
/// - Recto (right): odd page numbers (1, 3, 5, ...)
///
/// Note: Page 1 (title page) is recto, so verso pages are "before" recto
/// in reading order within a spread.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spread {
    /// Left page (even page numbers). None = blank.
    pub verso_page: Option<usize>,
    /// Right page (odd page numbers). None = blank.
    pub recto_page: Option<usize>,
}

impl Spread {
    /// Create a new spread with both pages
    pub fn new(verso: Option<usize>, recto: Option<usize>) -> Self {
        Self {
            verso_page: verso,
            recto_page: recto,
        }
    }

    /// Create a blank spread
    pub fn blank() -> Self {
        Self::default()
    }

    /// Check if both pages are blank
    pub fn is_blank(&self) -> bool {
        self.verso_page.is_none() && self.recto_page.is_none()
    }

    /// Check if at least one page has content
    pub fn has_content(&self) -> bool {
        self.verso_page.is_some() || self.recto_page.is_some()
    }
}

/// Position and orientation of a spread within a sheet layout.
///
/// A spread position defines where the spread's two pages will be
/// placed on the physical sheet, including any rotation.
#[derive(Debug, Clone, PartialEq)]
pub struct SpreadPosition {
    /// The spread's page assignments
    pub spread: Spread,
    /// Origin point (bottom-left of spread bounding box)
    pub origin: Point,
    /// Total width of the spread area (both pages combined)
    pub width: f32,
    /// Height of the spread area
    pub height: f32,
    /// Whether this spread is rotated 180 degrees
    pub rotated: bool,
    /// Index of this spread within the arrangement (0-based)
    pub spread_index: usize,
}

impl SpreadPosition {
    /// Create a new spread position
    pub fn new(
        spread: Spread,
        origin: Point,
        width: f32,
        height: f32,
        rotated: bool,
        spread_index: usize,
    ) -> Self {
        Self {
            spread,
            origin,
            width,
            height,
            rotated,
            spread_index,
        }
    }

    /// Create an empty spread position (for layout calculation)
    pub fn empty(
        origin: Point,
        width: f32,
        height: f32,
        rotated: bool,
        spread_index: usize,
    ) -> Self {
        Self {
            spread: Spread::blank(),
            origin,
            width,
            height,
            rotated,
            spread_index,
        }
    }

    /// Get the bounding rectangle for this spread
    pub fn bounds(&self) -> Rect {
        Rect::new(self.origin.x, self.origin.y, self.width, self.height)
    }

    /// Width of each page (half the spread width)
    pub fn page_width(&self) -> f32 {
        self.width / 2.0
    }

    /// Get rotation in degrees (0 or 180)
    pub fn rotation_degrees(&self) -> f32 {
        if self.rotated { 180.0 } else { 0.0 }
    }

    /// Get the center point of this spread
    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.width / 2.0,
            self.origin.y + self.height / 2.0,
        )
    }
}

/// Which edges of a spread have cut lines (vs folds or sheet edges).
///
/// Cut edges need cut margin applied. This is used to determine
/// where trim marks should appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpreadCutEdges {
    /// Cut line above this spread
    pub top: bool,
    /// Cut line below this spread
    pub bottom: bool,
    /// Cut line to the left of this spread
    pub left: bool,
    /// Cut line to the right of this spread
    pub right: bool,
}

impl SpreadCutEdges {
    /// No cuts on any edge
    pub fn none() -> Self {
        Self::default()
    }

    /// Check if any edge has a cut
    pub fn any(&self) -> bool {
        self.top || self.bottom || self.left || self.right
    }
}

/// Content areas for the two pages in a spread.
///
/// These are the final rectangles where page content is rendered,
/// after applying all margins.
#[derive(Debug, Clone, PartialEq)]
pub struct SpreadContentAreas {
    /// Content area for the verso (left) page
    pub verso: Rect,
    /// Content area for the recto (right) page
    pub recto: Rect,
}

impl SpreadContentAreas {
    /// Create new spread content areas
    pub fn new(verso: Rect, recto: Rect) -> Self {
        Self { verso, recto }
    }
}

/// A sheet side layout using the spread-based model.
///
/// Contains all spread positions for one side of a physical sheet.
#[derive(Debug, Clone)]
pub struct SpreadSheetLayout {
    /// Which side of the physical sheet
    pub side: SheetSide,
    /// All spread positions for this side
    pub spreads: Vec<SpreadPosition>,
    /// The leaf area bounds (inside sheet margins)
    pub leaf_bounds: Rect,
}

impl SpreadSheetLayout {
    /// Create a new spread sheet layout
    pub fn new(side: SheetSide, spreads: Vec<SpreadPosition>, leaf_bounds: Rect) -> Self {
        Self {
            side,
            spreads,
            leaf_bounds,
        }
    }

    /// Number of spreads on this sheet side
    pub fn spread_count(&self) -> usize {
        self.spreads.len()
    }

    /// Get spreads that have any content
    pub fn non_blank_spreads(&self) -> impl Iterator<Item = &SpreadPosition> {
        self.spreads.iter().filter(|s| s.spread.has_content())
    }
}

// =============================================================================
// Page and Sheet Sides
// =============================================================================

/// Which side of a bound book this page appears on after folding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageSide {
    /// Right-hand page (odd page numbers in final book: 1, 3, 5, ...)
    /// The spine edge is on the left
    #[default]
    Recto,
    /// Left-hand page (even page numbers in final book: 2, 4, 6, ...)
    /// The spine edge is on the right
    Verso,
}

impl PageSide {
    /// Returns true if this is a recto (right-hand) page
    pub fn is_recto(self) -> bool {
        matches!(self, PageSide::Recto)
    }

    /// Returns the opposite side
    pub fn opposite(self) -> Self {
        match self {
            PageSide::Recto => PageSide::Verso,
            PageSide::Verso => PageSide::Recto,
        }
    }

    /// Get page side from 1-based page number
    pub fn from_page_number(page_num: usize) -> Self {
        if page_num % 2 == 1 {
            PageSide::Recto // Odd pages are recto
        } else {
            PageSide::Verso // Even pages are verso
        }
    }
}

/// Which physical side of the printed sheet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SheetSide {
    /// Front of the sheet (printed first in duplex)
    #[default]
    Front,
    /// Back of the sheet (printed second in duplex)
    Back,
}

impl SheetSide {
    /// Returns true if this is the front side
    pub fn is_front(self) -> bool {
        matches!(self, SheetSide::Front)
    }

    /// Returns the opposite side
    pub fn opposite(self) -> Self {
        match self {
            SheetSide::Front => SheetSide::Back,
            SheetSide::Back => SheetSide::Front,
        }
    }
}

// =============================================================================
// Grid Position
// =============================================================================

/// Position within the grid (row, column)
///
/// Row 0 is the top row, column 0 is the leftmost column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GridPosition {
    /// Row index (0 = top row)
    pub row: usize,
    /// Column index (0 = leftmost column)
    pub col: usize,
}

impl GridPosition {
    /// Create a new grid position
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    /// Convert to flat index in row-major order
    pub fn to_index(self, cols: usize) -> usize {
        self.row * cols + self.col
    }

    /// Create from flat index in row-major order
    pub fn from_index(index: usize, cols: usize) -> Self {
        Self {
            row: index / cols,
            col: index % cols,
        }
    }
}

// =============================================================================
// Signature Slot
// =============================================================================

/// A page's position within a signature
///
/// This captures all the information needed to place a page correctly:
/// - Where it goes in the grid
/// - Whether it needs rotation
/// - Which side of the book it will be on after folding
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureSlot {
    /// Index in the flat signature order (0..pages_per_sig)
    pub slot_index: usize,
    /// Which sheet side (front/back)
    pub sheet_side: SheetSide,
    /// Position in grid (row, col) - row 0 is top
    pub grid_pos: GridPosition,
    /// Whether this slot needs 180° rotation
    pub rotated: bool,
    /// Which book side this page will be on after folding
    pub page_side: PageSide,
}

impl SignatureSlot {
    /// Create a new signature slot
    pub fn new(
        slot_index: usize,
        sheet_side: SheetSide,
        row: usize,
        col: usize,
        rotated: bool,
        page_side: PageSide,
    ) -> Self {
        Self {
            slot_index,
            sheet_side,
            grid_pos: GridPosition::new(row, col),
            rotated,
            page_side,
        }
    }

    /// Get rotation in degrees (0 or 180)
    pub fn rotation_degrees(&self) -> f32 {
        if self.rotated { 180.0 } else { 0.0 }
    }
}

// =============================================================================
// Rectangle
// =============================================================================

/// A rectangular area in points
///
/// Used for cell bounds, content areas, and page placements.
/// Coordinates are in PDF space (origin at bottom-left).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// X position (left edge)
    pub x: f32,
    /// Y position (bottom edge)
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from corner points (left, bottom, right, top)
    pub fn from_corners(left: f32, bottom: f32, right: f32, top: f32) -> Self {
        Self {
            x: left,
            y: bottom,
            width: right - left,
            height: top - bottom,
        }
    }

    /// Left edge x coordinate (same as x)
    pub fn left(&self) -> f32 {
        self.x
    }

    /// Bottom edge y coordinate (same as y)
    pub fn bottom(&self) -> f32 {
        self.y
    }

    /// Right edge x coordinate
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Top edge y coordinate
    pub fn top(&self) -> f32 {
        self.y + self.height
    }

    /// Center x coordinate
    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    /// Center y coordinate
    pub fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }

    /// Center point as (x, y) tuple
    pub fn center(&self) -> (f32, f32) {
        (self.center_x(), self.center_y())
    }

    /// Area of the rectangle
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Check if the rectangle has positive area
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    /// Inset the rectangle by the given amounts
    pub fn inset(&self, left: f32, bottom: f32, right: f32, top: f32) -> Self {
        Self {
            x: self.x + left,
            y: self.y + bottom,
            width: self.width - left - right,
            height: self.height - bottom - top,
        }
    }

    /// Inset the rectangle uniformly on all sides
    pub fn inset_uniform(&self, amount: f32) -> Self {
        self.inset(amount, amount, amount, amount)
    }
}

// =============================================================================
// Page Placement
// =============================================================================

/// Final placement of a source page on the output sheet
///
/// This is the result of all layout calculations and contains
/// everything needed to render the page.
#[derive(Debug, Clone, PartialEq)]
pub struct PagePlacement {
    /// Source page index (None = blank page)
    pub source_page: Option<usize>,
    /// Position and size of the page content in points
    pub content_rect: Rect,
    /// Rotation to apply in degrees (0.0 or 180.0)
    pub rotation_degrees: f32,
    /// Scale factor applied to the source page
    pub scale: f32,
    /// The signature slot this placement corresponds to
    pub slot: SignatureSlot,
}

impl PagePlacement {
    /// Returns true if this is a blank page
    pub fn is_blank(&self) -> bool {
        self.source_page.is_none()
    }

    /// Returns true if the page is rotated
    pub fn is_rotated(&self) -> bool {
        self.rotation_degrees.abs() > 0.1
    }
}

// =============================================================================
// Sheet Layout
// =============================================================================

/// Information about a single output sheet side
///
/// Contains all the page placements and bounds for rendering one side
/// of a physical sheet.
#[derive(Debug, Clone)]
pub struct SheetLayout {
    /// Which side of the physical sheet
    pub side: SheetSide,
    /// All page placements for this side
    pub placements: Vec<PagePlacement>,
    /// The leaf area bounds (inside sheet margins)
    pub leaf_bounds: Rect,
}

impl SheetLayout {
    /// Get placements that have actual source pages (not blank)
    pub fn non_blank_placements(&self) -> impl Iterator<Item = &PagePlacement> {
        self.placements.iter().filter(|p| !p.is_blank())
    }

    /// Number of non-blank pages on this sheet side
    pub fn content_count(&self) -> usize {
        self.placements.iter().filter(|p| !p.is_blank()).count()
    }
}
