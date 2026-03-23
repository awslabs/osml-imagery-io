//! Field iterator for traversing structure fields.
//!
//! This module provides an iterator over accessible field paths.
//! Repeated fields yield the base field name once (e.g., `"BAND_INFO"`),
//! and callers use `get()` to obtain a `Value::Array` of all elements.

use crate::parser::expression::EvalResult;

use super::StructureAccessor;

/// Iterator over accessible field paths.
///
/// Yields each field's base name once. For repeated fields, the base name
/// is yielded (e.g., `"BAND_INFO"`) rather than expanded indexed names
/// (`"BAND_INFO_0"`, `"BAND_INFO_1"`, etc.). Callers should use
/// `accessor.get(field_id)` which returns `Value::Array` for repeated fields.
pub struct FieldIterator<'a, 'b> {
    accessor: &'b StructureAccessor<'a>,
    field_index: usize,
}

impl<'a, 'b> FieldIterator<'a, 'b> {
    /// Create a new field iterator for the given accessor.
    pub fn new(accessor: &'b StructureAccessor<'a>) -> Self {
        Self {
            accessor,
            field_index: 0,
        }
    }
}

impl<'a, 'b> Iterator for FieldIterator<'a, 'b> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        while self.field_index < self.accessor.definition.fields.len() {
            let field = &self.accessor.definition.fields[self.field_index];
            self.field_index += 1;

            // Check if field is accessible (condition met)
            if let Some(ref condition) = field.condition {
                if let Ok(ctx) = self.accessor.build_eval_context() {
                    if let Ok(EvalResult::Boolean(false)) =
                        self.accessor.evaluator.evaluate(condition, &ctx)
                    {
                        continue;
                    }
                }
            }

            // For repeated fields, yield the base field name once
            if field.repeat.is_some() {
                return Some(field.id.clone());
            }

            // Non-repeated field — verify it is accessible
            let path = field.id.clone();
            if self.accessor.has(&path) {
                return Some(path);
            }
        }

        None
    }
}
