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
#[allow(clippy::approx_constant)]
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
fn parse_removed_navigator_root_is_plain_field_ref() {
    // `_root`/`_parent`/`_io` are no longer special variables; a bare token
    // parses as an ordinary field reference.
    let expr = ExpressionEvaluator::parse("_root").unwrap();
    assert_eq!(expr, Expression::FieldRef("_root".to_string()));
}

#[test]
fn parse_removed_navigator_dotted_is_plain_field_ref() {
    // A dotted navigator like `_parent.NPAR` lowers to a single dotted
    // FieldRef path — no special-var handling remains.
    let expr = ExpressionEvaluator::parse("_parent.NPAR").unwrap();
    assert_eq!(expr, Expression::FieldRef("_parent.NPAR".to_string()));
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

// Subscript (`[]`) parsing tests — Phase 1 (parse-only).

#[test]
fn parse_map_subscript_string_key() {
    // MAP[KEY] → Index { base: FieldRef(MAP), index: FieldRef(KEY) }
    let expr = ExpressionEvaluator::parse("sensrb_value_widths[TIME_STAMP_TYPE]").unwrap();
    assert_eq!(
        expr,
        Expression::Index {
            base: Box::new(Expression::FieldRef("sensrb_value_widths".to_string())),
            index: Box::new(Expression::FieldRef("TIME_STAMP_TYPE".to_string())),
        }
    );
}

#[test]
fn parse_array_subscript_index() {
    // A[i] → Index { base: FieldRef(A), index: FieldRef(i) }
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[i]").unwrap();
    assert_eq!(
        expr,
        Expression::Index {
            base: Box::new(Expression::FieldRef("IMAGE_RECORDS".to_string())),
            index: Box::new(Expression::FieldRef("i".to_string())),
        }
    );
}

#[test]
fn parse_array_subscript_member_deref() {
    // A[i].F → member deref after subscript lowers to a string-keyed Index.
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[i].NCOLCB").unwrap();
    assert_eq!(
        expr,
        Expression::Index {
            base: Box::new(Expression::Index {
                base: Box::new(Expression::FieldRef("IMAGE_RECORDS".to_string())),
                index: Box::new(Expression::FieldRef("i".to_string())),
            }),
            index: Box::new(Expression::Literal(Literal::String("NCOLCB".to_string()))),
        }
    );
}

#[test]
fn parse_array_subscript_index_expression_member() {
    // A[_index - 1].F → arbitrary index expression + member deref.
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[_index - 1].NCOLCB").unwrap();
    assert_eq!(
        expr,
        Expression::Index {
            base: Box::new(Expression::Index {
                base: Box::new(Expression::FieldRef("IMAGE_RECORDS".to_string())),
                index: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::SpecialVar(SpecialVariable::Index)),
                    op: BinaryOperator::Sub,
                    right: Box::new(Expression::Literal(Literal::Integer(1))),
                }),
            }),
            index: Box::new(Expression::Literal(Literal::String("NCOLCB".to_string()))),
        }
    );
}

#[test]
fn parse_subscript_uses_index_special_var() {
    // A[_index] → the index is a normal sub-expression, so `_index` resolves
    // to the special variable.
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[_index]").unwrap();
    assert_eq!(
        expr,
        Expression::Index {
            base: Box::new(Expression::FieldRef("IMAGE_RECORDS".to_string())),
            index: Box::new(Expression::SpecialVar(SpecialVariable::Index)),
        }
    );
}

#[test]
fn parse_subscript_left_associative_chain() {
    // A[i][j] → left-associative: (A[i])[j].
    let expr = ExpressionEvaluator::parse("A[i][j]").unwrap();
    assert_eq!(
        expr,
        Expression::Index {
            base: Box::new(Expression::Index {
                base: Box::new(Expression::FieldRef("A".to_string())),
                index: Box::new(Expression::FieldRef("i".to_string())),
            }),
            index: Box::new(Expression::FieldRef("j".to_string())),
        }
    );
}

#[test]
fn parse_subscript_composes_with_method_call() {
    // MAP[KEY].to_i → method call applies to the subscript result.
    let expr = ExpressionEvaluator::parse("MAP[KEY].to_i").unwrap();
    assert_eq!(
        expr,
        Expression::MethodCall {
            target: Box::new(Expression::Index {
                base: Box::new(Expression::FieldRef("MAP".to_string())),
                index: Box::new(Expression::FieldRef("KEY".to_string())),
            }),
            method: "to_i".to_string(),
        }
    );
}

#[test]
fn parse_subscript_unterminated_error() {
    // Missing closing bracket is a syntax error.
    assert!(ExpressionEvaluator::parse("MAP[KEY").is_err());
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

#[test]
fn eval_bare_name_resolves_against_flat_scope() {
    // Nested expressions reference enclosing fields by bare name; the flat
    // value scope (seeded with inherited enclosing scalars) resolves them.
    let expr = ExpressionEvaluator::parse("LEN.to_i").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("LEN", EvalResult::String("3".to_string()));
    let result = evaluator.evaluate(&expr, &ctx).unwrap();
    assert_eq!(result, EvalResult::Integer(3));
}

#[test]
fn eval_removed_navigator_path_does_not_resolve_to_bare_field() {
    // With navigator string-lowering gone, `_root.LEN` is a literal dotted
    // FieldRef that no longer falls back to the bare `LEN`; it errors.
    let expr = ExpressionEvaluator::parse("_root.LEN.to_i").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new().with_field("LEN", EvalResult::String("3".to_string()));
    assert!(evaluator.evaluate(&expr, &ctx).is_err());
}

#[test]
fn eval_unknown_field_errors() {
    let expr = ExpressionEvaluator::parse("MISSING").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let ctx = EvalContext::new();
    assert!(evaluator.evaluate(&expr, &ctx).is_err());
}

#[test]
fn eval_bare_intermediary_node_is_unknown_field() {
    // A bare reference to an intermediary node (a const map here) is not a valid
    // final value: `get_scalar` skips it, so the FieldRef arm errors just as it
    // would for an absent name. Only a scalar leaf resolves.
    let expr = ExpressionEvaluator::parse("widths").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let mut table = std::collections::HashMap::new();
    table.insert("06a".to_string(), EvalResult::Integer(11));
    let mut ctx = EvalContext::new();
    ctx.insert_node("widths", Node::Map(table));
    assert!(matches!(
        evaluator.evaluate(&expr, &ctx),
        Err(crate::parser::error::ExpressionError::UnknownField { .. })
    ));
}

#[test]
fn eval_scalar_leaf_shadows_nothing_and_resolves() {
    // A scalar leaf inserted via the node API resolves through the same FieldRef
    // path the flat scope used before the node-tree refactor.
    let expr = ExpressionEvaluator::parse("N + 1").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let mut ctx = EvalContext::new();
    ctx.insert_scalar("N", EvalResult::Integer(41));
    assert_eq!(
        evaluator.evaluate(&expr, &ctx).unwrap(),
        EvalResult::Integer(42)
    );
}

// Subscript (`[]`) map-lookup evaluation tests — Phase 3.

/// A context holding a `sensrb_value_widths`-style const map plus a scalar code
/// field, the shape SENSRB uses (`sensrb_value_widths[TIME_STAMP_TYPE]`).
fn widths_context(code: &str) -> EvalContext {
    let mut table = std::collections::HashMap::new();
    table.insert("06a".to_string(), EvalResult::Integer(11));
    table.insert("06b".to_string(), EvalResult::Integer(12));
    let mut ctx = EvalContext::new();
    ctx.insert_node("sensrb_value_widths", Node::Map(table));
    ctx.insert_scalar("TIME_STAMP_TYPE", EvalResult::String(code.to_string()));
    ctx
}

#[test]
fn eval_map_lookup_resolves_to_mapped_scalar() {
    // MAP[KEY] resolves to the mapped scalar (the map-lookup arm).
    let expr = ExpressionEvaluator::parse("sensrb_value_widths[TIME_STAMP_TYPE]").unwrap();
    let evaluator = ExpressionEvaluator::new();
    assert_eq!(
        evaluator.evaluate(&expr, &widths_context("06b")).unwrap(),
        EvalResult::Integer(12)
    );
    assert_eq!(
        evaluator.evaluate(&expr, &widths_context("06a")).unwrap(),
        EvalResult::Integer(11)
    );
}

#[test]
fn eval_map_lookup_composes_with_method_call() {
    // The subscript result is a normal scalar, so `.to_i` etc. apply — the shape
    // `size:` / `repeat-expr:` expressions use.
    let expr = ExpressionEvaluator::parse("sensrb_value_widths[TIME_STAMP_TYPE].to_i").unwrap();
    let evaluator = ExpressionEvaluator::new();
    assert_eq!(
        evaluator.evaluate(&expr, &widths_context("06a")).unwrap(),
        EvalResult::Integer(11)
    );
}

#[test]
fn eval_map_lookup_unknown_key_errors_naming_map_and_key() {
    // Unknown-key is an error (not a silent fallback), and the error names both
    // the map and the missing key.
    let expr = ExpressionEvaluator::parse("sensrb_value_widths[TIME_STAMP_TYPE]").unwrap();
    let evaluator = ExpressionEvaluator::new();
    match evaluator.evaluate(&expr, &widths_context("99z")) {
        Err(crate::parser::error::ExpressionError::UnknownKey { map, key }) => {
            assert_eq!(map, "sensrb_value_widths");
            assert_eq!(key, "99z");
        }
        other => panic!("expected UnknownKey, got {:?}", other),
    }
}

#[test]
fn eval_map_lookup_missing_base_is_unknown_field() {
    // Subscripting a name that is not in the context errors as UnknownField.
    let expr = ExpressionEvaluator::parse("no_such_map[K]").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let mut ctx = EvalContext::new();
    ctx.insert_scalar("K", EvalResult::String("x".to_string()));
    assert!(matches!(
        evaluator.evaluate(&expr, &ctx),
        Err(crate::parser::error::ExpressionError::UnknownField { .. })
    ));
}

#[test]
fn eval_map_lookup_integer_key_coerces_to_string() {
    // A numeric key is rendered to its string form so integer-keyed maps work.
    let expr = ExpressionEvaluator::parse("m[k]").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let mut table = std::collections::HashMap::new();
    table.insert("7".to_string(), EvalResult::Integer(70));
    let mut ctx = EvalContext::new();
    ctx.insert_node("m", Node::Map(table));
    ctx.insert_scalar("k", EvalResult::Integer(7));
    assert_eq!(
        evaluator.evaluate(&expr, &ctx).unwrap(),
        EvalResult::Integer(70)
    );
}

#[test]
fn eval_subscript_on_scalar_is_type_error() {
    // Subscripting a scalar leaf (not a map/array) is a type error at `[]`.
    let expr = ExpressionEvaluator::parse("N[K]").unwrap();
    let evaluator = ExpressionEvaluator::new();
    let mut ctx = EvalContext::new();
    ctx.insert_scalar("N", EvalResult::Integer(3));
    ctx.insert_scalar("K", EvalResult::String("x".to_string()));
    assert!(matches!(
        evaluator.evaluate(&expr, &ctx),
        Err(crate::parser::error::ExpressionError::TypeError { .. })
    ));
}

// Array-subscript + member-deref evaluation tests — Phase 6.

/// Build a context holding an `IMAGE_RECORDS`-style array of structs, each with
/// an `NCOLCB` scalar member, plus a scalar `NROWCB` — the RSMDCB shape.
fn image_records_context() -> EvalContext {
    let record = |ncolcb: i64| {
        let mut m = std::collections::HashMap::new();
        m.insert("NCOLCB".to_string(), Node::Scalar(EvalResult::Integer(ncolcb)));
        Node::Struct(m)
    };
    let mut ctx = EvalContext::new();
    ctx.insert_node(
        "IMAGE_RECORDS",
        Node::Array(vec![record(3), record(5), record(7)]),
    );
    ctx.insert_scalar("NROWCB", EvalResult::Integer(4));
    ctx
}

#[test]
fn eval_array_subscript_member_deref_resolves_element_member() {
    // `ARRAY[i].FIELD` resolves to the indexed element's member scalar.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context();
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[1].NCOLCB").unwrap();
    assert_eq!(
        evaluator.evaluate(&expr, &ctx).unwrap(),
        EvalResult::Integer(5)
    );
}

#[test]
fn eval_array_subscript_with_index_expression_and_arithmetic() {
    // The RSMDCB inner-count shape: `NROWCB.to_i * IMAGE_RECORDS[_index].NCOLCB.to_i`
    // with `_index` bound to the outer image loop counter.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context().with_index(2);
    let expr = ExpressionEvaluator::parse(
        "NROWCB.to_i * IMAGE_RECORDS[_index].NCOLCB.to_i",
    )
    .unwrap();
    // NROWCB(4) * IMAGE_RECORDS[2].NCOLCB(7) = 28.
    assert_eq!(
        evaluator.evaluate(&expr, &ctx).unwrap(),
        EvalResult::Integer(28)
    );
}

#[test]
fn eval_array_subscript_index_minus_one() {
    // `A[_index - 1].F` — an arbitrary index expression selects an earlier element.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context().with_index(2);
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[_index - 1].NCOLCB").unwrap();
    // IMAGE_RECORDS[1].NCOLCB = 5.
    assert_eq!(
        evaluator.evaluate(&expr, &ctx).unwrap(),
        EvalResult::Integer(5)
    );
}

#[test]
fn eval_array_subscript_out_of_range_errors() {
    // Indexing past the end is an error (not a silent fallback), naming the array.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context();
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[9].NCOLCB").unwrap();
    match evaluator.evaluate(&expr, &ctx) {
        Err(crate::parser::error::ExpressionError::IndexOutOfRange { array, index, len }) => {
            assert_eq!(array, "IMAGE_RECORDS");
            assert_eq!(index, 9);
            assert_eq!(len, 3);
        }
        other => panic!("expected IndexOutOfRange, got {:?}", other),
    }
}

#[test]
fn eval_array_subscript_negative_index_errors() {
    // A negative index is out of range, not a Python-style wrap-around.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context();
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[0 - 1].NCOLCB").unwrap();
    assert!(matches!(
        evaluator.evaluate(&expr, &ctx),
        Err(crate::parser::error::ExpressionError::IndexOutOfRange { .. })
    ));
}

#[test]
fn eval_array_element_scalar_resolves_without_member() {
    // A `Node::Array` of scalar leaves indexes to the element scalar directly.
    let evaluator = ExpressionEvaluator::new();
    let mut ctx = EvalContext::new();
    ctx.insert_node(
        "XS",
        Node::Array(vec![
            Node::Scalar(EvalResult::Integer(10)),
            Node::Scalar(EvalResult::Integer(20)),
        ]),
    );
    let expr = ExpressionEvaluator::parse("XS[1]").unwrap();
    assert_eq!(
        evaluator.evaluate(&expr, &ctx).unwrap(),
        EvalResult::Integer(20)
    );
}

#[test]
fn eval_bare_array_element_struct_is_type_error() {
    // A bare `A[i]` landing on a struct element (no following `.member`) is an
    // intermediary at the final boundary — a type error, not a value.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context();
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[0]").unwrap();
    assert!(matches!(
        evaluator.evaluate(&expr, &ctx),
        Err(crate::parser::error::ExpressionError::TypeError { .. })
    ));
}

#[test]
fn eval_struct_member_deref_unknown_member_errors() {
    // Dereferencing a member the struct does not declare errors as UnknownField.
    let evaluator = ExpressionEvaluator::new();
    let ctx = image_records_context();
    let expr = ExpressionEvaluator::parse("IMAGE_RECORDS[0].NOPE").unwrap();
    assert!(matches!(
        evaluator.evaluate(&expr, &ctx),
        Err(crate::parser::error::ExpressionError::UnknownField { .. })
    ));
}

#[test]
fn bind_type_params_evaluates_args_in_parent_scope() {
    // Each argument is evaluated against the parent context and bound to the
    // target type's param by position. Here `W.to_i + 1` (=5) binds to `width`.
    use crate::parser::types::ParamDefinition;

    let params = vec![ParamDefinition::new("width", Some("s4".to_string()))];
    let args = vec![ExpressionEvaluator::parse("W.to_i + 1").unwrap()];
    let evaluator = ExpressionEvaluator::new();
    let mut parent = EvalContext::new();
    parent.insert_scalar("W", EvalResult::String("4".to_string()));

    let bound = evaluator.bind_type_params(&params, &args, &parent).unwrap();
    assert_eq!(bound, vec![("width".to_string(), EvalResult::Integer(5))]);
}

#[test]
fn bind_type_params_binds_index_special_var() {
    // `_index` in an argument resolves against the parent context's index, the
    // RSMDCB `crscov_block(_index)` shape.
    use crate::parser::types::ParamDefinition;

    let params = vec![ParamDefinition::new("image_index", Some("s4".to_string()))];
    let args = vec![ExpressionEvaluator::parse("_index").unwrap()];
    let evaluator = ExpressionEvaluator::new();
    let parent = EvalContext::new().with_index(2);

    let bound = evaluator.bind_type_params(&params, &args, &parent).unwrap();
    assert_eq!(
        bound,
        vec![("image_index".to_string(), EvalResult::Integer(2))]
    );
}

#[test]
fn bind_type_params_propagates_arg_eval_error() {
    // An argument referencing an unavailable value errors (total-or-error), not
    // a silently-dropped binding.
    use crate::parser::types::ParamDefinition;

    let params = vec![ParamDefinition::new("width", None)];
    let args = vec![ExpressionEvaluator::parse("MISSING.to_i").unwrap()];
    let evaluator = ExpressionEvaluator::new();
    let parent = EvalContext::new();

    assert!(evaluator.bind_type_params(&params, &args, &parent).is_err());
}
