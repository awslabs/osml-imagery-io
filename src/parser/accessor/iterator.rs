//! Field iterator for traversing structure fields.
//!
//! This module provides an iterator over accessible field paths,
//! including expanded paths for repeated fields.

use crate::parser::expression::EvalResult;

use super::StructureAccessor;

/// Iterator over accessible field paths.
pub struct FieldIterator<'a, 'b> {
    accessor: &'b StructureAccessor<'a>,
    field_index: usize,
    repeat_index: Option<usize>,
    pending_paths: Vec<String>,
}

impl<'a, 'b> FieldIterator<'a, 'b> {
    /// Create a new field iterator for the given accessor.
    pub fn new(accessor: &'b StructureAccessor<'a>) -> Self {
        Self {
            accessor,
            field_index: 0,
            repeat_index: None,
            pending_paths: Vec::new(),
        }
    }
}

impl<'a, 'b> Iterator for FieldIterator<'a, 'b> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        // Return pending paths first
        if let Some(path) = self.pending_paths.pop() {
            return Some(path);
        }

        while self.field_index < self.accessor.definition.fields.len() {
            let field = &self.accessor.definition.fields[self.field_index];

            // Check if field is accessible (condition met)
            if let Some(ref condition) = field.condition {
                if let Ok(ctx) = self.accessor.build_eval_context() {
                    if let Ok(EvalResult::Boolean(false)) =
                        self.accessor.evaluator.evaluate(condition, &ctx)
                    {
                        self.field_index += 1;
                        continue;
                    }
                }
            }

            // Handle repeated fields
            if let Some(ref repeat) = field.repeat {
                if let Some(idx) = self.repeat_index {
                    // Get actual count by calculating field offset first
                    let offset = self.accessor.calculate_field_offset(&field.id, None).ok();
                    let count = if let Some((off, _)) = offset {
                        self.accessor
                            .get_actual_repeat_count(repeat, &field.id, off, field)
                            .unwrap_or(0)
                    } else {
                        0
                    };

                    if idx < count {
                        let path = format!("{}_{}", field.id, idx);
                        self.repeat_index = Some(idx + 1);
                        return Some(path);
                    }

                    // Move to next field
                    self.repeat_index = None;
                    self.field_index += 1;
                } else {
                    // Start iterating repeated field
                    self.repeat_index = Some(0);
                    continue;
                }
            } else {
                // Non-repeated field
                let path = field.id.clone();
                self.field_index += 1;

                // Verify field is accessible
                if self.accessor.has(&path) {
                    return Some(path);
                }
            }
        }

        None
    }
}
