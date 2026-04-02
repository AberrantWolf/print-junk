//! Layout calculation modules for imposition
//!
//! This module provides the spread-based layout system where `Spread` (verso + recto)
//! is the fundamental unit. Arrangements are built by composition:
//! - Folio = 1 spread
//! - Quarto = 2 spreads stacked (top rotated 180 degrees)
//! - Octavo = 4 spreads in 2x2 (top row rotated 180 degrees)
//!
//! Key modules:
//! - `spread` - Spread content area calculation
//! - `arrangement` - Compositional layout of spreads
//! - `page_order` - Page number assignment to spreads
//! - `trim_bounds` - Unified trim mark calculation
//! - `placement` - Page placement within spreads
//! - `types` - Core data types

pub mod arrangement;
pub mod page_order;
pub mod placement;
pub mod spread;
pub mod trim_bounds;
pub mod types;

// Re-export spread-based types from arrangement
pub use arrangement::{
    ArrangementConfig, CutPositions, calculate_cut_edges, calculate_spread_positions,
};

// Re-export page ordering functions
pub use page_order::{
    SignaturePageAssignment, apply_page_assignments, assign_pages_to_spreads,
    calculate_padded_page_count, calculate_signature_count,
};

// Re-export spread functions
pub use spread::{
    calculate_spread_content, create_folio_spread, create_octavo_spreads, create_quarto_spreads,
};

// Re-export placement functions
pub use placement::calculate_spread_placements;

// Re-export trim bounds types
pub use trim_bounds::{TrimContentBounds, TrimMarkPositions, UnifiedTrimBounds};

// Re-export all core types
pub use types::*;
