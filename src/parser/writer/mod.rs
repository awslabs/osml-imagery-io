//! Structure writer for encoding values into binary format.
//!
//! The [`StructureWriter`] uses streaming mode for sequential field writes.

mod encode;
pub(crate) mod integer;
mod streaming;
mod validation;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use super::error::WriteError;
use super::expression::{EvalContext, EvalResult, ExpressionEvaluator, Node};
use super::types::{FieldDefinition, RepeatSpec, SizeSpec, StructureDefinition};

use encode::encode_value;
use streaming::{
    advance_past_false_conditions, get_expected_streaming_field, get_last_written_field,
    get_repeat_count, get_streaming_field_size,
};
use validation::validate_encoding;

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
/// pass a `WriteValue::Array` holding all elements.
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
    /// Field nodes written so far, for the node-context tree. Scalar fields are
    /// lowered to `Node::Scalar` leaves at write time (replacing the old
    /// `written_values: HashMap<String, WriteValue>` re-derivation); array/struct
    /// fields are registered as `Node::Array`/`Node::Struct` by
    /// [`Self::set_node`] so an expression can navigate an enclosing repeated
    /// group (`IMAGE_RECORDS[image_index].NCOLCB`). The evaluator context is
    /// built directly from these nodes plus [`Self::inherited`].
    written_nodes: HashMap<String, Node>,
    /// Next expected field index
    next_field_index: usize,
    /// Count of elements written for current repeated field
    current_repeat_written: usize,
    /// Caller-supplied element counts for `repeat: eos`/`until` fields. Their
    /// count is not derivable from a count field, so on encode the length of
    /// the supplied value list is authoritative. Keyed by field id, recorded
    /// when the array is set. See [`Self::resolve_repeat_count`].
    eos_until_counts: HashMap<String, usize>,
    /// When true, enforce strict spec-compliant encoding validation on write.
    /// When false (default), numeric fields accept any printable ASCII.
    strict_encoding: bool,
    /// Field nodes inherited from the enclosing scope(s) when this writer
    /// serializes a nested structure. Seeded once before writing and overlaid by
    /// locally-written values in [`Self::build_eval_context`] (local wins). This
    /// is the "stack" of enclosing values that nested `size`/`repeat-expr`
    /// expressions (including `_root.`/`_parent.` navigators and an enclosing
    /// repeated group indexed by `ARRAY[i].FIELD`) resolve against. Carries
    /// scalar leaves and navigable `Array`/`Struct` nodes.
    inherited: HashMap<String, Node>,
}

impl StructureWriter {
    /// Create a new streaming writer.
    ///
    /// Fields must be written in definition order. By default, numeric field
    /// encoding is relaxed to accept any printable ASCII (BCS-A range).
    /// Call [`Self::set_strict_encoding`] to enforce spec-exact validation.
    pub fn new(definition: Arc<StructureDefinition>) -> Self {
        Self {
            definition,
            buffer: Vec::new(),
            position: 0,
            written: HashSet::new(),
            evaluator: ExpressionEvaluator::new(),
            written_nodes: HashMap::new(),
            next_field_index: 0,
            current_repeat_written: 0,
            eos_until_counts: HashMap::new(),
            strict_encoding: false,
            inherited: HashMap::new(),
        }
    }

    /// Create streaming writer (alias for `new` for backward compatibility).
    pub fn new_streaming(definition: Arc<StructureDefinition>) -> Self {
        Self::new(definition)
    }

    /// Set strict encoding validation mode.
    ///
    /// When true, all fields are validated against their declared encoding
    /// exactly (e.g. BCS-NPI rejects `+`, `-`, `.`). When false (default),
    /// numeric fields accept any printable ASCII.
    pub fn set_strict_encoding(&mut self, strict: bool) {
        self.strict_encoding = strict;
    }

    /// Seed the values inherited from the enclosing scope(s).
    ///
    /// Used when this writer serializes a nested structure: the caller passes a
    /// snapshot of the enclosing scope's scalar values so this writer's nested
    /// `size`/`repeat-expr` expressions can reference them. Locally-written
    /// values overlay these in [`Self::build_eval_context`] (local wins).
    pub fn set_inherited_context(&mut self, inherited: HashMap<String, Node>) {
        self.inherited = inherited;
    }

    /// Register an intermediary (`Array`/`Struct`) node for a field.
    ///
    /// Scalar leaves are recorded automatically as fields are written; this lets
    /// [`crate::parser::codec`] additionally register the array/struct shape of a
    /// repeated or nested field built from the input dict, so an expression can
    /// navigate an enclosing group (`IMAGE_RECORDS[image_index].NCOLCB`) on the
    /// write path exactly as on read.
    pub fn set_node(&mut self, name: impl Into<String>, node: Node) {
        self.written_nodes.insert(name.into(), node);
    }

    /// Snapshot the scalar values written so far, as an eval context map.
    ///
    /// This is the enclosing scope a nested sub-writer inherits: it combines the
    /// values inherited by this writer with everything written locally so far
    /// (local wins), so the snapshot reflects the full flat scope visible at the
    /// current point. Only scalar values participate, matching
    /// [`Self::build_eval_context`].
    pub fn eval_snapshot(&self) -> HashMap<String, Node> {
        self.build_eval_context().snapshot()
    }

    /// Write a value to a field.
    ///
    /// For repeated fields, pass a `WriteValue::Array` with all elements.
    pub fn set(&mut self, path: &str, value: impl Into<WriteValue>) -> Result<(), WriteError> {
        let write_value = value.into();

        // Array values for repeated fields: expand internally
        if let WriteValue::Array(ref elements) = write_value {
            let field_name = path.to_string();
            let field = self.find_field(&field_name)?;
            if field.repeat.is_some() {
                return self.write_array(&field_name, &field.clone(), elements.clone());
            }
        }

        let field = self.find_field(path)?;
        self.write_streaming(path, None, &field.clone(), write_value, path)
    }

    /// Check if a field has been written.
    pub fn is_set(&self, path: &str) -> bool {
        self.written.contains(path)
    }

    /// Determine whether a field should be written given what has been written so far.
    ///
    /// Returns `Ok(true)` for an unconditional field and for a conditional field
    /// whose `if:` evaluates to `Ok(Boolean(true))`; `Ok(false)` when it evaluates
    /// to `Ok(Boolean(false))` (the field is legitimately absent).
    ///
    /// **Total-or-error:** a condition the evaluator cannot resolve — a reference
    /// to a value not present in the write context — returns `Err`, not "active".
    /// The prior code treated any non-`false` result (including `Err`) as active,
    /// which let an unresolvable condition silently pass; that masked the same
    /// class of desync the read path had. `Ok(false)` still means "absent"; an
    /// unresolvable or non-boolean condition is now a propagated error.
    ///
    /// This lets [`crate::parser::codec::encode_fields`] gate writes on the same
    /// notion of presence the cursor uses, instead of inferring presence from
    /// which keys happen to be in the input map.
    pub fn is_field_active(&self, field: &FieldDefinition) -> Result<bool, WriteError> {
        match &field.condition {
            None => Ok(true),
            Some(condition) => {
                let ctx = self.build_eval_context();
                match self.evaluator.evaluate(condition, &ctx) {
                    Ok(EvalResult::Boolean(true)) => Ok(true),
                    Ok(EvalResult::Boolean(false)) => Ok(false),
                    Ok(_) => Err(WriteError::ValidationError {
                        path: field.id.clone(),
                        message: "Condition did not evaluate to boolean".to_string(),
                    }),
                    Err(e) => Err(WriteError::ValidationError {
                        path: field.id.clone(),
                        message: format!("Failed to evaluate condition: {}", e),
                    }),
                }
            }
        }
    }

    /// Finalize and return encoded bytes.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        for field in &self.definition.fields {
            if field.condition.is_some() {
                continue;
            }

            if let Some(ref repeat) = field.repeat {
                let ctx = self.build_eval_context();
                let expected_count = self.resolve_repeat_count(repeat, &field.id, &ctx)?;
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

    /// Resolve how many elements a repeated field emits/expects.
    ///
    /// For `Count`/`Expression` repeats the count comes from the definition (via
    /// [`get_repeat_count`]). For `Eos`/`Until` the count is not derivable from a
    /// count field, so the caller-supplied value list length recorded in
    /// [`Self::eos_until_counts`] is authoritative; if no list was supplied the
    /// field is treated as absent (count 0), matching `finish()`'s
    /// "list present ⇒ satisfied" check.
    fn resolve_repeat_count(
        &self,
        repeat: &RepeatSpec,
        field_name: &str,
        ctx: &EvalContext,
    ) -> Result<usize, WriteError> {
        match repeat {
            RepeatSpec::Eos | RepeatSpec::Until(_) => {
                Ok(self.eos_until_counts.get(field_name).copied().unwrap_or(0))
            }
            _ => get_repeat_count(repeat, field_name, &self.evaluator, ctx),
        }
    }

    /// Write an array of values for a repeated field.
    fn write_array(
        &mut self,
        field_name: &str,
        field: &FieldDefinition,
        elements: Vec<WriteValue>,
    ) -> Result<(), WriteError> {
        // For eos/until repeats the supplied list length is the authoritative
        // element count (there is no count field to validate against). Record it
        // before emitting so per-element advancement and finish() agree.
        if matches!(
            field.repeat,
            Some(RepeatSpec::Eos) | Some(RepeatSpec::Until(_))
        ) {
            self.eos_until_counts
                .insert(field_name.to_string(), elements.len());
        }
        if elements.is_empty() {
            // Zero-element array: verify ordering then advance past this field.
            let ctx = self.build_eval_context();
            self.next_field_index = advance_past_false_conditions(
                &self.definition,
                self.next_field_index,
                &self.evaluator,
                &ctx,
            );
            let expected_field =
                get_expected_streaming_field(&self.definition, self.next_field_index)?;
            if expected_field.id != field_name {
                return Err(WriteError::OutOfOrder {
                    path: field_name.to_string(),
                    expected_after: get_last_written_field(&self.definition, self.next_field_index),
                });
            }
            self.written.insert(field_name.to_string());
            self.next_field_index += 1;
            return Ok(());
        }
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
        let ctx = self.build_eval_context();
        self.next_field_index = advance_past_false_conditions(
            &self.definition,
            self.next_field_index,
            &self.evaluator,
            &ctx,
        );

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

        let size = get_streaming_field_size(field, &self.evaluator, &ctx)?;

        let encoded = if matches!(field.size, SizeSpec::Eos) {
            encode_eos_value(&value, field, path, self.strict_encoding)?
        } else {
            encode_value(
                &value,
                field,
                size,
                self.definition.endian,
                path,
                self.strict_encoding,
            )?
        };

        let actual_size = encoded.len();
        self.buffer.extend_from_slice(&encoded);
        self.position += actual_size;

        // Track the written scalar leaf for the eval context. Only scalars
        // participate here; `Bytes`/`Array` contribute no scalar leaf (a nested
        // struct is already collapsed to `Bytes` by the recursion). The array/
        // struct *shape* of a repeated or nested field is registered separately
        // by the codec via [`Self::set_node`], built from the input dict. A
        // repeated scalar field records its most recent element under the base
        // name, matching the prior `written_values` behavior.
        if let Some(leaf) = write_value_to_scalar(&value) {
            self.written_nodes
                .insert(field_name.to_string(), Node::Scalar(leaf));
        }

        // Handle repeat advancement
        if let Some(_idx) = index {
            self.current_repeat_written += 1;
            // Check if all elements written
            if let Some(ref repeat) = field.repeat {
                let expected_count = self.resolve_repeat_count(repeat, &field.id, &ctx)?;
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

    /// Build an evaluation context from the scalar values written so far.
    ///
    /// The node-context tree is built directly from [`Self::written_scalars`]
    /// (leaves lowered at write time) plus [`Self::inherited`]. Inherited
    /// (enclosing-scope) values are seeded first; locally-written values overlay
    /// them on name collision so a local field shadows an inherited one of the
    /// same name.
    fn build_eval_context(&self) -> EvalContext {
        let mut ctx = EvalContext::new();

        // Seed compile-time consts (const map/scalar nodes) first. The loader
        // stamps file-level consts onto every nested type, so a sub-writer for a
        // nested type also seeds them from its own `definition.consts` — a
        // nested `MAP[KEY]` subscript resolves on the write path too. Inherited
        // and locally-written scalars overlay these (a same-named field shadows
        // the const).
        for (name, node) in &self.definition.consts {
            ctx.insert_node(name.clone(), node.clone());
        }

        for (name, node) in &self.inherited {
            ctx.insert_node(name.clone(), node.clone());
        }

        for (name, node) in &self.written_nodes {
            ctx.insert_node(name.clone(), node.clone());
        }

        ctx
    }
}

/// Lower a [`WriteValue`] to the scalar [`EvalResult`] leaf the evaluator can
/// consume, or `None` for non-scalar values.
///
/// `Bytes` (including a nested struct already collapsed to bytes) and `Array`
/// (a repeated group) contribute no scalar leaf — matching the prior
/// `build_eval_context` match, which skipped both.
fn write_value_to_scalar(value: &WriteValue) -> Option<EvalResult> {
    match value {
        WriteValue::Integer(n) => Some(EvalResult::Integer(*n)),
        WriteValue::Unsigned(n) => Some(EvalResult::Integer(*n as i64)),
        WriteValue::String(s) => Some(EvalResult::String(s.clone())),
        WriteValue::Float(f) => Some(EvalResult::Float(*f)),
        WriteValue::Bytes(_) | WriteValue::Array(_) => None,
    }
}

/// Encode a value for a size-eos field (writes verbatim without padding or truncation).
fn encode_eos_value(
    value: &WriteValue,
    field: &FieldDefinition,
    path: &str,
    strict: bool,
) -> Result<Vec<u8>, WriteError> {
    let bytes = match value {
        WriteValue::String(s) => {
            let b = s.as_bytes();
            if let Some(encoding) = field.encoding {
                validate_encoding(b, encoding, path, strict)?;
            }
            b.to_vec()
        }
        WriteValue::Bytes(b) => b.clone(),
        _ => {
            return Err(WriteError::ConversionError {
                path: path.to_string(),
                message: format!(
                    "Cannot write {:?} to size-eos field (expected String or Bytes)",
                    std::mem::discriminant(value)
                ),
            });
        }
    };
    Ok(bytes)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
