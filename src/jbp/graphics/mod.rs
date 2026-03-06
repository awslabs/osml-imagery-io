//! Graphics segment support for JBP/NITF files.
//!
//! This module provides typed access to graphic segment subheaders and metadata.
//! Graphic segments contain CGM (Computer Graphics Metafile) vector graphics data
//! with associated metadata for display layering and positioning.
//!
//! # Key Components
//!
//! - [`GraphicSubheaderFacade`] - Typed access to graphic subheader fields
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::graphics::GraphicSubheaderFacade;
//!
//! let facade = GraphicSubheaderFacade::from_bytes(subheader_bytes, &registry, format)?;
//! let sdlvl = facade.sdlvl()?;  // Display level (001-999)
//! let salvl = facade.salvl()?;  // Attachment level (000-998)
//! let (row, col) = facade.sloc()?;  // Location offset
//! ```

mod facade;

pub use facade::GraphicSubheaderFacade;
