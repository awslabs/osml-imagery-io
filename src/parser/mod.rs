//! Data-driven binary parser infrastructure.
//!
//! This module provides a declarative approach to parsing and writing binary data
//! using YAML-based structure definitions (Kaitai Struct-compatible subset).
//! The parser is optimized for NITF's ASCII-centric design where most fields are
//! fixed-width character strings (BCS-A, BCS-N, ECS-A).
//!
//! # Key Components
//!
//! - [`StructureDefinition`] - Parsed structure definition from KSY files
//! - [`StructureAccessor`] - Lazy map-like interface for reading parsed values
//! - [`StructureWriter`] - Interface for encoding values into binary format
//! - [`StructureRegistry`] - Manages loading and caching of structure definitions
//! - [`ExpressionEvaluator`] - Evaluates expressions for computed values and conditions

mod accessor;
mod definition;
pub mod encoding;
mod error;
mod expression;
mod registry;
mod types;
mod value;
pub mod writer;

pub use accessor::StructureAccessor;
pub use definition::DefinitionLoader;
pub use error::{AccessError, ConversionError, ExpressionError, LoadError, WriteError};
pub use expression::{
    BinaryOperator, EvalContext, EvalResult, Expression, ExpressionEvaluator, Literal,
    SpecialVariable, UnaryOperator,
};
pub use registry::StructureRegistry;
pub use types::{
    Encoding, Endian, EnumDefinition, FieldDefinition, FieldType, RepeatSpec, SizeSpec,
    StructureDefinition,
};
pub use value::{StructValue, Value};
pub use writer::StructureWriter;
