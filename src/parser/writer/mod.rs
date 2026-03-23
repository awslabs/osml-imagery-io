//! Structure writer for encoding values into binary format.
//!
//! The [`StructureWriter`] uses streaming mode for sequential field writes.

mod encode;
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
use streaming::{
    get_expected_streaming_field, get_last_written_field, get_repeat_count,
    get_streaming_field_size,
};

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
    /// Array of values (for repeated fields)
    Array(Vec<WriteValue>),
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

impl<T: Into<WriteValue>> From<Vec<T>> for WriteValue {
    fn from(v: Vec<T>) -> Self {
        WriteValue::Array(v.into_iter().map(Into::into).collect())
    }
}

/// Writer for encoding values according to a structure definition.
///
/// Fields must be written in definition order. For repeated fields,
/// pass a `WriteValue::Array` or write elements sequentially with
/// indexed paths (`field_0`, `field_1`, ...).
pub struct StructureWriter {
    /// The structure definition
    definition: Arc<StructureDefinition>,
    /// Output buffer
    buffer: Vec<u8>,
    /// Current write position
    position: usize,
    /// Fields that have been written (base names)
    written: HashSet<String>,
    /// Expression evaluator for size expressions
    evaluator: ExpressionEvaluator,
    /// Values written so far (for expression evaluation)
    written_values: HashMap<String, WriteValue>,
    /// Next expected field index
    next_field_index: usize,
    /// Count of elements written for current repeated field
    current_repeat_written: usize,
}

impl StructureWriter {
    /// Create a new streaming writer.
    ///
    /// Fields must be written in definition order.
    pub fn new(definition: Arc<StructureDefinition>) -> Self {
        Self {
            definition,
            buffer: Vec::new(),
            position: 0,
            written: HashSet::new(),
            evaluator: ExpressionEvaluator::new(),
            written_values: HashMap::new(),
            next_field_index: 0,
            current_repeat_written: 0,
        }
    }

    /// Create streaming writer (alias for `new` for backward compatibility).
    pub fn new_streaming(definition: Arc<StructureDefinition>) -> Self {
        Self::new(definition)
    }

    /// Write a value to a field.
    ///
    /// For repeated fields, pass a `WriteValue::Array` with all elements,
    /// or write elements one at a time in order.
    pub fn set(&mut self, path: &str, value: impl Into<WriteValue>) -> Result<(), WriteError> {
        let write_value = value.into();

        // Check if this is an array value for a repeated field
        if let WriteValue::Array(ref elements) = write_value {
            let field_name = path.to_string();
            let field = self.find_field(&field_name)?;
            if field.repeat.is_some() {
                return self.write_array(&field_name, &field.clone(), elements.clone());
            }
        }

        // Parse the path to handle indexed fields (e.g., "items_0")
        let (field_name, index) = self.parse_path(path);
        let field = self.find_field(&field_name)?;

        self.write_streaming(&field_name, index, &field.clone(), write_value, path)
    }

    /// Check if a field has been written.
    pub fn is_set(&self, path: &str) -> bool {
        self.written.contains(path)
    }

    /// Finalize and return encoded bytes.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        for field in &self.definition.fields {
            if field.condition.is_some() {
                continue;
            }

            if let Some(ref repeat) = field.repeat {
                let ctx = self.build_eval_context();
                let expected_count = get_repeat_count(repeat, &field.id, &self.evaluator, &ctx)?;
                if expected_count > 0 && !self.written.contains(&field.id) {
                    return Err(WriteError::MissingRequired {
                        path: field.id.clone(),
                    });
                }
            } else if !self.written.contains(&field.id) {
                return Err(WriteError::MissingRequired {
                    path: field.id.clone(),
                });
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
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    // ==================== Private Helper Methods ====================

    /// Parse a field path into (field_name, optional_index).
    fn parse_path(&self, path: &str) -> (String, Option<usize>) {
        if let Some(underscore_pos) = path.rfind('_') {
            let potential_index = &path[underscore_pos + 1..];
            if let Ok(index) = potential_index.parse::<usize>() {
                let field_name = path[..underscore_pos].to_string();
                // Only treat as indexed if the base name is a known field
                if self.definition.fields.iter().any(|f| f.id == field_name) {
                    return (field_name, Some(index));
                }
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

    /// Write an array of values for a repeated field.
    fn write_array(
        &mut self,
        field_name: &str,
        field: &FieldDefinition,
        elements: Vec<WriteValue>,
    ) -> Result<(), WriteError> {
        for (i, elem) in elements.into_iter().enumerate() {
            let path = format!("{}_{}", field_name, i);
            self.write_streaming(field_name, Some(i), field, elem, &path)?;
        }
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
        let expected_field = get_expected_streaming_field(&self.definition, self.next_field_index)?;

        let is_expected = if let Some(idx) = index {
            expected_field.id == field_name && idx == self.current_repeat_written
        } else {
            expected_field.id == field_name && expected_field.repeat.is_none()
        };

        if !is_expected {
            return Err(WriteError::OutOfOrder {
                path: path.to_string(),
                expected_after: get_last_written_field(&self.definition, self.next_field_index),
            });
        }

        let ctx = self.build_eval_context();
        let size = get_streaming_field_size(field, &self.evaluator, &ctx)?;

        let encoded = encode_value(&value, field, size, self.definition.endian, path)?;

        self.buffer.extend_from_slice(&encoded);
        self.position += size;

        // Track written values for eval context
        let eval_key = field_name.to_string();
        self.written_values.insert(eval_key, value);

        // Handle repeat advancement
        if let Some(_idx) = index {
            self.current_repeat_written += 1;
            // Check if all elements written
            if let Some(ref repeat) = field.repeat {
                let expected_count = get_repeat_count(repeat, &field.id, &self.evaluator, &ctx)?;
                if self.current_repeat_written >= expected_count {
                    self.written.insert(field_name.to_string());
                    self.next_field_index += 1;
                    self.current_repeat_written = 0;
                }
            }
        } else {
            self.written.insert(field_name.to_string());
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
                WriteValue::Bytes(_) | WriteValue::Array(_) => {}
            }
        }

        ctx
    }
}


#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
