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
/// Fields are parsed on first access via a single O(n) pass through the
/// structure definition. This pass populates both the offset cache and
/// evaluation context, making all subsequent field accesses O(1) lookups.
pub struct StructureAccessor<'a> {
    /// The structure definition
    pub(crate) definition: Arc<StructureDefinition>,
    /// Source data buffer
    data: &'a [u8],
    /// Cached field offsets: path -> (offset, size)
    offset_cache: RefCell<HashMap<String, (usize, usize)>>,
    /// Cached evaluation context from single-pass parse (None = not yet parsed)
    parsed_context: RefCell<Option<EvalContext>>,
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
            parsed_context: RefCell::new(None),
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
            parsed_context: RefCell::new(None),
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

    /// Perform a single O(n) pass through all fields, populating the offset cache
    /// and evaluation context. After this call, all `get()` and `build_eval_context()`
    /// calls become O(1) lookups.
    ///
    /// This is called automatically on first field access. It eliminates the O(n²)
    /// behavior that occurred when each `get()` call walked from the start of the
    /// definition to compute offsets.
    fn ensure_parsed(&self) -> Result<(), AccessError> {
        // Already parsed — nothing to do
        if self.parsed_context.borrow().is_some() {
            return Ok(());
        }

        let mut ctx = EvalContext::new();
        let mut current_offset = 0;

        for field in &self.definition.fields {
            // Check if this field is conditional and not present
            if let Some(ref condition) = field.condition {
                let result = self.evaluator.evaluate(condition, &ctx);
                if let Ok(EvalResult::Boolean(false)) = result {
                    continue;
                }
            }

            // Get single-element size
            let size = match context::get_simple_field_size(
                field,
                &ctx,
                &self.evaluator,
                &self.definition,
                self.data,
                current_offset,
            ) {
                Ok(s) => s,
                Err(_) => {
                    // Can't determine this field's size — we can't compute correct
                    // offsets for any subsequent fields, so stop parsing here.
                    break;
                }
            };

            // Handle repeated fields — cache every element's offset
            if let Some(ref repeat) = field.repeat {
                let count = match repeat {
                    RepeatSpec::Count(n) => *n,
                    RepeatSpec::Expression(expr) => {
                        match self.evaluator.evaluate(expr, &ctx) {
                            Ok(EvalResult::Integer(n)) if n >= 0 => n as usize,
                            _ => 0,
                        }
                    }
                    RepeatSpec::Until(_) | RepeatSpec::Eos => {
                        // For until/eos we need to parse element-by-element below
                        0
                    }
                };

                match repeat {
                    RepeatSpec::Count(_) | RepeatSpec::Expression(_) => {
                        let mut elem_offset = current_offset;
                        for i in 0..count {
                            let elem_size = match &field.field_type {
                                FieldType::TypeRef(type_name) => {
                                    self.get_type_size(type_name, elem_offset).unwrap_or(size)
                                }
                                _ => size,
                            };
                            let cache_key = format!("{}_{}", field.id, i);
                            self.offset_cache
                                .borrow_mut()
                                .insert(cache_key, (elem_offset, elem_size));

                            // Read element value and add to context (for the field name,
                            // only the last element matters for expression evaluation,
                            // but we need to read each to advance the offset)
                            if elem_offset + elem_size <= self.data.len() {
                                if let Ok(value) =
                                    self.read_field_value(field, elem_offset, elem_size)
                                {
                                    let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);
                                }
                            }
                            elem_offset += elem_size;
                        }
                        // Cache the base field offset (no index) pointing to first element
                        self.offset_cache
                            .borrow_mut()
                            .insert(field.id.clone(), (current_offset, size));
                        current_offset = elem_offset;
                    }
                    RepeatSpec::Until(until_expr) => {
                        // Parse element-by-element until condition is true
                        let mut elem_offset = current_offset;
                        let mut i = 0;
                        loop {
                            let elem_size = match &field.field_type {
                                FieldType::TypeRef(type_name) => {
                                    self.get_type_size(type_name, elem_offset).unwrap_or(size)
                                }
                                _ => size,
                            };
                            if elem_offset + elem_size > self.data.len() {
                                break;
                            }
                            let cache_key = format!("{}_{}", field.id, i);
                            self.offset_cache
                                .borrow_mut()
                                .insert(cache_key, (elem_offset, elem_size));

                            let value = self.read_field_value(field, elem_offset, elem_size)?;
                            let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);

                            // Check until condition
                            let mut until_ctx = ctx.clone();
                            until_ctx.index = Some(i);
                            let _ = add_value_to_context_impl(&mut until_ctx, "_", &value);
                            if let Ok(EvalResult::Boolean(true)) =
                                self.evaluator.evaluate(until_expr, &until_ctx)
                            {
                                elem_offset += elem_size;
                                break;
                            }

                            i += 1;
                            elem_offset += elem_size;
                        }
                        self.offset_cache
                            .borrow_mut()
                            .insert(field.id.clone(), (current_offset, size));
                        current_offset = elem_offset;
                    }
                    RepeatSpec::Eos => {
                        // Parse until end of data
                        let mut elem_offset = current_offset;
                        let mut i = 0;
                        loop {
                            let elem_size = match &field.field_type {
                                FieldType::TypeRef(type_name) => {
                                    self.get_type_size(type_name, elem_offset).unwrap_or(size)
                                }
                                _ => size,
                            };
                            if elem_size == 0 || elem_offset + elem_size > self.data.len() {
                                break;
                            }
                            let cache_key = format!("{}_{}", field.id, i);
                            self.offset_cache
                                .borrow_mut()
                                .insert(cache_key, (elem_offset, elem_size));

                            if let Ok(value) =
                                self.read_field_value(field, elem_offset, elem_size)
                            {
                                let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);
                            }
                            i += 1;
                            elem_offset += elem_size;
                        }
                        self.offset_cache
                            .borrow_mut()
                            .insert(field.id.clone(), (current_offset, size));
                        current_offset = elem_offset;
                    }
                }
            } else {
                // Non-repeated field — cache offset and read value into context
                self.offset_cache
                    .borrow_mut()
                    .insert(field.id.clone(), (current_offset, size));

                if current_offset + size <= self.data.len() {
                    if let Ok(value) = self.read_field_value(field, current_offset, size) {
                        let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);
                    }
                }
                current_offset += size;
            }
        }

        *self.parsed_context.borrow_mut() = Some(ctx);
        Ok(())
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
    /// Yields each field's base name once. For repeated fields, the base name
    /// is yielded (e.g., `"items"`) rather than expanded indexed names.
    /// Use `get(field_id)` to obtain a `Value::Array` for repeated fields.
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
    ///
    /// Field names are taken as-is — no `_N` suffix parsing is performed.
    /// Repeated fields are accessed via `get("field")` which returns `Value::Array`.
    fn parse_path(&self, path: &str) -> (String, Option<usize>, Option<String>) {
        // Split on first dot to get the first component
        let (first, rest) = match path.find('.') {
            Some(pos) => (&path[..pos], Some(path[pos + 1..].to_string())),
            None => (path, None),
        };

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
    ///
    /// On first call, triggers `ensure_parsed()` which does a single O(n) pass
    /// through all fields, populating the offset cache. Subsequent calls are O(1).
    pub(crate) fn calculate_field_offset(
        &self,
        field_name: &str,
        index: Option<usize>,
    ) -> Result<(usize, usize), AccessError> {
        // Build cache key
        let cache_key = match index {
            Some(i) => format!("{}_{}", field_name, i),
            None => field_name.to_string(),
        };

        // Fast path: already cached
        if let Some(&(offset, size)) = self.offset_cache.borrow().get(&cache_key) {
            return Ok((offset, size));
        }

        // Trigger single-pass parse to populate all caches
        self.ensure_parsed()?;

        // Should be cached now
        if let Some(&(offset, size)) = self.offset_cache.borrow().get(&cache_key) {
            return Ok((offset, size));
        }

        // Field wasn't found during parsing — check if it's a conditional field
        // that was skipped
        if let Ok(field) = self.find_field(field_name) {
            if let Some(ref condition) = field.condition {
                return Err(AccessError::ConditionalNotPresent {
                    path: field_name.to_string(),
                    condition: format!("{:?}", condition),
                });
            }
        }

        Err(AccessError::UnknownField {
            path: cache_key,
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

    /// Get the size of a nested type.
    pub(crate) fn get_type_size(&self, type_name: &str, offset: usize) -> Result<usize, AccessError> {
        let nested_def = self.definition.types.get(type_name).ok_or_else(|| {
            AccessError::UnknownField {
                path: type_name.to_string(),
            }
        })?;

        // Build context for evaluating conditions within the nested type
        let mut nested_ctx = self.build_eval_context()?;
        let mut total_size = 0;
        
        for field in &nested_def.fields {
            // Check if this field is conditional
            if let Some(ref condition) = field.condition {
                let result = self.evaluator.evaluate(condition, &nested_ctx);
                match result {
                    Ok(EvalResult::Boolean(false)) => {
                        // Skip this conditional field
                        continue;
                    }
                    Ok(EvalResult::Boolean(true)) => {
                        // Condition is true, continue to process field
                    }
                    Err(_) => {
                        // Condition evaluation failed - skip this field
                        // This can happen when referenced fields don't exist yet
                        continue;
                    }
                    _ => {
                        // Condition didn't evaluate to boolean - skip field
                        continue;
                    }
                }
            }
            
            // Get field size (single element)
            let field_size = self.get_nested_field_size(field, &nested_ctx, offset + total_size)?;
            
            // Read field value and add to context for subsequent fields
            if offset + total_size + field_size <= self.data.len() {
                let field_data = &self.data[offset + total_size..offset + total_size + field_size];
                if let Ok(value) = self.read_simple_nested_value(field, field_data) {
                    let _ = self.add_value_to_context(&mut nested_ctx, &field.id, &value);
                }
            }
            
            // Calculate total field size including repetitions
            let total_field_size = self.get_nested_total_field_size(field, &nested_ctx, field_size, offset + total_size)?;
            
            total_size += total_field_size;
        }

        Ok(total_size)
    }
    
    /// Get the total size of a field within a nested type, including repetitions.
    fn get_nested_total_field_size(
        &self,
        field: &FieldDefinition,
        ctx: &EvalContext,
        element_size: usize,
        base_offset: usize,
    ) -> Result<usize, AccessError> {
        match &field.repeat {
            None => Ok(element_size),
            Some(RepeatSpec::Count(n)) => Ok(element_size * n),
            Some(RepeatSpec::Expression(expr)) => {
                let result = self.evaluator.evaluate(expr, ctx).map_err(|e| {
                    AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: e.to_string(),
                    }
                })?;

                match result {
                    EvalResult::Integer(n) if n >= 0 => {
                        // For TypeRef fields with variable-length elements, calculate each element's size
                        if let FieldType::TypeRef(type_name) = &field.field_type {
                            let mut total = 0;
                            let mut current_offset = base_offset;
                            for _ in 0..(n as usize) {
                                let elem_size = self.get_type_size(type_name, current_offset)?;
                                total += elem_size;
                                current_offset += elem_size;
                            }
                            Ok(total)
                        } else {
                            Ok(element_size * n as usize)
                        }
                    }
                    _ => Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: "Repeat expression did not evaluate to positive integer".to_string(),
                    }),
                }
            }
            Some(RepeatSpec::Until(_)) | Some(RepeatSpec::Eos) => {
                // For until/eos, return element size as approximation
                Ok(element_size)
            }
        }
    }
    
    /// Get the size of a field within a nested type, using the nested context.
    fn get_nested_field_size(
        &self,
        field: &FieldDefinition,
        ctx: &EvalContext,
        offset: usize,
    ) -> Result<usize, AccessError> {
        match &field.size {
            SizeSpec::Fixed(size) => {
                if *size == 0 {
                    match &field.field_type {
                        FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                            Ok(*bytes as usize)
                        }
                        FieldType::TypeRef(type_name) => {
                            self.get_type_size(type_name, offset)
                        }
                        _ => Ok(0),
                    }
                } else {
                    Ok(*size)
                }
            }
            SizeSpec::Expression(expr) => {
                let result = self.evaluator.evaluate(expr, ctx).map_err(|e| {
                    AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: e.to_string(),
                    }
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
    
    /// Read a simple value from field data for nested type context building.
    fn read_simple_nested_value<'b>(&self, field: &FieldDefinition, data: &'b [u8]) -> Result<Value<'b>, AccessError> {
        use std::borrow::Cow;
        
        match &field.field_type {
            FieldType::String => {
                let s = std::str::from_utf8(data).unwrap_or("");
                Ok(Value::String(Cow::Borrowed(s)))
            }
            FieldType::Bytes => Ok(Value::Bytes(data)),
            FieldType::UnsignedInt(bytes) => {
                let n = match bytes {
                    1 => data.first().map(|&b| b as u64).unwrap_or(0),
                    2 if data.len() >= 2 => u16::from_be_bytes([data[0], data[1]]) as u64,
                    4 if data.len() >= 4 => {
                        u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64
                    }
                    _ => 0,
                };
                Ok(Value::Unsigned(n))
            }
            FieldType::SignedInt(bytes) => {
                let n = match bytes {
                    1 => data.first().map(|&b| b as i8 as i64 as u64).unwrap_or(0),
                    2 if data.len() >= 2 => {
                        i16::from_be_bytes([data[0], data[1]]) as i64 as u64
                    }
                    4 if data.len() >= 4 => {
                        i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64 as u64
                    }
                    _ => 0,
                };
                Ok(Value::Unsigned(n))
            }
            FieldType::TypeRef(_) => {
                Err(AccessError::UnknownField {
                    path: field.id.clone(),
                })
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
    ///
    /// Returns the cached context if available (populated by `ensure_parsed`),
    /// otherwise falls back to building from scratch.
    pub(crate) fn build_eval_context(&self) -> Result<EvalContext, AccessError> {
        // Return cached context if we've already done the full parse
        if let Some(ref ctx) = *self.parsed_context.borrow() {
            return Ok(ctx.clone());
        }
        self.build_eval_context_up_to("")
    }

    /// Build evaluation context with fields up to (but not including) the specified field.
    ///
    /// If the full context has been cached by `ensure_parsed`, returns a clone of it
    /// (which is a superset of any partial context). For the "up to" case during
    /// `ensure_parsed` itself, falls back to the uncached path.
    pub(crate) fn build_eval_context_up_to(&self, stop_at: &str) -> Result<EvalContext, AccessError> {
        // If we have a full cached context, it's a superset of any partial context
        if let Some(ref ctx) = *self.parsed_context.borrow() {
            return Ok(ctx.clone());
        }
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
