//! Expression parsing and evaluation.
//!
//! The expression evaluator supports field references, arithmetic, comparison,
//! and logical operators for computed values, conditionals, and repeat counts.
//!
//! # Expression Syntax
//!
//! The parser supports a subset of Kaitai Struct expression syntax:
//! - Field references: `field_name`, `parent.child`, `arr_0.field`
//! - Arithmetic: `+`, `-`, `*`, `/`, `%`
//! - Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
//! - Logical: `and`, `or`, `not`
//! - Method calls: `.to_i`, `.to_s`, `.length`
//! - Special variables: `_index`, `_root`, `_parent`, `_io`
//! - Literals: integers, floats, strings, booleans

mod eval;
pub(crate) mod lexer;
mod ops;
pub(crate) mod parser;

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod tests;

// Re-export public types
pub use eval::{EvalContext, ExpressionEvaluator};

/// Parsed expression AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// Literal value
    Literal(Literal),
    /// Field reference (dot-notation path)
    FieldRef(String),
    /// Binary operation
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    /// Unary operation
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },
    /// Method call (.to_i, .to_s, .length)
    MethodCall {
        target: Box<Expression>,
        method: String,
    },
    /// Special variable (_index, _root, _parent, _io)
    SpecialVar(SpecialVariable),
}

/// Literal values in expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer literal
    Integer(i64),
    /// Float literal
    Float(f64),
    /// String literal
    String(String),
    /// Boolean literal
    Boolean(bool),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    // Logical
    And,
    Or,
    // Bitwise
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    ShiftLeft,
    ShiftRight,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Neg,
    BitwiseNot,
}

/// Special variables available in expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialVariable {
    /// Current repetition index
    Index,
    /// Root structure
    Root,
    /// Parent structure
    Parent,
    /// I/O stream info (pos, size, eof)
    Io,
}

/// Result of expression evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalResult {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Bytes(Vec<u8>),
}
