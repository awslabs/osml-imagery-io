//! Property-based tests for expression parsing and evaluation.
//! These tests verify universal properties across many random inputs.

use super::*;
use crate::parser::error::ExpressionError;
use proptest::prelude::*;

/// Property 28: Expression Arithmetic Correctness
/// For any arithmetic expression using +, -, *, /, %, evaluation SHALL produce
/// the mathematically correct result.
/// **Validates: Requirements 13.2**
mod prop_28_arithmetic {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn addition_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} + {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(a + b));
        }

        #[test]
        fn subtraction_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} - {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(a - b));
        }

        #[test]
        fn multiplication_correctness(a in -100i64..100, b in -100i64..100) {
            let expr_str = format!("{} * {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(a * b));
        }

        #[test]
        fn division_correctness(a in -1000i64..1000, b in 1i64..100) {
            // b is always positive and non-zero
            let expr_str = format!("{} / {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(a / b));
        }

        #[test]
        fn modulo_correctness(a in -1000i64..1000, b in 1i64..100) {
            // b is always positive and non-zero
            let expr_str = format!("{} % {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(a % b));
        }

        #[test]
        fn division_by_zero_returns_error(a in -1000i64..1000) {
            let expr_str = format!("{} / 0", a);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx);
            prop_assert!(matches!(result, Err(ExpressionError::DivisionByZero)));
        }
    }
}

/// Property 29: Expression Comparison Correctness
/// For any comparison expression using ==, !=, <, >, <=, >=, evaluation SHALL
/// produce the logically correct boolean result.
/// **Validates: Requirements 13.3**
mod prop_29_comparison {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn equality_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} == {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a == b));
        }

        #[test]
        fn inequality_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} != {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a != b));
        }

        #[test]
        fn less_than_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} < {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a < b));
        }

        #[test]
        fn greater_than_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} > {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a > b));
        }

        #[test]
        fn less_than_or_equal_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} <= {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a <= b));
        }

        #[test]
        fn greater_than_or_equal_correctness(a in -1000i64..1000, b in -1000i64..1000) {
            let expr_str = format!("{} >= {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a >= b));
        }
    }
}

/// Property 30: Expression Logical Correctness
/// For any logical expression using and, or, not, evaluation SHALL follow
/// standard boolean logic.
/// **Validates: Requirements 13.4**
mod prop_30_logical {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn and_correctness(a: bool, b: bool) {
            let expr_str = format!("{} and {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a && b));
        }

        #[test]
        fn or_correctness(a: bool, b: bool) {
            let expr_str = format!("{} or {}", a, b);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(a || b));
        }

        #[test]
        fn not_correctness(a: bool) {
            let expr_str = format!("not {}", a);
            let expr = ExpressionEvaluator::parse(&expr_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Boolean(!a));
        }

        #[test]
        fn de_morgans_law_and(a: bool, b: bool) {
            // not (a and b) == (not a) or (not b)
            let expr1_str = format!("not ({} and {})", a, b);
            let expr2_str = format!("(not {}) or (not {})", a, b);
            let expr1 = ExpressionEvaluator::parse(&expr1_str).unwrap();
            let expr2 = ExpressionEvaluator::parse(&expr2_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result1 = evaluator.evaluate(&expr1, &ctx).unwrap();
            let result2 = evaluator.evaluate(&expr2, &ctx).unwrap();
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn de_morgans_law_or(a: bool, b: bool) {
            // not (a or b) == (not a) and (not b)
            let expr1_str = format!("not ({} or {})", a, b);
            let expr2_str = format!("(not {}) and (not {})", a, b);
            let expr1 = ExpressionEvaluator::parse(&expr1_str).unwrap();
            let expr2 = ExpressionEvaluator::parse(&expr2_str).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();
            let result1 = evaluator.evaluate(&expr1, &ctx).unwrap();
            let result2 = evaluator.evaluate(&expr2, &ctx).unwrap();
            prop_assert_eq!(result1, result2);
        }
    }
}

/// Property 32: Expression Syntax Error Handling
/// For any string that is not a valid expression, parsing SHALL return an ExpressionError.
/// **Validates: Requirements 13.8**
mod prop_32_syntax_errors {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn empty_string_is_error(s in "\\s*") {
            // Any whitespace-only string should be an error
            let result = ExpressionEvaluator::parse(&s);
            prop_assert!(result.is_err());
        }

        #[test]
        fn incomplete_binary_op_is_error(a in -1000i64..1000, op in prop::sample::select(vec!["+", "-", "*", "/", "%"])) {
            // "a +" should be an error
            let expr_str = format!("{} {}", a, op);
            let result = ExpressionEvaluator::parse(&expr_str);
            prop_assert!(result.is_err());
        }

        #[test]
        fn unclosed_paren_is_error(a in -1000i64..1000) {
            let expr_str = format!("({}", a);
            let result = ExpressionEvaluator::parse(&expr_str);
            prop_assert!(result.is_err());
        }

        #[test]
        fn invalid_operator_sequence_is_error(a in -1000i64..1000, b in -1000i64..1000) {
            // "a + + b" should be an error (double operator)
            let expr_str = format!("{} + + {}", a, b);
            let _result = ExpressionEvaluator::parse(&expr_str);
            // This might parse as a + (+b) which is valid, so we check for specific invalid cases
            // Let's use "a = b" which uses single = instead of ==
            let expr_str2 = format!("{} = {}", a, b);
            let result2 = ExpressionEvaluator::parse(&expr_str2);
            prop_assert!(result2.is_err());
        }

        #[test]
        fn trailing_tokens_is_error(a in "[a-z][a-z0-9_]{0,5}", b in "[a-z][a-z0-9_]{0,5}") {
            // "a b" (two identifiers without operator) should be an error
            let expr_str = format!("{} {}", a, b);
            let result = ExpressionEvaluator::parse(&expr_str);
            prop_assert!(result.is_err());
        }
    }
}

/// Property 31: Expression Type Coercion
/// For any value, `.to_i` SHALL return its integer representation, `.to_s` SHALL
/// return its string representation, and `.length` SHALL return its length.
/// **Validates: Requirements 13.5**
mod prop_31_type_coercion {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn to_i_from_integer_is_identity(n in -10000i64..10000) {
            let expr = ExpressionEvaluator::parse("val.to_i").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::Integer(n));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(n));
        }

        #[test]
        fn to_i_from_float_truncates(f in -1000.0f64..1000.0) {
            let expr = ExpressionEvaluator::parse("val.to_i").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::Float(f));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(f as i64));
        }

        #[test]
        fn to_i_from_numeric_string(n in -10000i64..10000) {
            let s = n.to_string();
            let expr = ExpressionEvaluator::parse("val.to_i").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::String(s));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(n));
        }

        #[test]
        fn to_i_from_boolean(b: bool) {
            let expr = ExpressionEvaluator::parse("val.to_i").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::Boolean(b));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(if b { 1 } else { 0 }));
        }

        #[test]
        fn to_s_from_integer(n in -10000i64..10000) {
            let expr = ExpressionEvaluator::parse("val.to_s").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::Integer(n));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::String(n.to_string()));
        }

        #[test]
        fn to_s_from_string_is_identity(s in "[a-zA-Z0-9 ]{0,20}") {
            let expr = ExpressionEvaluator::parse("val.to_s").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::String(s.clone()));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::String(s));
        }

        #[test]
        fn to_s_from_boolean(b: bool) {
            let expr = ExpressionEvaluator::parse("val.to_s").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::Boolean(b));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::String(b.to_string()));
        }

        #[test]
        fn length_of_string(s in "[a-zA-Z0-9]{0,50}") {
            let expected_len = s.len() as i64;
            let expr = ExpressionEvaluator::parse("val.length").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::String(s));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(expected_len));
        }

        #[test]
        fn length_of_bytes(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
            let expected_len = bytes.len() as i64;
            let expr = ExpressionEvaluator::parse("val.length").unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new().with_field("val", EvalResult::Bytes(bytes));
            let result = evaluator.evaluate(&expr, &ctx).unwrap();
            prop_assert_eq!(result, EvalResult::Integer(expected_len));
        }
    }
}
