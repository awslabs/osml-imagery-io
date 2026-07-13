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
//! - Method calls: `.to_i`, `.to_s`, `.length`, `.strip`
//! - Special variables: `_index`
//! - Literals: integers, floats, strings, booleans
//!
//! Field resolution is flat: a bare name resolves against the merged scope
//! (consts < inherited enclosing scalars < local fields). The `_root`,
//! `_parent`, and `_io` navigators were removed — nested expressions reference
//! enclosing fields by bare name, resolved via the inherited-scope seeding in
//! `StructureAccessor`/`StructureWriter`.

pub(crate) mod context;
mod eval;
pub(crate) mod lexer;
mod ops;
pub(crate) mod parser;

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod tests;

// Re-export public types
pub use context::{EvalContext, Node};
pub use eval::ExpressionEvaluator;

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
    /// Method call (.to_i, .to_s, .length, .strip)
    MethodCall {
        target: Box<Expression>,
        method: String,
    },
    /// Postfix subscript: `base[index]`. Navigates a context node — a const map
    /// keyed by string, or an array indexed numerically — with the result
    /// composing with a following `.member` access. Parsing only in Phase 1;
    /// evaluation lands with the shared node-context refactor (Phases 2–3, 6).
    Index {
        base: Box<Expression>,
        index: Box<Expression>,
    },
    /// Special variable (_index)
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

impl EvalResult {
    /// Coerce this scalar to a string key for a const-map subscript.
    ///
    /// A `String` is used verbatim; an `Integer`/`Float`/`Boolean` is rendered
    /// so numeric-keyed maps also work. `Bytes` has no sensible key form and is
    /// a type error. Used by the `[]` map-lookup arm to build the lookup key.
    pub fn expect_string(&self) -> Result<String, crate::parser::error::ExpressionError> {
        match self {
            EvalResult::String(s) => Ok(s.clone()),
            EvalResult::Integer(n) => Ok(n.to_string()),
            EvalResult::Float(f) => Ok(f.to_string()),
            EvalResult::Boolean(b) => Ok(b.to_string()),
            EvalResult::Bytes(_) => Err(crate::parser::error::ExpressionError::TypeError {
                operator: "[] key".to_string(),
                operand_type: "Bytes".to_string(),
            }),
        }
    }

    /// Coerce this scalar to an integer index for an array subscript.
    ///
    /// An `Integer` is used verbatim; a `Float` is truncated toward zero and a
    /// numeric `String` is parsed (BCS-N fields decode to strings, so an index
    /// expression like `IMAGE_RECORDS[_index]` where `_index` comes from a count
    /// field can be a string). `Boolean`/`Bytes` have no index form and are a
    /// type error. Used by the `[]` array-index arm.
    pub fn expect_integer(&self) -> Result<i64, crate::parser::error::ExpressionError> {
        match self {
            EvalResult::Integer(n) => Ok(*n),
            EvalResult::Float(f) => Ok(*f as i64),
            EvalResult::String(s) => {
                s.trim()
                    .parse::<i64>()
                    .map_err(|_| crate::parser::error::ExpressionError::TypeError {
                        operator: "[] index".to_string(),
                        operand_type: format!("String({})", s),
                    })
            }
            other => Err(crate::parser::error::ExpressionError::TypeError {
                operator: "[] index".to_string(),
                operand_type: format!("{:?}", other),
            }),
        }
    }
}
