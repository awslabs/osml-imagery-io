//! Composite dataset reader for multi-file pyramids.
//!
//! This module provides a `CompositeDatasetReader` that merges assets from
//! multiple format-specific readers into a single reader. It is used by
//! `IO::open()` to support R-set multi-file pyramids where each file
//! represents a different resolution level.

mod reader;
mod wrapper;

pub use reader::CompositeDatasetReader;
pub use wrapper::OverviewAssetWrapper;
