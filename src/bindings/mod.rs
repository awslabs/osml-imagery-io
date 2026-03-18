//! High-performance reading and writing of geospatial imagery datasets.
//!
//! The ``aws.osml.io`` package provides access to NITF, GeoTIFF, and other
//! geospatial formats through a unified Python API. Open a dataset with
//! :class:`IO`, then retrieve individual assets — images, metadata, text,
//! structured data, or vector graphics — through a :class:`DatasetReader` or
//! :class:`DatasetWriter` context manager.
//!
//! Key classes:
//!
//! * :class:`IO` — factory for opening datasets in read or write mode.
//! * :class:`DatasetReader` — read access to an existing dataset (context manager).
//! * :class:`DatasetWriter` — write access to a new or existing dataset (context manager).
//! * :class:`ImageAssetProvider` — blocked/tiled image access returning NumPy arrays.
//! * :class:`MetadataProvider` — key-value metadata access.
//! * :class:`TextAssetProvider` — text asset access.
//! * :class:`DataAssetProvider` — structured data (XML/JSON) access.
//! * :class:`GraphicsAssetProvider` — vector graphics (CGM) access.
//! * :class:`StructureRegistry`, :class:`StructureAccessor`, :class:`StructureWriter` — binary structure parsing and encoding.

pub mod asset;
pub mod buffered_image;
pub mod buffered_metadata;
pub mod buffered_text;
pub mod data;
pub mod graphics;
pub mod image;
pub mod io;
pub mod metadata;
pub mod parser;
pub mod reader;
pub mod text;
pub mod writer;

pub use asset::PyAssetProvider;
pub use buffered_image::PyBufferedImageAssetProvider;
pub use buffered_metadata::PyBufferedMetadataProvider;
pub use buffered_text::PyBufferedTextAssetProvider;
pub use data::PyDataAssetProvider;
pub use graphics::PyGraphicsAssetProvider;
pub use image::PyImageAssetProvider;
pub use io::IO;
pub use metadata::PyMetadataProvider;
pub use parser::{PyStructureAccessor, PyStructureDefinition, PyStructureRegistry, PyStructureWriter, PyValue};
pub use reader::PyDatasetReader;
pub use text::PyTextAssetProvider;
pub use writer::PyDatasetWriter;
