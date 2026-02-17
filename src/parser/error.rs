//! Error types for the data-driven binary parser.
//!
//! This module defines error types for all parser operations including
//! definition loading, structure access, value conversion, writing, and
//! expression evaluation.

use thiserror::Error;

/// Errors during structure definition loading.
#[derive(Error, Debug)]
pub enum LoadError {
    /// YAML syntax error
    #[error("YAML parse error: {message}")]
    YamlError {
        /// Error message from the YAML parser
        message: String,
    },

    /// Missing required field in definition
    #[error("Missing required field '{field}' in {context}")]
    MissingField {
        /// Name of the missing field
        field: String,
        /// Context where the field was expected
        context: String,
    },

    /// Invalid field type specification
    #[error("Invalid type '{type_str}' in {context}")]
    InvalidType {
        /// The invalid type string
        type_str: String,
        /// Context where the type was found
        context: String,
    },

    /// Reference to undefined type
    #[error("Undefined type '{type_name}' referenced in {context}")]
    UndefinedType {
        /// Name of the undefined type
        type_name: String,
        /// Context where the reference was found
        context: String,
    },

    /// Invalid expression syntax
    #[error("Invalid expression '{expr}': {message}")]
    InvalidExpression {
        /// The invalid expression string
        expr: String,
        /// Error message describing the problem
        message: String,
    },

    /// I/O error reading file
    #[error("I/O error: {source}")]
    IoError {
        /// The underlying I/O error
        #[from]
        source: std::io::Error,
    },
}

/// Errors during structure access (reading).
#[derive(Error, Debug)]
pub enum AccessError {
    /// Unknown field path
    #[error("Unknown field path: '{path}'")]
    UnknownField {
        /// The invalid field path
        path: String,
    },

    /// Unexpected end of data
    #[error("Unexpected end of data at '{path}': expected {expected} bytes, got {available}")]
    UnexpectedEof {
        /// Field path where EOF occurred
        path: String,
        /// Expected number of bytes
        expected: usize,
        /// Actually available bytes
        available: usize,
    },

    /// Conditional field not present
    #[error("Conditional field '{path}' not present (condition: {condition})")]
    ConditionalNotPresent {
        /// Field path
        path: String,
        /// The condition expression that evaluated to false
        condition: String,
    },

    /// Encoding validation error
    #[error("Encoding error at '{path}' ({encoding}): {message}")]
    EncodingError {
        /// Field path where the error occurred
        path: String,
        /// Expected encoding (e.g., "BCS-A", "BCS-N")
        encoding: String,
        /// Description of the encoding violation
        message: String,
    },

    /// Expression evaluation failed
    #[error("Expression evaluation failed at '{path}': {message}")]
    ExpressionError {
        /// Field path where the expression was evaluated
        path: String,
        /// Error message from expression evaluation
        message: String,
    },

    /// Field is not contiguous in memory
    #[error("Field '{path}' is not contiguous in memory")]
    NonContiguous {
        /// Field path
        path: String,
    },
}

/// Errors during value conversion.
#[derive(Error, Debug)]
pub enum ConversionError {
    /// Type mismatch during conversion
    #[error("Cannot convert {from_type} to {to_type}")]
    TypeMismatch {
        /// Source type name
        from_type: &'static str,
        /// Target type name
        to_type: &'static str,
    },

    /// Failed to parse value
    #[error("Failed to parse '{value}' as {target_type}: {message}")]
    ParseError {
        /// The value that failed to parse
        value: String,
        /// Target type name
        target_type: &'static str,
        /// Description of the parse failure
        message: String,
    },
}

/// Errors during structure writing.
#[derive(Error, Debug)]
pub enum WriteError {
    /// Field written out of order in streaming mode
    #[error("Field '{path}' written out of order (expected after '{expected_after}')")]
    OutOfOrder {
        /// Field path that was written out of order
        path: String,
        /// The field that should have been written first
        expected_after: String,
    },

    /// Value too large for field
    #[error("Value too large for field '{path}': max {max_size} bytes, got {actual_size}")]
    ValueTooLarge {
        /// Field path
        path: String,
        /// Maximum allowed size in bytes
        max_size: usize,
        /// Actual size of the value
        actual_size: usize,
    },

    /// Required field not written
    #[error("Required field '{path}' not written")]
    MissingRequired {
        /// Field path of the missing required field
        path: String,
    },

    /// Validation error during write
    #[error("Invalid value for field '{path}': {message}")]
    ValidationError {
        /// Field path
        path: String,
        /// Description of the validation failure
        message: String,
    },

    /// Conversion error during write
    #[error("Conversion error for field '{path}': {message}")]
    ConversionError {
        /// Field path
        path: String,
        /// Description of the conversion failure
        message: String,
    },
}

/// Errors during expression evaluation.
#[derive(Error, Debug)]
pub enum ExpressionError {
    /// Syntax error in expression
    #[error("Syntax error in expression: {message}")]
    SyntaxError {
        /// Description of the syntax error
        message: String,
    },

    /// Unknown field reference in expression
    #[error("Unknown field reference: '{field}'")]
    UnknownField {
        /// The unknown field name
        field: String,
    },

    /// Type error during evaluation
    #[error("Type error: cannot apply {operator} to {operand_type}")]
    TypeError {
        /// The operator that failed
        operator: String,
        /// The type of the operand
        operand_type: String,
    },

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_error_yaml_error_display() {
        let err = LoadError::YamlError {
            message: "unexpected character".to_string(),
        };
        assert_eq!(err.to_string(), "YAML parse error: unexpected character");
    }

    #[test]
    fn load_error_missing_field_display() {
        let err = LoadError::MissingField {
            field: "id".to_string(),
            context: "meta section".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Missing required field 'id' in meta section"
        );
    }

    #[test]
    fn load_error_undefined_type_display() {
        let err = LoadError::UndefinedType {
            type_name: "custom_header".to_string(),
            context: "field 'header'".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Undefined type 'custom_header' referenced in field 'header'"
        );
    }

    #[test]
    fn access_error_unknown_field_display() {
        let err = AccessError::UnknownField {
            path: "header.invalid".to_string(),
        };
        assert_eq!(err.to_string(), "Unknown field path: 'header.invalid'");
    }

    #[test]
    fn access_error_unexpected_eof_display() {
        let err = AccessError::UnexpectedEof {
            path: "data".to_string(),
            expected: 100,
            available: 50,
        };
        assert_eq!(
            err.to_string(),
            "Unexpected end of data at 'data': expected 100 bytes, got 50"
        );
    }

    #[test]
    fn conversion_error_type_mismatch_display() {
        let err = ConversionError::TypeMismatch {
            from_type: "String",
            to_type: "i64",
        };
        assert_eq!(err.to_string(), "Cannot convert String to i64");
    }

    #[test]
    fn conversion_error_parse_error_display() {
        let err = ConversionError::ParseError {
            value: "abc".to_string(),
            target_type: "i64",
            message: "invalid digit".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Failed to parse 'abc' as i64: invalid digit"
        );
    }

    #[test]
    fn write_error_out_of_order_display() {
        let err = WriteError::OutOfOrder {
            path: "field_b".to_string(),
            expected_after: "field_a".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Field 'field_b' written out of order (expected after 'field_a')"
        );
    }

    #[test]
    fn write_error_value_too_large_display() {
        let err = WriteError::ValueTooLarge {
            path: "name".to_string(),
            max_size: 10,
            actual_size: 15,
        };
        assert_eq!(
            err.to_string(),
            "Value too large for field 'name': max 10 bytes, got 15"
        );
    }

    #[test]
    fn write_error_missing_required_display() {
        let err = WriteError::MissingRequired {
            path: "id".to_string(),
        };
        assert_eq!(err.to_string(), "Required field 'id' not written");
    }

    #[test]
    fn expression_error_syntax_error_display() {
        let err = ExpressionError::SyntaxError {
            message: "unexpected token".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Syntax error in expression: unexpected token"
        );
    }

    #[test]
    fn expression_error_division_by_zero_display() {
        let err = ExpressionError::DivisionByZero;
        assert_eq!(err.to_string(), "Division by zero");
    }
}
