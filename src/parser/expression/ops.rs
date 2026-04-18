//! Binary and unary operator evaluation helpers.

use super::EvalResult;
use crate::parser::error::ExpressionError;

/// Evaluate addition operation.
pub(crate) fn eval_add(left: EvalResult, right: EvalResult) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Integer(a + b)),
        (EvalResult::Float(a), EvalResult::Float(b)) => Ok(EvalResult::Float(a + b)),
        (EvalResult::Integer(a), EvalResult::Float(b)) => Ok(EvalResult::Float(a as f64 + b)),
        (EvalResult::Float(a), EvalResult::Integer(b)) => Ok(EvalResult::Float(a + b as f64)),
        (EvalResult::String(a), EvalResult::String(b)) => Ok(EvalResult::String(a + &b)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "+".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate subtraction operation.
pub(crate) fn eval_sub(left: EvalResult, right: EvalResult) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Integer(a - b)),
        (EvalResult::Float(a), EvalResult::Float(b)) => Ok(EvalResult::Float(a - b)),
        (EvalResult::Integer(a), EvalResult::Float(b)) => Ok(EvalResult::Float(a as f64 - b)),
        (EvalResult::Float(a), EvalResult::Integer(b)) => Ok(EvalResult::Float(a - b as f64)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "-".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate multiplication operation.
pub(crate) fn eval_mul(left: EvalResult, right: EvalResult) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Integer(a * b)),
        (EvalResult::Float(a), EvalResult::Float(b)) => Ok(EvalResult::Float(a * b)),
        (EvalResult::Integer(a), EvalResult::Float(b)) => Ok(EvalResult::Float(a as f64 * b)),
        (EvalResult::Float(a), EvalResult::Integer(b)) => Ok(EvalResult::Float(a * b as f64)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "*".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate division operation.
pub(crate) fn eval_div(left: EvalResult, right: EvalResult) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => {
            if b == 0 {
                Err(ExpressionError::DivisionByZero)
            } else {
                Ok(EvalResult::Integer(a / b))
            }
        }
        (EvalResult::Float(a), EvalResult::Float(b)) => {
            if b == 0.0 {
                Err(ExpressionError::DivisionByZero)
            } else {
                Ok(EvalResult::Float(a / b))
            }
        }
        (EvalResult::Integer(a), EvalResult::Float(b)) => {
            if b == 0.0 {
                Err(ExpressionError::DivisionByZero)
            } else {
                Ok(EvalResult::Float(a as f64 / b))
            }
        }
        (EvalResult::Float(a), EvalResult::Integer(b)) => {
            if b == 0 {
                Err(ExpressionError::DivisionByZero)
            } else {
                Ok(EvalResult::Float(a / b as f64))
            }
        }
        (l, r) => Err(ExpressionError::TypeError {
            operator: "/".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate modulo operation.
pub(crate) fn eval_mod(left: EvalResult, right: EvalResult) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => {
            if b == 0 {
                Err(ExpressionError::DivisionByZero)
            } else {
                Ok(EvalResult::Integer(a % b))
            }
        }
        (l, r) => Err(ExpressionError::TypeError {
            operator: "%".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Check if two values are equal.
pub(crate) fn values_equal(left: &EvalResult, right: &EvalResult) -> bool {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => a == b,
        (EvalResult::Float(a), EvalResult::Float(b)) => (a - b).abs() < f64::EPSILON,
        (EvalResult::Integer(a), EvalResult::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
        (EvalResult::Float(a), EvalResult::Integer(b)) => (a - *b as f64).abs() < f64::EPSILON,
        (EvalResult::String(a), EvalResult::String(b)) => a == b,
        (EvalResult::Boolean(a), EvalResult::Boolean(b)) => a == b,
        (EvalResult::Bytes(a), EvalResult::Bytes(b)) => a == b,
        _ => false,
    }
}

/// Evaluate comparison operation.
pub(crate) fn eval_compare<F, G>(
    left: EvalResult,
    right: EvalResult,
    int_cmp: F,
    float_cmp: G,
) -> Result<EvalResult, ExpressionError>
where
    F: Fn(i64, i64) -> bool,
    G: Fn(f64, f64) -> bool,
{
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Boolean(int_cmp(a, b))),
        (EvalResult::Float(a), EvalResult::Float(b)) => Ok(EvalResult::Boolean(float_cmp(a, b))),
        (EvalResult::Integer(a), EvalResult::Float(b)) => {
            Ok(EvalResult::Boolean(float_cmp(a as f64, b)))
        }
        (EvalResult::Float(a), EvalResult::Integer(b)) => {
            Ok(EvalResult::Boolean(float_cmp(a, b as f64)))
        }
        (l, r) => Err(ExpressionError::TypeError {
            operator: "comparison".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate logical AND operation.
pub(crate) fn eval_logical_and(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Boolean(a), EvalResult::Boolean(b)) => Ok(EvalResult::Boolean(a && b)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "and".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate logical OR operation.
pub(crate) fn eval_logical_or(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Boolean(a), EvalResult::Boolean(b)) => Ok(EvalResult::Boolean(a || b)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "or".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate bitwise AND operation.
pub(crate) fn eval_bitwise_and(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Integer(a & b)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "&".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate bitwise OR operation.
pub(crate) fn eval_bitwise_or(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Integer(a | b)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "|".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate bitwise XOR operation.
pub(crate) fn eval_bitwise_xor(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => Ok(EvalResult::Integer(a ^ b)),
        (l, r) => Err(ExpressionError::TypeError {
            operator: "^".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate left shift operation.
pub(crate) fn eval_shift_left(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => {
            if !(0..64).contains(&b) {
                Err(ExpressionError::TypeError {
                    operator: "<<".to_string(),
                    operand_type: format!("shift amount {} out of range 0..63", b),
                })
            } else {
                Ok(EvalResult::Integer(a << b))
            }
        }
        (l, r) => Err(ExpressionError::TypeError {
            operator: "<<".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}

/// Evaluate right shift operation.
pub(crate) fn eval_shift_right(
    left: EvalResult,
    right: EvalResult,
) -> Result<EvalResult, ExpressionError> {
    match (left, right) {
        (EvalResult::Integer(a), EvalResult::Integer(b)) => {
            if !(0..64).contains(&b) {
                Err(ExpressionError::TypeError {
                    operator: ">>".to_string(),
                    operand_type: format!("shift amount {} out of range 0..63", b),
                })
            } else {
                Ok(EvalResult::Integer(a >> b))
            }
        }
        (l, r) => Err(ExpressionError::TypeError {
            operator: ">>".to_string(),
            operand_type: format!("{:?} and {:?}", l, r),
        }),
    }
}
