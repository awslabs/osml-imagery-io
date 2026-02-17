//! Expression evaluation logic.

use std::collections::HashMap;

use super::ops::{
    eval_add, eval_compare, eval_div, eval_logical_and, eval_logical_or, eval_mod, eval_mul,
    eval_sub, values_equal,
};
use super::parser::Parser;
use super::{BinaryOperator, EvalResult, Expression, Literal, SpecialVariable, UnaryOperator};
use crate::parser::error::ExpressionError;

/// Context for expression evaluation containing field values.
#[derive(Debug, Clone)]
pub struct EvalContext {
    /// Field values by path
    pub fields: HashMap<String, EvalResult>,
    /// Current repetition index (for _index)
    pub index: Option<usize>,
}

impl EvalContext {
    /// Create a new empty evaluation context.
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            index: None,
        }
    }

    /// Set a field value.
    pub fn with_field(mut self, path: impl Into<String>, value: EvalResult) -> Self {
        self.fields.insert(path.into(), value);
        self
    }

    /// Set the current index.
    pub fn with_index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }
}

impl Default for EvalContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluates expressions in the context of a structure.
pub struct ExpressionEvaluator;

impl ExpressionEvaluator {
    /// Create a new expression evaluator.
    pub fn new() -> Self {
        Self
    }

    /// Parse an expression from a string.
    pub fn parse(expr: &str) -> Result<Expression, ExpressionError> {
        use super::lexer::Token;

        if expr.trim().is_empty() {
            return Err(ExpressionError::SyntaxError {
                message: "Empty expression".to_string(),
            });
        }
        let mut parser = Parser::new(expr)?;
        let result = parser.parse_expression()?;
        // Ensure we consumed all input
        if *parser.current_token() != Token::Eof {
            return Err(ExpressionError::SyntaxError {
                message: format!(
                    "Unexpected token after expression: {:?}",
                    parser.current_token()
                ),
            });
        }
        Ok(result)
    }

    /// Evaluate an expression to a value given a context.
    pub fn evaluate(
        &self,
        expr: &Expression,
        context: &EvalContext,
    ) -> Result<EvalResult, ExpressionError> {
        match expr {
            Expression::Literal(lit) => Ok(match lit {
                Literal::Integer(n) => EvalResult::Integer(*n),
                Literal::Float(f) => EvalResult::Float(*f),
                Literal::String(s) => EvalResult::String(s.clone()),
                Literal::Boolean(b) => EvalResult::Boolean(*b),
            }),
            Expression::FieldRef(path) => context
                .fields
                .get(path)
                .cloned()
                .ok_or_else(|| ExpressionError::UnknownField {
                    field: path.clone(),
                }),
            Expression::SpecialVar(var) => match var {
                SpecialVariable::Index => context
                    .index
                    .map(|i| EvalResult::Integer(i as i64))
                    .ok_or_else(|| ExpressionError::UnknownField {
                        field: "_index".to_string(),
                    }),
                SpecialVariable::Root | SpecialVariable::Parent | SpecialVariable::Io => {
                    Err(ExpressionError::UnknownField {
                        field: format!("{:?}", var),
                    })
                }
            },
            Expression::BinaryOp { left, op, right } => {
                let left_val = self.evaluate(left, context)?;
                let right_val = self.evaluate(right, context)?;
                self.eval_binary_op(*op, left_val, right_val)
            }
            Expression::UnaryOp { op, operand } => {
                let val = self.evaluate(operand, context)?;
                self.eval_unary_op(*op, val)
            }
            Expression::MethodCall { target, method } => {
                let val = self.evaluate(target, context)?;
                self.eval_method_call(val, method)
            }
        }
    }

    fn eval_binary_op(
        &self,
        op: BinaryOperator,
        left: EvalResult,
        right: EvalResult,
    ) -> Result<EvalResult, ExpressionError> {
        match op {
            // Arithmetic operators
            BinaryOperator::Add => eval_add(left, right),
            BinaryOperator::Sub => eval_sub(left, right),
            BinaryOperator::Mul => eval_mul(left, right),
            BinaryOperator::Div => eval_div(left, right),
            BinaryOperator::Mod => eval_mod(left, right),
            // Comparison operators
            BinaryOperator::Eq => Ok(EvalResult::Boolean(values_equal(&left, &right))),
            BinaryOperator::Ne => Ok(EvalResult::Boolean(!values_equal(&left, &right))),
            BinaryOperator::Lt => eval_compare(left, right, |a, b| a < b, |a, b| a < b),
            BinaryOperator::Gt => eval_compare(left, right, |a, b| a > b, |a, b| a > b),
            BinaryOperator::Le => eval_compare(left, right, |a, b| a <= b, |a, b| a <= b),
            BinaryOperator::Ge => eval_compare(left, right, |a, b| a >= b, |a, b| a >= b),
            // Logical operators
            BinaryOperator::And => eval_logical_and(left, right),
            BinaryOperator::Or => eval_logical_or(left, right),
        }
    }

    fn eval_unary_op(
        &self,
        op: UnaryOperator,
        val: EvalResult,
    ) -> Result<EvalResult, ExpressionError> {
        match op {
            UnaryOperator::Not => match val {
                EvalResult::Boolean(b) => Ok(EvalResult::Boolean(!b)),
                v => Err(ExpressionError::TypeError {
                    operator: "not".to_string(),
                    operand_type: format!("{:?}", v),
                }),
            },
            UnaryOperator::Neg => match val {
                EvalResult::Integer(n) => Ok(EvalResult::Integer(-n)),
                EvalResult::Float(f) => Ok(EvalResult::Float(-f)),
                v => Err(ExpressionError::TypeError {
                    operator: "-".to_string(),
                    operand_type: format!("{:?}", v),
                }),
            },
        }
    }

    fn eval_method_call(
        &self,
        val: EvalResult,
        method: &str,
    ) -> Result<EvalResult, ExpressionError> {
        match method {
            "to_i" => match val {
                EvalResult::Integer(n) => Ok(EvalResult::Integer(n)),
                EvalResult::Float(f) => Ok(EvalResult::Integer(f as i64)),
                EvalResult::String(s) => s.trim().parse::<i64>().map(EvalResult::Integer).map_err(
                    |_| ExpressionError::TypeError {
                        operator: "to_i".to_string(),
                        operand_type: format!("String({})", s),
                    },
                ),
                EvalResult::Boolean(b) => Ok(EvalResult::Integer(if b { 1 } else { 0 })),
                v => Err(ExpressionError::TypeError {
                    operator: "to_i".to_string(),
                    operand_type: format!("{:?}", v),
                }),
            },
            "to_s" => match val {
                EvalResult::Integer(n) => Ok(EvalResult::String(n.to_string())),
                EvalResult::Float(f) => Ok(EvalResult::String(f.to_string())),
                EvalResult::String(s) => Ok(EvalResult::String(s)),
                EvalResult::Boolean(b) => Ok(EvalResult::String(b.to_string())),
                EvalResult::Bytes(b) => {
                    Ok(EvalResult::String(String::from_utf8_lossy(&b).to_string()))
                }
            },
            "length" => match val {
                EvalResult::String(s) => Ok(EvalResult::Integer(s.len() as i64)),
                EvalResult::Bytes(b) => Ok(EvalResult::Integer(b.len() as i64)),
                v => Err(ExpressionError::TypeError {
                    operator: "length".to_string(),
                    operand_type: format!("{:?}", v),
                }),
            },
            _ => Err(ExpressionError::SyntaxError {
                message: format!("Unknown method: {}", method),
            }),
        }
    }
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
