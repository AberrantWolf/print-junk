//! Unified trim bounds calculation
//!
//! Trim marks should be positioned based on the **largest content rectangle**
//! across all pages in the book, not per-page. This ensures consistent trim
//! positions regardless of source page size variations.
//!
//! From the user's diagram analysis:
//! > "Where trim markings go is calculated by the largest content rect
//! > (including leaf margins) used across the whole book."
//!
//! This module calculates unified bounds for trim mark placement.

use super::Rect;

// =============================================================================
// Unified Trim Bounds
// =============================================================================

/// Unified trim bounds for consistent mark placement.
///
/// Rather than placing trim marks relative to each individual page's
/// content area, we calculate a unified "trim rectangle" based on
/// the maximum content dimensions across all pages. This ensures:
///
/// 1. Trim marks are in consistent positions on every sheet
/// 2. Cut lines pass through the same positions regardless of page content
/// 3. The final trimmed book has uniform page sizes
#[derive(Debug, Clone, Default)]
pub struct UnifiedTrimBounds {
    /// Maximum content width across all pages
    pub max_content_width: f32,
    /// Maximum content height across all pages
    pub max_content_height: f32,
}

impl UnifiedTrimBounds {
    /// Create new unified trim bounds
    pub fn new(max_content_width: f32, max_content_height: f32) -> Self {
        Self {
            max_content_width,
            max_content_height,
        }
    }

    /// Calculate unified bounds from a collection of content rectangles.
    ///
    /// Finds the maximum width and height across all rectangles.
    pub fn from_content_rects(rects: &[Rect]) -> Self {
        let max_width = rects
            .iter()
            .filter(|r| r.is_valid())
            .map(|r| r.width)
            .fold(0.0_f32, |a, b| a.max(b));

        let max_height = rects
            .iter()
            .filter(|r| r.is_valid())
            .map(|r| r.height)
            .fold(0.0_f32, |a, b| a.max(b));

        Self {
            max_content_width: max_width,
            max_content_height: max_height,
        }
    }

    /// Calculate unified bounds from page dimensions directly.
    ///
    /// Takes a list of (width, height) tuples for source pages.
    pub fn from_page_dimensions(dimensions: &[(f32, f32)]) -> Self {
        let max_width = dimensions
            .iter()
            .map(|(w, _)| *w)
            .fold(0.0_f32, |a, b| a.max(b));

        let max_height = dimensions
            .iter()
            .map(|(_, h)| *h)
            .fold(0.0_f32, |a, b| a.max(b));

        Self {
            max_content_width: max_width,
            max_content_height: max_height,
        }
    }

    /// Check if bounds are valid (non-zero dimensions)
    pub fn is_valid(&self) -> bool {
        self.max_content_width > 0.0 && self.max_content_height > 0.0
    }
}

// =============================================================================
// Trim Mark Positions
// =============================================================================

/// Positions for trim marks on a sheet.
///
/// Trim marks appear at cut line intersections with content boundaries.
/// This structure defines where those marks should be placed based on
/// unified content bounds.
#[derive(Debug, Clone, Default)]
pub struct TrimMarkPositions {
    /// Positions where vertical cut lines intersect content (x coordinates)
    pub vertical_cut_x: Vec<f32>,
    /// Positions where horizontal cut lines intersect content (y coordinates)
    pub horizontal_cut_y: Vec<f32>,
    /// Content boundary positions (for mark endpoints)
    pub content_bounds: Vec<TrimContentBounds>,
}

/// Content bounds for a single spread position, used for trim marks.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrimContentBounds {
    /// Left edge of content
    pub left: f32,
    /// Right edge of content
    pub right: f32,
    /// Bottom edge of content
    pub bottom: f32,
    /// Top edge of content
    pub top: f32,
}

impl TrimContentBounds {
    /// Create from a rectangle
    pub fn from_rect(rect: &Rect) -> Self {
        Self {
            left: rect.x,
            right: rect.right(),
            bottom: rect.y,
            top: rect.top(),
        }
    }

    /// Create from unified bounds centered at a position
    pub fn from_unified(unified: &UnifiedTrimBounds, center_x: f32, center_y: f32) -> Self {
        let half_w = unified.max_content_width / 2.0;
        let half_h = unified.max_content_height / 2.0;
        Self {
            left: center_x - half_w,
            right: center_x + half_w,
            bottom: center_y - half_h,
            top: center_y + half_h,
        }
    }

    /// Width of the content area
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Height of the content area
    pub fn height(&self) -> f32 {
        self.top - self.bottom
    }
}

impl TrimMarkPositions {
    /// Create trim positions from cut lines and content bounds.
    ///
    /// # Arguments
    /// * `vertical_cuts` - X positions of vertical cut lines
    /// * `horizontal_cuts` - Y positions of horizontal cut lines
    /// * `content_bounds` - Bounds of content areas for each spread
    pub fn new(
        vertical_cuts: Vec<f32>,
        horizontal_cuts: Vec<f32>,
        content_bounds: Vec<TrimContentBounds>,
    ) -> Self {
        Self {
            vertical_cut_x: vertical_cuts,
            horizontal_cut_y: horizontal_cuts,
            content_bounds,
        }
    }

    /// Check if there are any cuts requiring trim marks
    pub fn has_cuts(&self) -> bool {
        !self.vertical_cut_x.is_empty() || !self.horizontal_cut_y.is_empty()
    }
}

// =============================================================================
// Calculate Trim Positions for Arrangements
// =============================================================================

/// Calculate trim mark positions for an arrangement.
///
/// This determines where trim marks should appear based on:
/// - Cut line positions (from arrangement type)
/// - Unified content bounds (from page dimensions)
/// - Spread positions on the sheet
pub fn calculate_trim_positions(
    cut_positions: &super::arrangement::CutPositions,
    spread_content_bounds: &[TrimContentBounds],
) -> TrimMarkPositions {
    TrimMarkPositions::new(
        cut_positions.vertical.clone(),
        cut_positions.horizontal.clone(),
        spread_content_bounds.to_vec(),
    )
}

#[cfg(test)]
#[path = "tests/trim_bounds_tests.rs"]
mod tests;
