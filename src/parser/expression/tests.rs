//! Unit tests for expression parsing and evaluation.

use super::*;

#[test]
fn parse_integer_literal() {
    let expr = ExpressionEvaluator::parse("42").unwrap();
    assert_eq!(expr, Expression::Literal(Literal::Integer(42)));
}

#[test]
fn parse_negative_integer() {
    let expr = ExpressionEvaluator::parse("-42").unwrap();
    assert_eq!(
        expr,
        Expression::UnaryOp {
            op: UnaryOperator::Neg,
            operand: Box::new(Expression::Literal(Literal::Integer(42))),
        }
    );
}

#[test]
fn parse_float_literal() {
    let expr = ExpressionEvaluator::parse("2.718").unwrap();
    assert_eq!(expr, Expression::Literal(Literal::Float(2.718)));
}

#[test]
fn parse_string_literal() {
    let expr = ExpressionEvaluator::parse("\"hello\"").unwrap();
    assert_eq!(
        expr,
        Expression::Literal(Literal::String("hello".to_string()))
    );
}

#[test]
fn parse_boolean_true() {
    let expr = ExpressionEvaluator::parse("true").unwrap();
    assert_eq!(expr, Expression::Literal(Literal::Boolean(true)));
}

#[test]
fn parse_boolean_false() {
    let expr = ExpressionEvaluator::parse("false").unwrap();
    assert_eq!(expr, Expression::Literal(Literal::Boolean(false)));
}

#[test]
fn parse_field_reference() {
    let expr = ExpressionEvaluator::parse("field_name").unwrap();
    assert_eq!(expr, Expression::FieldRef("field_name".to_string()));
}

#[test]
fn parse_nested_field_reference() {
    let expr = ExpressionEvaluator::parse("parent.child").unwrap();
    assert_eq!(expr, Expression::FieldRef("parent.child".to_string()));
}

#[test]
fn parse_deeply_nested_field() {
    let expr = ExpressionEvaluator::parse("a.b.c.d").unwrap();
    assert_eq!(expr, Expression::FieldRef("a.b.c.d".to_string()));
}

#[test]
fn parse_special_var_index() {
    let expr = ExpressionEvaluator::parse("_index").unwrap();
    assert_eq!(expr, Expression::SpecialVar(SpecialVariable::Index));
}

#[test]
fn parse_special_var_root() {
    let expr = ExpressionEvaluator::parse("_root").unwrap();
    assert_eq!(expr, Expression::SpecialVar(SpecialVariable::Root));
}

#[test]
fn parse_special_var_parent() {
    let expr = ExpressionEvaluator::parse("_parent").unwrap();
    assert_eq!(expr, Expression::SpecialVar(SpecialVariable::Parent));
}

#[test]
fn parse_special_var_io() {
    let expr = ExpressionEvaluator::parse("_io").unwrap();
    assert_eq!(expr, Expression::SpecialVar(SpecialVariable::Io));
}

#[test]
fn parse_addition() {
    let expr = ExpressionEvaluator::parse("1 + 2").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(1))),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Literal(Literal::Integer(2))),
        }
    );
}

#[test]
fn parse_subtraction() {
    let expr = ExpressionEvaluator::parse("5 - 3").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(5))),
            op: BinaryOperator::Sub,
            right: Box::new(Expression::Literal(Literal::Integer(3))),
        }
    );
}

#[test]
fn parse_multiplication() {
    let expr = ExpressionEvaluator::parse("2 * 3").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(2))),
            op: BinaryOperator::Mul,
            right: Box::new(Expression::Literal(Literal::Integer(3))),
        }
    );
}

#[test]
fn parse_division() {
    let expr = ExpressionEvaluator::parse("10 / 2").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(10))),
            op: BinaryOperator::Div,
            right: Box::new(Expression::Literal(Literal::Integer(2))),
        }
    );
}

#[test]
fn parse_modulo() {
    let expr = ExpressionEvaluator::parse("10 % 3").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(10))),
            op: BinaryOperator::Mod,
            right: Box::new(Expression::Literal(Literal::Integer(3))),
        }
    );
}

#[test]
fn parse_comparison_eq() {
    let expr = ExpressionEvaluator::parse("a == b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Eq,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_comparison_ne() {
    let expr = ExpressionEvaluator::parse("a != b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Ne,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_comparison_lt() {
    let expr = ExpressionEvaluator::parse("a < b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Lt,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_comparison_gt() {
    let expr = ExpressionEvaluator::parse("a > b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Gt,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_comparison_le() {
    let expr = ExpressionEvaluator::parse("a <= b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Le,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_comparison_ge() {
    let expr = ExpressionEvaluator::parse("a >= b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Ge,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_logical_and() {
    let expr = ExpressionEvaluator::parse("a and b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::And,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_logical_or() {
    let expr = ExpressionEvaluator::parse("a or b").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::FieldRef("a".to_string())),
            op: BinaryOperator::Or,
            right: Box::new(Expression::FieldRef("b".to_string())),
        }
    );
}

#[test]
fn parse_logical_not() {
    let expr = ExpressionEvaluator::parse("not a").unwrap();
    assert_eq!(
        expr,
        Expression::UnaryOp {
            op: UnaryOperator::Not,
            operand: Box::new(Expression::FieldRef("a".to_string())),
        }
    );
}

#[test]
fn parse_method_to_i() {
    let expr = ExpressionEvaluator::parse("field.to_i").unwrap();
    assert_eq!(
        expr,
        Expression::MethodCall {
            target: Box::new(Expression::FieldRef("field".to_string())),
            method: "to_i".to_string(),
        }
    );
}

#[test]
fn parse_method_to_s() {
    let expr = ExpressionEvaluator::parse("field.to_s").unwrap();
    assert_eq!(
        expr,
        Expression::MethodCall {
            target: Box::new(Expression::FieldRef("field".to_string())),
            method: "to_s".to_string(),
        }
    );
}

#[test]
fn parse_method_length() {
    let expr = ExpressionEvaluator::parse("field.length").unwrap();
    assert_eq!(
        expr,
        Expression::MethodCall {
            target: Box::new(Expression::FieldRef("field".to_string())),
            method: "length".to_string(),
        }
    );
}

#[test]
fn parse_parentheses() {
    let expr = ExpressionEvaluator::parse("(1 + 2) * 3").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Literal(Literal::Integer(1))),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Literal(Literal::Integer(2))),
            }),
            op: BinaryOperator::Mul,
            right: Box::new(Expression::Literal(Literal::Integer(3))),
        }
    );
}

#[test]
fn parse_operator_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let expr = ExpressionEvaluator::parse("1 + 2 * 3").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(1))),
            op: BinaryOperator::Add,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Literal(Literal::Integer(2))),
                op: BinaryOperator::Mul,
                right: Box::new(Expression::Literal(Literal::Integer(3))),
            }),
        }
    );
}

#[test]
fn parse_complex_expression() {
    let expr = ExpressionEvaluator::parse("numi.to_i > 0 and version == \"02.10\"").unwrap();
    assert_eq!(
        expr,
        Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::MethodCall {
                    target: Box::new(Expression::FieldRef("numi".to_string())),
                    method: "to_i".to_string(),
                }),
                op: BinaryOperator::Gt,
                right: Box::new(Expression::Literal(Literal::Integer(0))),
            }),
            op: BinaryOperator::And,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::FieldRef("version".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expression::Literal(Literal::String("02.10".to_string()))),
            }),
        }
    );
}

#[test]
fn parse_empty_expression_error() {
    let result = ExpressionEvaluator::parse("");
    assert!(result.is_err());
}

#[test]
fn parse_invalid_expression_error() {
    let result = ExpressionEvaluator::parse("1 +");
    assert!(result.is_err());
}

#[test]
fn parse_unexpected_token_error() {
    let result = ExpressionEvaluator::parse("1 2");
    assert!(result.is_err());
}

// Evaluation tests
#[test]
fn eval_integer_literal() {
    let expr = ExpressionEvaluator::parse("42").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(42));
}

#[test]
fn eval_addition() {
    let expr = ExpressionEvaluator::parse("1 + 2").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(3));
}

#[test]
fn eval_subtraction() {
    let expr = ExpressionEvaluator::parse("5 - 3").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(2));
}

#[test]
fn eval_multiplication() {
    let expr = ExpressionEvaluator::parse("4 * 5").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(20));
}

#[test]
fn eval_division() {
    let expr = ExpressionEvaluator::parse("10 / 2").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(5));
}

#[test]
fn eval_modulo() {
    let expr = ExpressionEvaluator::parse("10 % 3").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(1));
}

#[test]
fn eval_division_by_zero() {
    use crate::parser::error::ExpressionError;
    let expr = ExpressionEvaluator::parse("10 / 0").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx);
    assert!(matches!(result, Err(ExpressionError::DivisionByZero)));
}

#[test]
fn eval_comparison_eq_true() {
    let expr = ExpressionEvaluator::parse("5 == 5").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(true));
}

#[test]
fn eval_comparison_eq_false() {
    let expr = ExpressionEvaluator::parse("5 == 6").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(false));
}

#[test]
fn eval_comparison_lt() {
    let expr = ExpressionEvaluator::parse("3 < 5").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(true));
}

#[test]
fn eval_logical_and_true() {
    let expr = ExpressionEvaluator::parse("true and true").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(true));
}

#[test]
fn eval_logical_and_false() {
    let expr = ExpressionEvaluator::parse("true and false").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(false));
}

#[test]
fn eval_logical_or_true() {
    let expr = ExpressionEvaluator::parse("false or true").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(true));
}

#[test]
fn eval_logical_not() {
    let expr = ExpressionEvaluator::parse("not false").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(true));
}

#[test]
fn eval_negation() {
    let expr = ExpressionEvaluator::parse("-42").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(-42));
}

#[test]
fn eval_field_reference() {
    let expr = ExpressionEvaluator::parse("count").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("count", EvalResult::Integer(10));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(10));
}

#[test]
fn eval_field_reference_unknown() {
    use crate::parser::error::ExpressionError;
    let expr = ExpressionEvaluator::parse("unknown").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx);
    assert!(matches!(result, Err(ExpressionError::UnknownField { .. })));
}

#[test]
fn eval_index_variable() {
    let expr = ExpressionEvaluator::parse("_index").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_index(5);
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(5));
}

#[test]
fn eval_method_to_i_from_string() {
    let expr = ExpressionEvaluator::parse("num.to_i").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("num", EvalResult::String("42".to_string()));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(42));
}

#[test]
fn eval_method_to_s_from_int() {
    let expr = ExpressionEvaluator::parse("num.to_s").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("num", EvalResult::Integer(42));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::String("42".to_string()));
}

#[test]
fn eval_method_length() {
    let expr = ExpressionEvaluator::parse("text.length").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("text", EvalResult::String("hello".to_string()));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(5));
}

#[test]
fn eval_complex_expression() {
    // (count + 1) * 2
    let expr = ExpressionEvaluator::parse("(count + 1) * 2").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("count", EvalResult::Integer(4));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(10));
}

#[test]
fn eval_mixed_types_float_int() {
    let expr = ExpressionEvaluator::parse("3.5 + 2").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Float(5.5));
}

#[test]
fn eval_string_concatenation() {
    let expr = ExpressionEvaluator::parse("\"hello\" + \" world\"").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::String("hello world".to_string()));
}

#[test]
fn parse_method_strip() {
    let expr = ExpressionEvaluator::parse("field.strip").unwrap();
    assert_eq!(
        expr,
        Expression::MethodCall {
            target: Box::new(Expression::FieldRef("field".to_string())),
            method: "strip".to_string(),
        }
    );
}

#[test]
fn parse_chained_method_to_s_strip() {
    let expr = ExpressionEvaluator::parse("METOC_SOURCE.to_s.strip").unwrap();
    assert_eq!(
        expr,
        Expression::MethodCall {
            target: Box::new(Expression::MethodCall {
                target: Box::new(Expression::FieldRef("METOC_SOURCE".to_string())),
                method: "to_s".to_string(),
            }),
            method: "strip".to_string(),
        }
    );
}

#[test]
fn eval_method_strip() {
    let expr = ExpressionEvaluator::parse("field.strip").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("field", EvalResult::String("  hello  ".to_string()));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::String("hello".to_string()));
}

#[test]
fn eval_chained_to_s_strip_eq() {
    // Mirrors: METOC_SOURCE.to_s.strip == "NONTRADITIONAL"
    let expr = ExpressionEvaluator::parse("METOC_SOURCE.to_s.strip == \"NONTRADITIONAL\"").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field(
        "METOC_SOURCE",
        EvalResult::String("NONTRADITIONAL      ".to_string()),
    );
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(true));
}

#[test]
fn eval_chained_to_s_strip_ne_empty() {
    // Mirrors: LOCATION_SHAPE.to_s.strip != ""
    let expr = ExpressionEvaluator::parse("LOCATION_SHAPE.to_s.strip != \"\"").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx =
        EvalContext::new().with_field("LOCATION_SHAPE", EvalResult::String("   ".to_string()));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Boolean(false));
}

#[test]
fn eval_strip_on_non_string_is_error() {
    let expr = ExpressionEvaluator::parse("num.strip").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("num", EvalResult::Integer(42));
    let result = evaluator.evaluate(&expr, &ctx);
    assert!(result.is_err());
}
