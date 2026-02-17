//! Lazy accessor for reading structure fields from binary data.
//!
//! The [`StructureAccessor`] provides a map-like interface for reading parsed values
//! on-demand, with offset caching for repeated access efficiency.
//!
//! # Submodules
//!
//! - [`offset`] - Offset calculation for field locations
//! - [`read`] - Value reading from binary data
//! - [`context`] - Evaluation context building
//! - [`iterator`] - Field iteration support

mod context;
mod iterator;
mod offset;
mod read;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::parser::error::AccessError;
use crate::parser::expression::{EvalContext, EvalResult, ExpressionEvaluator};
use crate::parser::types::{
    Encoding, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
};
use crate::parser::value::Value;

pub use iterator::FieldIterator;

use context::{add_value_to_context_impl, build_context_from_definition};
use read::{read_field_value_from_bytes, read_signed, read_unsigned};

/// Information about a field's location and type.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// Field type
    pub field_type: FieldType,
    /// Size in bytes
    pub size: usize,
    /// Offset from structure start
    pub offset: usize,
    /// Character encoding (if applicable)
    pub encoding: Option<Encoding>,
    /// Documentation string
    pub doc: Option<String>,
}


/// Lazy accessor for reading structure fields from binary data.
///
/// Fields are parsed on-demand when accessed, with computed offsets cached
/// for repeated access efficiency.
pub struct StructureAccessor<'a> {
    /// The structure definition
    pub(crate) definition: Arc<StructureDefinition>,
    /// Source data buffer
    data: &'a [u8],
    /// Cached field offsets: path -> (offset, size)
    offset_cache: RefCell<HashMap<String, (usize, usize)>>,
    /// Expression evaluator
    pub(crate) evaluator: ExpressionEvaluator,
    /// Parent accessor for nested structures
    #[allow(dead_code)]
    parent: Option<&'a StructureAccessor<'a>>,
    /// Base offset within parent data
    #[allow(dead_code)]
    base_offset: usize,
}

impl<'a> StructureAccessor<'a> {
    /// Create accessor from definition and data buffer.
    pub fn new(definition: Arc<StructureDefinition>, data: &'a [u8]) -> Result<Self, AccessError> {
        Ok(Self {
            definition,
            data,
            offset_cache: RefCell::new(HashMap::new()),
            evaluator: ExpressionEvaluator::new(),
            parent: None,
            base_offset: 0,
        })
    }

    /// Create a nested accessor for a sub-structure.
    pub fn new_nested(
        definition: Arc<StructureDefinition>,
        data: &'a [u8],
        parent: &'a StructureAccessor<'a>,
        base_offset: usize,
    ) -> Result<Self, AccessError> {
        Ok(Self {
            definition,
            data,
            offset_cache: RefCell::new(HashMap::new()),
            evaluator: ExpressionEvaluator::new(),
            parent: Some(parent),
            base_offset,
        })
    }

    /// Get the structure definition.
    pub fn definition(&self) -> &StructureDefinition {
        &self.definition
    }

    /// Get the underlying data buffer.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }


    /// Access a field by dot-notation path.
    ///
    /// # Arguments
    /// * `path` - Field path using dot notation (e.g., "parent.child" or "items_0.value")
    ///
    /// # Returns
    /// The parsed value or an error if the field doesn't exist or can't be parsed.
    pub fn get(&self, path: &str) -> Result<Value<'a>, AccessError> {
        // Check if this is a repeated field access (e.g., "items_0")
        let (field_name, index, rest) = self.parse_path(path);

        // Find the field definition
        let field = self.find_field(&field_name)?;

        // Check condition if present
        if let Some(ref condition) = field.condition {
            let ctx = self.build_eval_context()?;
            let result = self.evaluator.evaluate(condition, &ctx).map_err(|e| {
                AccessError::ExpressionError {
                    path: path.to_string(),
                    message: e.to_string(),
                }
            })?;

            match result {
                EvalResult::Boolean(false) => {
                    return Err(AccessError::ConditionalNotPresent {
                        path: path.to_string(),
                        condition: format!("{:?}", condition),
                    });
                }
                EvalResult::Boolean(true) => {}
                _ => {
                    return Err(AccessError::ExpressionError {
                        path: path.to_string(),
                        message: "Condition did not evaluate to boolean".to_string(),
                    });
                }
            }
        }

        // Calculate offset and size
        let (offset, size) = self.calculate_field_offset(&field_name, index)?;

        // Handle repeated fields
        if let Some(ref repeat) = field.repeat {
            if let Some(idx) = index {
                // Accessing a specific element - need to verify it exists
                let count = self.get_actual_repeat_count(repeat, &field_name, offset, field)?;
                if idx >= count {
                    return Err(AccessError::UnknownField {
                        path: path.to_string(),
                    });
                }

                // Accessing a specific element
                let element_value = self.read_field_value(field, offset, size)?;

                // If there's more path to traverse, handle nested access
                if let Some(remaining) = rest {
                    return self.access_nested(&element_value, &remaining);
                }
                return Ok(element_value);
            } else {
                // Accessing the entire array
                let count = self.get_actual_repeat_count(repeat, &field_name, offset, field)?;
                let mut values = Vec::with_capacity(count.min(1000)); // Cap initial capacity
                let mut current_offset = offset;

                for i in 0..count {
                    // Check bounds
                    let elem_size = self.get_element_size(field, current_offset)?;
                    if current_offset + elem_size > self.data.len() {
                        break;
                    }

                    let value = self.read_field_value(field, current_offset, elem_size)?;
                    values.push(value);
                    current_offset += elem_size;

                    // Check until condition
                    if let RepeatSpec::Until(ref until_expr) = repeat {
                        let mut ctx = self.build_eval_context()?;
                        ctx.index = Some(i);
                        // Add current element value to context for _ reference
                        self.add_value_to_context(&mut ctx, "_", &values[i])?;

                        let result =
                            self.evaluator
                                .evaluate(until_expr, &ctx)
                                .map_err(|e| AccessError::ExpressionError {
                                    path: field_name.clone(),
                                    message: e.to_string(),
                                })?;

                        if let EvalResult::Boolean(true) = result {
                            break;
                        }
                    }
                }

                return Ok(Value::Array(values));
            }
        }

        // Read the field value
        let value = self.read_field_value(field, offset, size)?;

        // If there's more path to traverse, handle nested access
        if let Some(remaining) = rest {
            return self.access_nested(&value, &remaining);
        }

        Ok(value)
    }


    /// Check if a field exists and is accessible.
    ///
    /// Returns true if the field exists in the definition and any conditions are met.
    pub fn has(&self, path: &str) -> bool {
        self.get(path).is_ok()
    }

    /// Get field metadata (type, size, offset).
    pub fn field_info(&self, path: &str) -> Option<FieldInfo> {
        let (field_name, index, _) = self.parse_path(path);
        let field = self.find_field(&field_name).ok()?;
        let (offset, size) = self.calculate_field_offset(&field_name, index).ok()?;

        Some(FieldInfo {
            field_type: field.field_type.clone(),
            size,
            offset,
            encoding: field.encoding,
            doc: field.doc.clone(),
        })
    }

    /// Iterate over all accessible field paths.
    ///
    /// This includes expanded paths for repeated fields (e.g., "items_0", "items_1").
    pub fn fields(&self) -> impl Iterator<Item = String> + '_ {
        FieldIterator::new(self)
    }

    /// Get raw byte slice for a field (zero-copy).
    ///
    /// Returns a slice directly into the source buffer without copying.
    /// Returns NonContiguous error for array fields accessed without an index,
    /// as the entire array may not be contiguous in memory.
    pub fn raw_slice(&self, path: &str) -> Result<&'a [u8], AccessError> {
        let (field_name, index, rest) = self.parse_path(path);

        // Check if this is a repeated field accessed without an index
        if let Ok(field) = self.find_field(&field_name) {
            if field.repeat.is_some() && index.is_none() {
                // Accessing entire array - may not be contiguous
                return Err(AccessError::NonContiguous {
                    path: path.to_string(),
                });
            }
        }

        // Check for nested path access - nested structures may not be contiguous
        if rest.is_some() {
            return Err(AccessError::NonContiguous {
                path: path.to_string(),
            });
        }

        let (offset, size) = self.field_byte_range(path)?;
        if offset + size > self.data.len() {
            return Err(AccessError::UnexpectedEof {
                path: path.to_string(),
                expected: size,
                available: self.data.len().saturating_sub(offset),
            });
        }
        Ok(&self.data[offset..offset + size])
    }

    /// Get byte offset and length for a field.
    ///
    /// Returns NonContiguous error for array fields accessed without an index,
    /// as the entire array may not be contiguous in memory.
    pub fn field_byte_range(&self, path: &str) -> Result<(usize, usize), AccessError> {
        let (field_name, index, rest) = self.parse_path(path);

        // Check if this is a repeated field accessed without an index
        if let Ok(field) = self.find_field(&field_name) {
            if field.repeat.is_some() && index.is_none() {
                // Accessing entire array - may not be contiguous
                return Err(AccessError::NonContiguous {
                    path: path.to_string(),
                });
            }
        }

        // Check for nested path access - nested structures may not be contiguous
        if rest.is_some() {
            return Err(AccessError::NonContiguous {
                path: path.to_string(),
            });
        }

        self.calculate_field_offset(&field_name, index)
    }


    // ==================== Private Helper Methods ====================

    /// Parse a field path into components.
    /// Returns (field_name, optional_index, optional_remaining_path)
    fn parse_path(&self, path: &str) -> (String, Option<usize>, Option<String>) {
        // Split on first dot to get the first component
        let (first, rest) = match path.find('.') {
            Some(pos) => (&path[..pos], Some(path[pos + 1..].to_string())),
            None => (path, None),
        };

        // Check if the first component has an underscore index (e.g., "items_0")
        if let Some(underscore_pos) = first.rfind('_') {
            let potential_index = &first[underscore_pos + 1..];
            if let Ok(index) = potential_index.parse::<usize>() {
                let field_name = first[..underscore_pos].to_string();
                return (field_name, Some(index), rest);
            }
        }

        (first.to_string(), None, rest)
    }

    /// Find a field definition by name.
    fn find_field(&self, name: &str) -> Result<&FieldDefinition, AccessError> {
        self.definition
            .fields
            .iter()
            .find(|f| f.id == name)
            .ok_or_else(|| AccessError::UnknownField {
                path: name.to_string(),
            })
    }

    /// Calculate the offset and size for a field.
    pub(crate) fn calculate_field_offset(
        &self,
        field_name: &str,
        index: Option<usize>,
    ) -> Result<(usize, usize), AccessError> {
        // Check cache first
        let cache_key = match index {
            Some(i) => format!("{}_{}", field_name, i),
            None => field_name.to_string(),
        };

        if let Some(&(offset, size)) = self.offset_cache.borrow().get(&cache_key) {
            return Ok((offset, size));
        }

        // Calculate offset by iterating through fields
        let mut current_offset = 0;

        for field in &self.definition.fields {
            // Check if this field is conditional and not present
            if let Some(ref condition) = field.condition {
                let ctx = self.build_eval_context_up_to(&field.id)?;
                let result =
                    self.evaluator
                        .evaluate(condition, &ctx)
                        .map_err(|e| AccessError::ExpressionError {
                            path: field.id.clone(),
                            message: e.to_string(),
                        })?;

                if let EvalResult::Boolean(false) = result {
                    // Skip this field
                    if field.id == field_name {
                        return Err(AccessError::ConditionalNotPresent {
                            path: field_name.to_string(),
                            condition: format!("{:?}", condition),
                        });
                    }
                    continue;
                }
            }

            if field.id == field_name {
                // Found the field
                let size = self.get_field_size(field, current_offset)?;

                // Handle repeated fields
                if let Some(ref repeat) = field.repeat {
                    if let Some(idx) = index {
                        // Calculate offset to specific element
                        let mut elem_offset = current_offset;
                        for i in 0..idx {
                            let elem_size = self.get_element_size(field, elem_offset)?;
                            elem_offset += elem_size;

                            // Check until condition
                            if let RepeatSpec::Until(ref until_expr) = repeat {
                                let value =
                                    self.read_field_value(field, elem_offset - elem_size, elem_size)?;
                                let mut ctx = self.build_eval_context_up_to(&field.id)?;
                                ctx.index = Some(i);
                                self.add_value_to_context(&mut ctx, "_", &value)?;

                                let result =
                                    self.evaluator.evaluate(until_expr, &ctx).map_err(|e| {
                                        AccessError::ExpressionError {
                                            path: field_name.to_string(),
                                            message: e.to_string(),
                                        }
                                    })?;

                                if let EvalResult::Boolean(true) = result {
                                    return Err(AccessError::UnknownField {
                                        path: format!("{}_{}", field_name, idx),
                                    });
                                }
                            }
                        }
                        let elem_size = self.get_element_size(field, elem_offset)?;

                        // Cache the result
                        self.offset_cache
                            .borrow_mut()
                            .insert(cache_key, (elem_offset, elem_size));

                        return Ok((elem_offset, elem_size));
                    }
                }

                // Cache the result
                self.offset_cache
                    .borrow_mut()
                    .insert(cache_key, (current_offset, size));

                return Ok((current_offset, size));
            }

            // Move past this field
            let field_size = self.get_total_field_size(field, current_offset)?;
            current_offset += field_size;
        }

        Err(AccessError::UnknownField {
            path: field_name.to_string(),
        })
    }


    /// Get the size of a single field (not including repetitions).
    fn get_field_size(&self, field: &FieldDefinition, offset: usize) -> Result<usize, AccessError> {
        match &field.size {
            SizeSpec::Fixed(size) => {
                if *size == 0 {
                    // Size comes from type
                    match &field.field_type {
                        FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                            Ok(*bytes as usize)
                        }
                        FieldType::TypeRef(type_name) => {
                            // Get size from nested type
                            self.get_type_size(type_name, offset)
                        }
                        _ => Ok(0),
                    }
                } else {
                    Ok(*size)
                }
            }
            SizeSpec::Expression(expr) => {
                let ctx = self.build_eval_context()?;
                let result =
                    self.evaluator
                        .evaluate(expr, &ctx)
                        .map_err(|e| AccessError::ExpressionError {
                            path: field.id.clone(),
                            message: e.to_string(),
                        })?;

                match result {
                    EvalResult::Integer(n) if n >= 0 => Ok(n as usize),
                    _ => Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: "Size expression did not evaluate to positive integer".to_string(),
                    }),
                }
            }
        }
    }

    /// Get the size of a single element in a repeated field.
    fn get_element_size(
        &self,
        field: &FieldDefinition,
        offset: usize,
    ) -> Result<usize, AccessError> {
        self.get_field_size(field, offset)
    }

    /// Get the total size of a field including all repetitions.
    fn get_total_field_size(
        &self,
        field: &FieldDefinition,
        offset: usize,
    ) -> Result<usize, AccessError> {
        let element_size = self.get_field_size(field, offset)?;

        match &field.repeat {
            None => Ok(element_size),
            Some(RepeatSpec::Count(n)) => Ok(element_size * n),
            Some(RepeatSpec::Expression(expr)) => {
                let ctx = self.build_eval_context()?;
                let result =
                    self.evaluator
                        .evaluate(expr, &ctx)
                        .map_err(|e| AccessError::ExpressionError {
                            path: field.id.clone(),
                            message: e.to_string(),
                        })?;

                match result {
                    EvalResult::Integer(n) if n >= 0 => Ok(element_size * n as usize),
                    _ => Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: "Repeat expression did not evaluate to positive integer"
                            .to_string(),
                    }),
                }
            }
            Some(RepeatSpec::Until(_)) | Some(RepeatSpec::Eos) => {
                // For until/eos, we need to actually parse to determine size
                // This is expensive but necessary
                let count = self.get_repeat_count(field.repeat.as_ref().unwrap(), &field.id)?;
                let mut total_size = 0;
                let mut current_offset = offset;

                for _ in 0..count {
                    let elem_size = self.get_element_size(field, current_offset)?;
                    total_size += elem_size;
                    current_offset += elem_size;
                }

                Ok(total_size)
            }
        }
    }

    /// Get the size of a nested type.
    fn get_type_size(&self, type_name: &str, offset: usize) -> Result<usize, AccessError> {
        let nested_def = self.definition.types.get(type_name).ok_or_else(|| {
            AccessError::UnknownField {
                path: type_name.to_string(),
            }
        })?;

        // Calculate total size of nested type
        let mut total_size = 0;
        for field in &nested_def.fields {
            total_size += self.get_field_size(field, offset + total_size)?;
        }

        Ok(total_size)
    }


    /// Get the repeat count for a repeated field.
    fn get_repeat_count(&self, repeat: &RepeatSpec, field_name: &str) -> Result<usize, AccessError> {
        match repeat {
            RepeatSpec::Count(n) => Ok(*n),
            RepeatSpec::Expression(expr) => {
                let ctx = self.build_eval_context()?;
                let result =
                    self.evaluator
                        .evaluate(expr, &ctx)
                        .map_err(|e| AccessError::ExpressionError {
                            path: field_name.to_string(),
                            message: e.to_string(),
                        })?;

                match result {
                    EvalResult::Integer(n) if n >= 0 => Ok(n as usize),
                    _ => Err(AccessError::ExpressionError {
                        path: field_name.to_string(),
                        message: "Repeat expression did not evaluate to positive integer"
                            .to_string(),
                    }),
                }
            }
            RepeatSpec::Until(_) => {
                // For until, we need to count by parsing
                // This is handled in the get method
                Ok(usize::MAX) // Placeholder - actual count determined during parsing
            }
            RepeatSpec::Eos => {
                // For eos, count is determined by remaining data
                Ok(usize::MAX) // Placeholder - actual count determined during parsing
            }
        }
    }

    /// Get the actual repeat count by parsing the data.
    pub(crate) fn get_actual_repeat_count(
        &self,
        repeat: &RepeatSpec,
        field_name: &str,
        start_offset: usize,
        field: &FieldDefinition,
    ) -> Result<usize, AccessError> {
        match repeat {
            RepeatSpec::Count(n) => Ok(*n),
            RepeatSpec::Expression(expr) => {
                let ctx = self.build_eval_context()?;
                let result =
                    self.evaluator
                        .evaluate(expr, &ctx)
                        .map_err(|e| AccessError::ExpressionError {
                            path: field_name.to_string(),
                            message: e.to_string(),
                        })?;

                match result {
                    EvalResult::Integer(n) if n >= 0 => Ok(n as usize),
                    _ => Err(AccessError::ExpressionError {
                        path: field_name.to_string(),
                        message: "Repeat expression did not evaluate to positive integer"
                            .to_string(),
                    }),
                }
            }
            RepeatSpec::Until(until_expr) => {
                // Parse elements until condition is true
                let mut count = 0;
                let mut current_offset = start_offset;

                loop {
                    let elem_size = self.get_element_size(field, current_offset)?;
                    if current_offset + elem_size > self.data.len() {
                        break;
                    }

                    let value = self.read_field_value(field, current_offset, elem_size)?;
                    count += 1;

                    // Check until condition
                    let mut ctx = self.build_eval_context()?;
                    ctx.index = Some(count - 1);
                    self.add_value_to_context(&mut ctx, "_", &value)?;

                    let result = self.evaluator.evaluate(until_expr, &ctx).map_err(|e| {
                        AccessError::ExpressionError {
                            path: field_name.to_string(),
                            message: e.to_string(),
                        }
                    })?;

                    if let EvalResult::Boolean(true) = result {
                        break;
                    }

                    current_offset += elem_size;
                }

                Ok(count)
            }
            RepeatSpec::Eos => {
                // Count elements until end of data
                let elem_size = self.get_element_size(field, start_offset)?;
                if elem_size == 0 {
                    return Ok(0);
                }

                let remaining = self.data.len().saturating_sub(start_offset);
                Ok(remaining / elem_size)
            }
        }
    }


    /// Read a field value from the data buffer.
    fn read_field_value(
        &self,
        field: &FieldDefinition,
        offset: usize,
        size: usize,
    ) -> Result<Value<'a>, AccessError> {
        // Check bounds
        if offset + size > self.data.len() {
            return Err(AccessError::UnexpectedEof {
                path: field.id.clone(),
                expected: size,
                available: self.data.len().saturating_sub(offset),
            });
        }

        let bytes = &self.data[offset..offset + size];
        read_field_value_from_bytes(field, bytes, self.definition.endian)
    }

    /// Build evaluation context with all parsed fields.
    pub(crate) fn build_eval_context(&self) -> Result<EvalContext, AccessError> {
        self.build_eval_context_up_to("")
    }

    /// Build evaluation context with fields up to (but not including) the specified field.
    pub(crate) fn build_eval_context_up_to(&self, stop_at: &str) -> Result<EvalContext, AccessError> {
        build_context_from_definition(
            &self.definition,
            self.data,
            &self.evaluator,
            stop_at,
            |field, offset, size| self.read_field_value(field, offset, size),
        )
    }

    /// Add a value to the evaluation context.
    pub(crate) fn add_value_to_context(
        &self,
        ctx: &mut EvalContext,
        name: &str,
        value: &Value<'a>,
    ) -> Result<(), AccessError> {
        add_value_to_context_impl(ctx, name, value)
    }


    /// Access a nested field within a value.
    fn access_nested(&self, value: &Value<'a>, path: &str) -> Result<Value<'a>, AccessError> {
        match value {
            Value::Struct(struct_val) => {
                // Get the nested type definition
                let nested_def = self
                    .definition
                    .types
                    .get(&struct_val.type_name)
                    .ok_or_else(|| AccessError::UnknownField {
                        path: struct_val.type_name.clone(),
                    })?;

                // Parse the nested path and calculate offset manually
                let (field_name, _index, rest) = self.parse_path(path);

                // Find the field in the nested definition
                let field = nested_def
                    .fields
                    .iter()
                    .find(|f| f.id == field_name)
                    .ok_or_else(|| AccessError::UnknownField {
                        path: field_name.clone(),
                    })?;

                // Calculate offset within nested structure
                let mut offset = 0;
                for f in &nested_def.fields {
                    if f.id == field_name {
                        break;
                    }
                    // Get field size
                    let size = match &f.size {
                        SizeSpec::Fixed(s) => *s,
                        _ => match &f.field_type {
                            FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                                *bytes as usize
                            }
                            _ => 0,
                        },
                    };
                    offset += size;
                }

                // Get field size
                let size = match &field.size {
                    SizeSpec::Fixed(s) => *s,
                    _ => match &field.field_type {
                        FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                            *bytes as usize
                        }
                        _ => 0,
                    },
                };

                // Check bounds
                if offset + size > struct_val.data.len() {
                    return Err(AccessError::UnexpectedEof {
                        path: path.to_string(),
                        expected: size,
                        available: struct_val.data.len().saturating_sub(offset),
                    });
                }

                let bytes = &struct_val.data[offset..offset + size];

                // Read the value
                let value = match &field.field_type {
                    FieldType::String => {
                        let s =
                            std::str::from_utf8(bytes).map_err(|e| AccessError::EncodingError {
                                path: field.id.clone(),
                                encoding: "UTF-8".to_string(),
                                message: e.to_string(),
                            })?;
                        Value::String(Cow::Borrowed(s))
                    }
                    FieldType::Bytes => Value::Bytes(bytes),
                    FieldType::UnsignedInt(byte_size) => {
                        let val = read_unsigned(bytes, *byte_size, nested_def.endian)?;
                        Value::Unsigned(val)
                    }
                    FieldType::SignedInt(byte_size) => {
                        let val = read_signed(bytes, *byte_size, nested_def.endian)?;
                        Value::Unsigned(val as u64)
                    }
                    FieldType::TypeRef(type_name) => Value::from_struct(bytes, type_name.clone()),
                };

                // If there's more path to traverse, recurse
                if let Some(remaining) = rest {
                    return self.access_nested(&value, &remaining);
                }

                Ok(value)
            }
            Value::Array(arr) => {
                // Parse index from path
                let (field_name, arr_index, rest) = self.parse_path(path);
                if let Some(idx) = arr_index {
                    if idx < arr.len() {
                        if let Some(remaining) = rest {
                            return self.access_nested(&arr[idx], &remaining);
                        }
                        return Ok(arr[idx].clone());
                    }
                }
                // Try to access by name if it's an array element
                if let Ok(idx) = field_name.parse::<usize>() {
                    if idx < arr.len() {
                        return Ok(arr[idx].clone());
                    }
                }
                Err(AccessError::UnknownField {
                    path: path.to_string(),
                })
            }
            _ => Err(AccessError::UnknownField {
                path: path.to_string(),
            }),
        }
    }
}


// Implement Index trait for bracket notation access
impl<'a> std::ops::Index<&str> for StructureAccessor<'a> {
    type Output = Value<'a>;

    fn index(&self, path: &str) -> &Self::Output {
        // Note: This is a limitation - we can't return a reference to a temporary
        // In practice, use get() for proper error handling
        panic!(
            "Use get() method instead of index operator for proper error handling. Path: {}",
            path
        );
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
