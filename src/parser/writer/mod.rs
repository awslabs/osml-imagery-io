//! Structure writer for encoding values into binary format.
//!
//! The [`StructureWriter`] supports both fixed-size mode (out-of-order writes)
//! and streaming mode (sequential writes) for encoding binary data.

mod encode;
mod fixed;
mod integer;
mod streaming;
mod validation;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use super::error::WriteError;
use super::expression::{EvalContext, EvalResult, ExpressionEvaluator};
use super::types::{FieldDefinition, StructureDefinition};

use encode::encode_value;
use fixed::calculate_fixed_layout;
use streaming::{
    get_expected_streaming_field, get_last_written_field, get_repeat_count,
    get_streaming_field_size, is_expected_index, should_advance_streaming_position,
};

/// Writing mode for the structure writer.
#[derive(Debug, Clone)]
pub enum WriterMode {
    /// Fixed-size buffer, fields can be written in any order
    Fixed { size: usize },
    /// Streaming mode, fields must be written in order
    Streaming,
}

/// Value types accepted for writing.
#[derive(Debug, Clone)]
pub enum WriteValue {
    /// String value
    String(String),
    /// Raw bytes
    Bytes(Vec<u8>),
    /// Signed integer
    Integer(i64),
    /// Unsigned integer
    Unsigned(u64),
    /// Floating-point value
    Float(f64),
}

impl From<String> for WriteValue {
    fn from(s: String) -> Self {
        WriteValue::String(s)
    }
}

impl From<&str> for WriteValue {
    fn from(s: &str) -> Self {
        WriteValue::String(s.to_string())
    }
}

impl From<Vec<u8>> for WriteValue {
    fn from(bytes: Vec<u8>) -> Self {
        WriteValue::Bytes(bytes)
    }
}

impl From<&[u8]> for WriteValue {
    fn from(bytes: &[u8]) -> Self {
        WriteValue::Bytes(bytes.to_vec())
    }
}

impl From<i64> for WriteValue {
    fn from(n: i64) -> Self {
        WriteValue::Integer(n)
    }
}

impl From<i32> for WriteValue {
    fn from(n: i32) -> Self {
        WriteValue::Integer(n as i64)
    }
}

impl From<u64> for WriteValue {
    fn from(n: u64) -> Self {
        WriteValue::Unsigned(n)
    }
}

impl From<u32> for WriteValue {
    fn from(n: u32) -> Self {
        WriteValue::Unsigned(n as u64)
    }
}

impl From<f64> for WriteValue {
    fn from(n: f64) -> Self {
        WriteValue::Float(n)
    }
}

/// Writer for encoding values according to a structure definition.
pub struct StructureWriter {
    /// The structure definition
    definition: Arc<StructureDefinition>,
    /// Output buffer
    buffer: Vec<u8>,
    /// Current write position (for streaming mode)
    position: usize,
    /// Fields that have been written
    written: HashSet<String>,
    /// Writing mode
    mode: WriterMode,
    /// Cached field offsets: field_id -> (offset, size)
    field_offsets: HashMap<String, (usize, usize)>,
    /// Expression evaluator for size expressions
    evaluator: ExpressionEvaluator,
    /// Values written so far (for expression evaluation)
    written_values: HashMap<String, WriteValue>,
    /// Next expected field index for streaming mode
    next_field_index: usize,
}

impl StructureWriter {
    /// Create writer for fixed-size structure.
    ///
    /// Pre-allocates a buffer of the correct size based on the structure definition.
    /// Fields can be written in any order.
    pub fn new_fixed(definition: Arc<StructureDefinition>) -> Result<Self, WriteError> {
        // Calculate total size and field offsets
        let (total_size, field_offsets) = calculate_fixed_layout(&definition)?;

        // Pre-allocate buffer filled with zeros
        let buffer = vec![0u8; total_size];

        Ok(Self {
            definition,
            buffer,
            position: 0,
            written: HashSet::new(),
            mode: WriterMode::Fixed { size: total_size },
            field_offsets,
            evaluator: ExpressionEvaluator::new(),
            written_values: HashMap::new(),
            next_field_index: 0,
        })
    }

    /// Create streaming writer for variable-size structure.
    ///
    /// Fields must be written in definition order.
    pub fn new_streaming(definition: Arc<StructureDefinition>) -> Self {
        Self {
            definition,
            buffer: Vec::new(),
            position: 0,
            written: HashSet::new(),
            mode: WriterMode::Streaming,
            field_offsets: HashMap::new(),
            evaluator: ExpressionEvaluator::new(),
            written_values: HashMap::new(),
            next_field_index: 0,
        }
    }

    /// Write a value to a field.
    ///
    /// In fixed-size mode, fields can be written in any order.
    /// In streaming mode, fields must be written in definition order.
    pub fn set(&mut self, path: &str, value: impl Into<WriteValue>) -> Result<(), WriteError> {
        let write_value = value.into();

        // Parse the path to handle indexed fields (e.g., "items_0")
        let (field_name, index) = self.parse_path(path);

        // Find the field definition
        let field = self.find_field(&field_name)?;

        match &self.mode {
            WriterMode::Fixed { .. } => {
                self.write_fixed(&field_name, index, &field.clone(), write_value, path)
            }
            WriterMode::Streaming => {
                self.write_streaming(&field_name, index, &field.clone(), write_value, path)
            }
        }
    }

    /// Check if a field has been written.
    pub fn is_set(&self, path: &str) -> bool {
        self.written.contains(path)
    }

    /// Finalize and return encoded bytes.
    ///
    /// Returns MissingRequired error if any required fields have not been written.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        // Check that all required fields have been written
        for field in &self.definition.fields {
            // Skip conditional fields - they may not be required
            if field.condition.is_some() {
                continue;
            }

            // Check if field is repeated
            if let Some(ref repeat) = field.repeat {
                // For repeated fields, check if at least the expected count was written
                let ctx = self.build_eval_context();
                let expected_count = get_repeat_count(repeat, &field.id, &self.evaluator, &ctx)?;
                for i in 0..expected_count {
                    let indexed_path = format!("{}_{}", field.id, i);
                    if !self.written.contains(&indexed_path) {
                        return Err(WriteError::MissingRequired {
                            path: indexed_path,
                        });
                    }
                }
            } else {
                // Non-repeated field
                if !self.written.contains(&field.id) {
                    return Err(WriteError::MissingRequired {
                        path: field.id.clone(),
                    });
                }
            }
        }

        Ok(self.buffer)
    }

    /// Write to an output stream.
    pub fn write_to<W: Write>(self, mut writer: W) -> Result<usize, WriteError> {
        let bytes = self.finish()?;
        let len = bytes.len();
        writer
            .write_all(&bytes)
            .map_err(|e| WriteError::ValidationError {
                path: "output".to_string(),
                message: e.to_string(),
            })?;
        Ok(len)
    }

    /// Get the current buffer contents without validation.
    ///
    /// This is useful for testing or when you want to inspect partial writes.
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    // ==================== Private Helper Methods ====================

    /// Parse a field path into (field_name, optional_index).
    fn parse_path(&self, path: &str) -> (String, Option<usize>) {
        // Check if the path has an underscore index (e.g., "items_0")
        if let Some(underscore_pos) = path.rfind('_') {
            let potential_index = &path[underscore_pos + 1..];
            if let Ok(index) = potential_index.parse::<usize>() {
                let field_name = path[..underscore_pos].to_string();
                return (field_name, Some(index));
            }
        }
        (path.to_string(), None)
    }

    /// Find a field definition by name.
    fn find_field(&self, name: &str) -> Result<FieldDefinition, WriteError> {
        self.definition
            .fields
            .iter()
            .find(|f| f.id == name)
            .cloned()
            .ok_or_else(|| WriteError::ValidationError {
                path: name.to_string(),
                message: "Unknown field".to_string(),
            })
    }

    /// Write a field in fixed-size mode.
    fn write_fixed(
        &mut self,
        field_name: &str,
        index: Option<usize>,
        field: &FieldDefinition,
        value: WriteValue,
        path: &str,
    ) -> Result<(), WriteError> {
        // Get the offset and size for this field
        let cache_key = match index {
            Some(i) => format!("{}_{}", field_name, i),
            None => field_name.to_string(),
        };

        let (offset, size) = self
            .field_offsets
            .get(&cache_key)
            .copied()
            .ok_or_else(|| WriteError::ValidationError {
                path: path.to_string(),
                message: "Field not found in fixed layout".to_string(),
            })?;

        // Encode the value
        let encoded = encode_value(&value, field, size, self.definition.endian, path)?;

        // Write to buffer
        self.buffer[offset..offset + size].copy_from_slice(&encoded);

        // Mark as written
        self.written.insert(cache_key.clone());
        self.written_values.insert(cache_key, value);

        Ok(())
    }

    /// Write a field in streaming mode.
    fn write_streaming(
        &mut self,
        field_name: &str,
        index: Option<usize>,
        field: &FieldDefinition,
        value: WriteValue,
        path: &str,
    ) -> Result<(), WriteError> {
        // Find the expected field at current position
        let expected_field = get_expected_streaming_field(&self.definition, self.next_field_index)?;

        // Check if we're writing the expected field
        let is_expected = if let Some(idx) = index {
            // For indexed fields, check both field name and index
            expected_field.id == field_name && is_expected_index(&expected_field, idx, &self.written)
        } else {
            expected_field.id == field_name && expected_field.repeat.is_none()
        };

        if !is_expected {
            return Err(WriteError::OutOfOrder {
                path: path.to_string(),
                expected_after: get_last_written_field(&self.definition, self.next_field_index),
            });
        }

        // Get the size for this field
        let ctx = self.build_eval_context();
        let size = get_streaming_field_size(field, &self.evaluator, &ctx)?;

        // Encode the value
        let encoded = encode_value(&value, field, size, self.definition.endian, path)?;

        // Append to buffer
        self.buffer.extend_from_slice(&encoded);
        self.position += size;

        // Mark as written
        let cache_key = match index {
            Some(i) => format!("{}_{}", field_name, i),
            None => field_name.to_string(),
        };
        self.written.insert(cache_key.clone());
        self.written_values.insert(cache_key, value);

        // Advance to next field if this completes the current field
        let ctx = self.build_eval_context();
        if should_advance_streaming_position(field, index, &self.written, &self.evaluator, &ctx) {
            self.next_field_index += 1;
        }

        Ok(())
    }

    /// Build an evaluation context from written values.
    fn build_eval_context(&self) -> EvalContext {
        let mut ctx = EvalContext::new();

        for (name, value) in &self.written_values {
            match value {
                WriteValue::Integer(n) => {
                    ctx.fields.insert(name.clone(), EvalResult::Integer(*n));
                }
                WriteValue::Unsigned(n) => {
                    ctx.fields
                        .insert(name.clone(), EvalResult::Integer(*n as i64));
                }
                WriteValue::String(s) => {
                    ctx.fields
                        .insert(name.clone(), EvalResult::String(s.clone()));
                }
                WriteValue::Float(f) => {
                    ctx.fields.insert(name.clone(), EvalResult::Float(*f));
                }
                WriteValue::Bytes(_) => {
                    // Bytes don't have a direct EvalResult representation
                }
            }
        }

        ctx
    }
}


#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
