//! Accessor for reading structure fields from binary data.
//!
//! The [`StructureAccessor`] provides a map-like interface for reading parsed values,
//! parsing all fields in a single upfront pass into a value dictionary.
//!
//! # Submodules
//!
//! - [`context`] - Evaluation context building
//! - [`iterator`] - Field iteration support
//! - [`read`] - Value reading from binary data

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
    Encoding, Endian, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
};
use crate::parser::value::Value;

pub use iterator::FieldIterator;

use context::{add_value_to_context_impl, build_context_from_definition, get_simple_field_size};
use read::{read_field_value_from_bytes, read_float, read_signed, read_unsigned};

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

/// Accessor for reading structure fields from binary data.
///
/// On first access, a single O(n) pass parses all fields into a value
/// dictionary. Subsequent `get()` calls are O(1) lookups. Repeated fields
/// are stored as `Value::Array` under the base field name.
pub struct StructureAccessor<'a> {
    /// The structure definition
    pub(crate) definition: Arc<StructureDefinition>,
    /// Source data buffer
    data: &'a [u8],
    /// Cached offset map for raw_slice: field_id -> (offset, size)
    /// For repeated fields, stores (base_offset, element_size) under the base name,
    /// and total_size under "{field_id}__total" for the full span.
    offset_cache: RefCell<HashMap<String, (usize, usize)>>,
    /// Repeat element offsets: field_id -> Vec<(offset, size)> for each element
    repeat_offsets: RefCell<HashMap<String, Vec<(usize, usize)>>>,
    /// Cached evaluation context from single-pass parse (None = not yet parsed)
    parsed_context: RefCell<Option<EvalContext>>,
    /// Whether ensure_parsed has been called
    parsed: RefCell<bool>,
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
            repeat_offsets: RefCell::new(HashMap::new()),
            parsed_context: RefCell::new(None),
            parsed: RefCell::new(false),
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
            repeat_offsets: RefCell::new(HashMap::new()),
            parsed_context: RefCell::new(None),
            parsed: RefCell::new(false),
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

    /// Perform a single O(n) pass through all fields, populating the
    /// offset cache, repeat offsets, and evaluation context.
    fn ensure_parsed(&self) -> Result<(), AccessError> {
        if *self.parsed.borrow() {
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
            let size = match get_simple_field_size(
                field,
                &ctx,
                &self.evaluator,
                &self.definition,
                self.data,
                current_offset,
            ) {
                Ok(s) => s,
                Err(_) => break,
            };

            // Handle repeated fields — store element offsets
            if let Some(ref repeat) = field.repeat {
                let count = match repeat {
                    RepeatSpec::Count(n) => *n,
                    RepeatSpec::Expression(expr) => match self.evaluator.evaluate(expr, &ctx) {
                        Ok(EvalResult::Integer(n)) if n >= 0 => n as usize,
                        _ => 0,
                    },
                    RepeatSpec::Until(_) | RepeatSpec::Eos => 0,
                };

                match repeat {
                    RepeatSpec::Count(_) | RepeatSpec::Expression(_) => {
                        let mut elem_offset = current_offset;
                        // `count` is derived from (untrusted) field data, so don't
                        // pre-allocate for the full declared count — cap the initial
                        // capacity and let the vector grow as elements are read.
                        let mut elem_offsets = Vec::with_capacity(count.min(1000));
                        // Store base field offset
                        self.offset_cache
                            .borrow_mut()
                            .insert(field.id.clone(), (current_offset, size));
                        for _i in 0..count {
                            let elem_size = match &field.field_type {
                                FieldType::TypeRef(type_name) => self
                                    .get_type_size(type_name, elem_offset, &ctx)
                                    .unwrap_or(size),
                                _ => size,
                            };
                            // Once an element no longer fits, no later element can
                            // either (offsets only advance) — stop instead of spinning
                            // through a huge declared count against a short buffer.
                            if elem_offset + elem_size > self.data.len() {
                                break;
                            }
                            // Read value for eval context
                            if let Ok(value) = self.read_field_value(field, elem_offset, elem_size)
                            {
                                let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);
                            }
                            elem_offsets.push((elem_offset, elem_size));
                            elem_offset += elem_size;
                        }
                        self.repeat_offsets
                            .borrow_mut()
                            .insert(field.id.clone(), elem_offsets);
                        current_offset = elem_offset;
                    }
                    RepeatSpec::Until(until_expr) => {
                        let mut elem_offset = current_offset;
                        let mut elem_offsets = Vec::new();
                        self.offset_cache
                            .borrow_mut()
                            .insert(field.id.clone(), (current_offset, size));
                        let mut i = 0;
                        loop {
                            let elem_size = match &field.field_type {
                                FieldType::TypeRef(type_name) => self
                                    .get_type_size(type_name, elem_offset, &ctx)
                                    .unwrap_or(size),
                                _ => size,
                            };
                            if elem_offset + elem_size > self.data.len() {
                                break;
                            }
                            let value = self.read_field_value(field, elem_offset, elem_size)?;
                            let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);

                            // Check until condition
                            let mut until_ctx = ctx.clone();
                            until_ctx.index = Some(i);
                            let _ = add_value_to_context_impl(&mut until_ctx, "_", &value);

                            elem_offsets.push((elem_offset, elem_size));

                            if let Ok(EvalResult::Boolean(true)) =
                                self.evaluator.evaluate(until_expr, &until_ctx)
                            {
                                elem_offset += elem_size;
                                break;
                            }
                            i += 1;
                            elem_offset += elem_size;
                        }
                        self.repeat_offsets
                            .borrow_mut()
                            .insert(field.id.clone(), elem_offsets);
                        current_offset = elem_offset;
                    }
                    RepeatSpec::Eos => {
                        let mut elem_offset = current_offset;
                        let mut elem_offsets = Vec::new();
                        self.offset_cache
                            .borrow_mut()
                            .insert(field.id.clone(), (current_offset, size));
                        loop {
                            let elem_size = match &field.field_type {
                                FieldType::TypeRef(type_name) => self
                                    .get_type_size(type_name, elem_offset, &ctx)
                                    .unwrap_or(size),
                                _ => size,
                            };
                            if elem_size == 0 || elem_offset + elem_size > self.data.len() {
                                break;
                            }
                            if let Ok(value) = self.read_field_value(field, elem_offset, elem_size)
                            {
                                let _ = add_value_to_context_impl(&mut ctx, &field.id, &value);
                                elem_offsets.push((elem_offset, elem_size));
                            }
                            elem_offset += elem_size;
                        }
                        self.repeat_offsets
                            .borrow_mut()
                            .insert(field.id.clone(), elem_offsets);
                        current_offset = elem_offset;
                    }
                }
            } else {
                // Non-repeated field — store in offset cache
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
        *self.parsed.borrow_mut() = true;
        Ok(())
    }

    /// Access a field by dot-notation path.
    pub fn get(&self, path: &str) -> Result<Value<'a>, AccessError> {
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

        // Ensure parsed (populates offset_cache and repeat_offsets)
        self.ensure_parsed()?;

        // Handle repeated fields
        if field.repeat.is_some() {
            // Check if we have cached element offsets
            let elem_offsets = self.repeat_offsets.borrow().get(&field_name).cloned();
            if let Some(offsets) = elem_offsets {
                if let Some(idx) = index {
                    // Indexed access into array
                    if idx < offsets.len() {
                        let (elem_offset, elem_size) = offsets[idx];
                        let elem = self.read_field_value(field, elem_offset, elem_size)?;
                        if let Some(remaining) = rest {
                            return self.access_nested(&elem, &remaining);
                        }
                        return Ok(elem);
                    } else {
                        return Err(AccessError::UnknownField {
                            path: path.to_string(),
                        });
                    }
                } else {
                    // Return entire array
                    let mut values = Vec::with_capacity(offsets.len());
                    for (elem_offset, elem_size) in &offsets {
                        let value = self.read_field_value(field, *elem_offset, *elem_size)?;
                        values.push(value);
                    }
                    return Ok(Value::Array(values));
                }
            }

            // Fall back to offset-based access
            let (offset, size) = self.calculate_field_offset(&field_name, index)?;
            if let Some(_idx) = index {
                let element_value = self.read_field_value(field, offset, size)?;
                if let Some(remaining) = rest {
                    return self.access_nested(&element_value, &remaining);
                }
                return Ok(element_value);
            } else {
                let count = self.get_actual_repeat_count(
                    field.repeat.as_ref().unwrap(),
                    &field_name,
                    offset,
                    field,
                )?;
                let mut values = Vec::with_capacity(count.min(1000));
                let mut current_offset = offset;
                for i in 0..count {
                    let elem_size = self.get_element_size(field, current_offset)?;
                    if current_offset + elem_size > self.data.len() {
                        break;
                    }
                    let value = self.read_field_value(field, current_offset, elem_size)?;
                    values.push(value);
                    current_offset += elem_size;
                    if let RepeatSpec::Until(ref until_expr) = field.repeat.as_ref().unwrap() {
                        let mut ctx = self.build_eval_context()?;
                        ctx.index = Some(i);
                        self.add_value_to_context(&mut ctx, "_", &values[i])?;
                        let result = self.evaluator.evaluate(until_expr, &ctx).map_err(|e| {
                            AccessError::ExpressionError {
                                path: field_name.clone(),
                                message: e.to_string(),
                            }
                        })?;
                        if let EvalResult::Boolean(true) = result {
                            break;
                        }
                    }
                }
                return Ok(Value::Array(values));
            }
        }

        // Non-repeated field: read from offset cache
        let (offset, size) = self.calculate_field_offset(&field_name, index)?;
        let value = self.read_field_value(field, offset, size)?;
        if let Some(remaining) = rest {
            return self.access_nested(&value, &remaining);
        }
        Ok(value)
    }

    /// Check if a field exists and is accessible.
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
    pub fn fields(&self) -> impl Iterator<Item = String> + use<'_, 'a> {
        FieldIterator::new(self)
    }

    /// Get raw byte slice for a field (zero-copy).
    pub fn raw_slice(&self, path: &str) -> Result<&'a [u8], AccessError> {
        let (field_name, index, rest) = self.parse_path(path);

        if let Ok(field) = self.find_field(&field_name) {
            if field.repeat.is_some() && index.is_none() {
                return Err(AccessError::NonContiguous {
                    path: path.to_string(),
                });
            }
        }

        if rest.is_some() {
            return Err(AccessError::NonContiguous {
                path: path.to_string(),
            });
        }

        let (offset, size) = self.calculate_field_offset(&field_name, index)?;
        if offset + size > self.data.len() {
            return Err(AccessError::UnexpectedEof {
                path: path.to_string(),
                expected: size,
                available: self.data.len().saturating_sub(offset),
            });
        }
        Ok(&self.data[offset..offset + size])
    }

    // ==================== Private Helper Methods ====================

    /// Parse a field path into components.
    /// Returns (field_name, optional_index, optional_remaining_path)
    fn parse_path(&self, path: &str) -> (String, Option<usize>, Option<String>) {
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
    pub(crate) fn calculate_field_offset(
        &self,
        field_name: &str,
        _index: Option<usize>,
    ) -> Result<(usize, usize), AccessError> {
        // Fast path: already cached
        if let Some(&(offset, size)) = self.offset_cache.borrow().get(field_name) {
            return Ok((offset, size));
        }

        // Trigger single-pass parse to populate all caches
        self.ensure_parsed()?;

        if let Some(&(offset, size)) = self.offset_cache.borrow().get(field_name) {
            return Ok((offset, size));
        }

        if let Ok(field) = self.find_field(field_name) {
            if let Some(ref condition) = field.condition {
                return Err(AccessError::ConditionalNotPresent {
                    path: field_name.to_string(),
                    condition: format!("{:?}", condition),
                });
            }
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
                    match &field.field_type {
                        FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                            Ok(*bytes as usize)
                        }
                        FieldType::TypeRef(type_name) => {
                            let ctx = self.build_eval_context()?;
                            self.get_type_size(type_name, offset, &ctx)
                        }
                        _ => Ok(0),
                    }
                } else {
                    Ok(*size)
                }
            }
            SizeSpec::Expression(expr) => {
                let ctx = self.build_eval_context()?;
                let result = self.evaluator.evaluate(expr, &ctx).map_err(|e| {
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
            SizeSpec::Eos => Ok(self.data.len().saturating_sub(offset)),
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
    pub(crate) fn get_type_size(
        &self,
        type_name: &str,
        offset: usize,
        ctx: &EvalContext,
    ) -> Result<usize, AccessError> {
        let nested_def =
            self.definition
                .types
                .get(type_name)
                .ok_or_else(|| AccessError::UnknownField {
                    path: type_name.to_string(),
                })?;

        let mut nested_ctx = ctx.clone();
        let mut total_size = 0;

        for field in &nested_def.fields {
            if let Some(ref condition) = field.condition {
                let result = self.evaluator.evaluate(condition, &nested_ctx);
                match result {
                    Ok(EvalResult::Boolean(false)) => continue,
                    Ok(EvalResult::Boolean(true)) => {}
                    Err(_) => continue,
                    _ => continue,
                }
            }

            let field_size = self.get_nested_field_size(field, &nested_ctx, offset + total_size)?;

            if offset + total_size + field_size <= self.data.len() {
                let field_data = &self.data[offset + total_size..offset + total_size + field_size];
                if let Ok(value) = self.read_simple_nested_value(field, field_data) {
                    let _ = self.add_value_to_context(&mut nested_ctx, &field.id, &value);
                }
            }

            let total_field_size = self.get_nested_total_field_size(
                field,
                &nested_ctx,
                field_size,
                offset + total_size,
            )?;
            total_size += total_field_size;
        }

        Ok(total_size)
    }

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
                        if let FieldType::TypeRef(type_name) = &field.field_type {
                            let mut total = 0;
                            let mut current_offset = base_offset;
                            for _ in 0..(n as usize) {
                                let elem_size =
                                    self.get_type_size(type_name, current_offset, ctx)?;
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
                        message: "Repeat expression did not evaluate to positive integer"
                            .to_string(),
                    }),
                }
            }
            Some(RepeatSpec::Until(_)) | Some(RepeatSpec::Eos) => Ok(element_size),
        }
    }

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
                        FieldType::TypeRef(type_name) => self.get_type_size(type_name, offset, ctx),
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
            SizeSpec::Eos => Ok(self.data.len().saturating_sub(offset)),
        }
    }

    fn read_simple_nested_value<'b>(
        &self,
        field: &FieldDefinition,
        data: &'b [u8],
    ) -> Result<Value<'b>, AccessError> {
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
                    3 if data.len() >= 3 => {
                        u32::from_be_bytes([0, data[0], data[1], data[2]]) as u64
                    }
                    4 if data.len() >= 4 => {
                        u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64
                    }
                    8 if data.len() >= 8 => u64::from_be_bytes([
                        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                    ]),
                    _ => 0,
                };
                Ok(Value::Unsigned(n))
            }
            FieldType::SignedInt(bytes) => {
                let n: i64 = match bytes {
                    1 => data.first().map(|&b| b as i8 as i64).unwrap_or(0),
                    2 if data.len() >= 2 => i16::from_be_bytes([data[0], data[1]]) as i64,
                    3 if data.len() >= 3 => {
                        // Sign-extend a 24-bit value into i32 before widening.
                        let raw = u32::from_be_bytes([0, data[0], data[1], data[2]]);
                        ((raw << 8) as i32 >> 8) as i64
                    }
                    4 if data.len() >= 4 => {
                        i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64
                    }
                    8 if data.len() >= 8 => i64::from_be_bytes([
                        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                    ]),
                    _ => 0,
                };
                Ok(Value::Signed(n))
            }
            FieldType::Float(byte_size) => {
                // Like the integer arms above, this probing helper assumes
                // big-endian (every NITF/BIIF structure is BE). The authoritative
                // read paths honor the structure's declared endian via read_float.
                if data.len() < *byte_size as usize {
                    return Err(AccessError::UnknownField {
                        path: field.id.clone(),
                    });
                }
                Ok(Value::Float(read_float(data, *byte_size, Endian::Big)?))
            }
            FieldType::TypeRef(_) => Err(AccessError::UnknownField {
                path: field.id.clone(),
            }),
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
                let result = self.evaluator.evaluate(expr, &ctx).map_err(|e| {
                    AccessError::ExpressionError {
                        path: field_name.to_string(),
                        message: e.to_string(),
                    }
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
                let mut count = 0;
                let mut current_offset = start_offset;
                loop {
                    let elem_size = self.get_element_size(field, current_offset)?;
                    if current_offset + elem_size > self.data.len() {
                        break;
                    }
                    let value = self.read_field_value(field, current_offset, elem_size)?;
                    count += 1;
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
        if let Some(ref ctx) = *self.parsed_context.borrow() {
            return Ok(ctx.clone());
        }
        self.build_eval_context_up_to("")
    }

    pub(crate) fn build_eval_context_up_to(
        &self,
        stop_at: &str,
    ) -> Result<EvalContext, AccessError> {
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
                let nested_def = self
                    .definition
                    .types
                    .get(&struct_val.type_name)
                    .ok_or_else(|| AccessError::UnknownField {
                        path: struct_val.type_name.clone(),
                    })?;

                let (field_name, _index, rest) = self.parse_path(path);

                let field = nested_def
                    .fields
                    .iter()
                    .find(|f| f.id == field_name)
                    .ok_or_else(|| AccessError::UnknownField {
                        path: field_name.clone(),
                    })?;

                let mut offset = 0;
                for f in &nested_def.fields {
                    if f.id == field_name {
                        break;
                    }
                    let size = match &f.size {
                        SizeSpec::Fixed(s) => *s,
                        SizeSpec::Eos => struct_val.data.len().saturating_sub(offset),
                        _ => match &f.field_type {
                            FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                                *bytes as usize
                            }
                            _ => 0,
                        },
                    };
                    offset += size;
                }

                let size = match &field.size {
                    SizeSpec::Fixed(s) => *s,
                    SizeSpec::Eos => struct_val.data.len().saturating_sub(offset),
                    _ => match &field.field_type {
                        FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                            *bytes as usize
                        }
                        _ => 0,
                    },
                };

                if offset + size > struct_val.data.len() {
                    return Err(AccessError::UnexpectedEof {
                        path: path.to_string(),
                        expected: size,
                        available: struct_val.data.len().saturating_sub(offset),
                    });
                }

                let bytes = &struct_val.data[offset..offset + size];

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
                        Value::Signed(val)
                    }
                    FieldType::Float(byte_size) => {
                        let val = read_float(bytes, *byte_size, nested_def.endian)?;
                        Value::Float(val)
                    }
                    FieldType::TypeRef(type_name) => Value::from_struct(bytes, type_name.clone()),
                };

                if let Some(remaining) = rest {
                    return self.access_nested(&value, &remaining);
                }

                Ok(value)
            }
            Value::Array(arr) => {
                let (field_name, arr_index, rest) = self.parse_path(path);
                if let Some(idx) = arr_index {
                    if idx < arr.len() {
                        if let Some(remaining) = rest {
                            return self.access_nested(&arr[idx], &remaining);
                        }
                        return Ok(arr[idx].clone());
                    }
                }
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

impl<'a> std::ops::Index<&str> for StructureAccessor<'a> {
    type Output = Value<'a>;

    fn index(&self, path: &str) -> &Self::Output {
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
