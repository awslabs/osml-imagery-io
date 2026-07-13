//! Expression evaluation logic.

use super::context::{EvalContext, Node};
use super::ops::{
    eval_add, eval_bitwise_and, eval_bitwise_or, eval_bitwise_xor, eval_compare, eval_div,
    eval_logical_and, eval_logical_or, eval_mod, eval_mul, eval_shift_left, eval_shift_right,
    eval_sub, values_equal,
};
use super::parser::Parser;
use super::{BinaryOperator, EvalResult, Expression, Literal, SpecialVariable, UnaryOperator};
use crate::parser::error::ExpressionError;
use crate::parser::types::ParamDefinition;

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
            Expression::FieldRef(path) => {
                // Resolve a bare field name against the flat value scope. These
                // structures use a single flat value scope per structure (no
                // lexically-nested types, no shadowing beyond the local-wins
                // rule), so a nested expression's bare name like `LEN` or `NPAR`
                // resolves to the enclosing field of the same name. See
                // `StructureWriter::build_eval_context` and `StructureAccessor`
                // for how that scope is seeded with inherited (enclosing) field
                // values. Only a scalar leaf is a valid bare reference; a bare
                // intermediary node (const map, array, struct) yields
                // `UnknownField` since it is not a final value on its own.
                context
                    .get_scalar(path)
                    .cloned()
                    .ok_or_else(|| ExpressionError::UnknownField {
                        field: path.clone(),
                    })
            }
            Expression::SpecialVar(var) => match var {
                SpecialVariable::Index => context
                    .index
                    .map(|i| EvalResult::Integer(i as i64))
                    .ok_or_else(|| ExpressionError::UnknownField {
                        field: "_index".to_string(),
                    }),
            },
            Expression::BinaryOp { left, op, right } => {
                let left_val = self.evaluate(left, context)?;
                // Short-circuit: false and <anything> = false, true or <anything> = true
                match (op, &left_val) {
                    (BinaryOperator::And, EvalResult::Boolean(false)) => {
                        return Ok(EvalResult::Boolean(false));
                    }
                    (BinaryOperator::Or, EvalResult::Boolean(true)) => {
                        return Ok(EvalResult::Boolean(true));
                    }
                    _ => {}
                }
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
            // Subscript navigation. `base[index]` navigates a context node and
            // yields the *scalar* final the surrounding expression consumes:
            //   - a const `Map` does a string-keyed lookup → mapped scalar;
            //   - an `Array` does numeric indexing → the element's scalar leaf,
            //     or (for `A[i].F`) the parser has already lowered the `.F` into
            //     an outer string-keyed `Index` whose base is this `A[i]`, so the
            //     base here resolves to the element `Struct` and this arm reads
            //     member `F` off it;
            //   - a `Struct` does a string-keyed member read → member scalar.
            // Chained navigation (array element → struct member) is resolved by
            // `navigate`, which walks the container chain and returns the node to
            // index into here.
            Expression::Index { base, index } => {
                let node = self.navigate(base, context)?;
                match node {
                    Node::Map(table) => {
                        let key = self.evaluate(index, context)?.expect_string()?;
                        table
                            .get(&key)
                            .cloned()
                            .ok_or_else(|| ExpressionError::UnknownKey {
                                map: Self::base_name(base),
                                key,
                            })
                    }
                    Node::Array(elems) => {
                        let raw = self.evaluate(index, context)?.expect_integer()?;
                        let elem = Self::array_element(elems, raw, base)?;
                        // A bare `A[i]` used as a final must land on a scalar; if
                        // the element is a struct the caller wrote `A[i]` without
                        // a following `.member`, which is an intermediary at the
                        // final boundary.
                        elem.as_scalar().cloned().ok_or_else(|| ExpressionError::TypeError {
                            operator: "[]".to_string(),
                            operand_type: format!("{:?}", elem),
                        })
                    }
                    Node::Struct(fields) => {
                        // Member deref after a subscript (`A[i].F`) lowers to a
                        // string-keyed index into the element struct.
                        let key = self.evaluate(index, context)?.expect_string()?;
                        fields
                            .get(&key)
                            .and_then(Node::as_scalar)
                            .cloned()
                            .ok_or(ExpressionError::UnknownField { field: key })
                    }
                    Node::Scalar(_) => Err(ExpressionError::TypeError {
                        operator: "[]".to_string(),
                        operand_type: format!("{:?}", node),
                    }),
                }
            }
        }
    }

    /// Select an array element by a (possibly negative) index, erroring when it
    /// falls outside the array. Names the array for the error via `base`.
    fn array_element<'c>(
        elems: &'c [Node],
        raw: i64,
        base: &Expression,
    ) -> Result<&'c Node, ExpressionError> {
        let idx = usize::try_from(raw).ok().filter(|&i| i < elems.len());
        match idx {
            Some(i) => Ok(&elems[i]),
            None => Err(ExpressionError::IndexOutOfRange {
                array: Self::base_name(base),
                index: raw,
                len: elems.len(),
            }),
        }
    }

    /// Bind a parameterized type reference's arguments to the target type's
    /// parameters, evaluated in the enclosing (parent) context.
    ///
    /// Each argument expression in `args` is evaluated against `parent_ctx` and
    /// bound by position to `params[i].id`, producing the name→scalar map the
    /// caller seeds into the child scope (via the inherited-snapshot mechanism)
    /// so `IMAGE_RECORDS[image_index].NCOLCB`-style references resolve inside the
    /// nested type. Arity is validated at load
    /// ([`crate::parser::definition::DefinitionLoader::validate_type_references`]),
    /// so a defensive length check here only guards against a hand-built
    /// definition; a mismatch binds the positions that line up and ignores the
    /// rest rather than erroring at eval time.
    pub fn bind_type_params(
        &self,
        params: &[ParamDefinition],
        args: &[Expression],
        parent_ctx: &EvalContext,
    ) -> Result<Vec<(String, EvalResult)>, ExpressionError> {
        let mut bound = Vec::with_capacity(params.len().min(args.len()));
        for (param, arg) in params.iter().zip(args.iter()) {
            let value = self.evaluate(arg, parent_ctx)?;
            bound.push((param.id.clone(), value));
        }
        Ok(bound)
    }

    /// Resolve the base of a subscript to the context [`Node`] it names.
    ///
    /// The base is either a bare reference to a named node — a const map, an
    /// array, or a struct — resolved against the context tree, or itself a
    /// subscript (`A[i]` as the base of `A[i].F`). In the chained case this
    /// recurses: it resolves the inner container and indexes one level into it,
    /// returning the sub-`Node` (typically the element `Struct` an outer
    /// `.member` reads from). Indexing a `Map` yields a scalar, not a node, so a
    /// `Map` can only be the *final* container in a chain — using one as an
    /// intermediate base is a type error.
    fn navigate<'c>(
        &self,
        base: &Expression,
        context: &'c EvalContext,
    ) -> Result<&'c Node, ExpressionError> {
        match base {
            Expression::FieldRef(name) => {
                context
                    .get(name)
                    .ok_or_else(|| ExpressionError::UnknownField {
                        field: name.clone(),
                    })
            }
            Expression::Index {
                base: inner_base,
                index,
            } => {
                let container = self.navigate(inner_base, context)?;
                match container {
                    Node::Array(elems) => {
                        let raw = self.evaluate(index, context)?.expect_integer()?;
                        Self::array_element(elems, raw, inner_base)
                    }
                    Node::Struct(fields) => {
                        let key = self.evaluate(index, context)?.expect_string()?;
                        fields.get(&key).ok_or(ExpressionError::UnknownField {
                            field: key,
                        })
                    }
                    other => Err(ExpressionError::TypeError {
                        operator: "[]".to_string(),
                        operand_type: format!("{:?}", other),
                    }),
                }
            }
            _ => Err(ExpressionError::TypeError {
                operator: "[]".to_string(),
                operand_type: "non-navigable base expression".to_string(),
            }),
        }
    }

    /// Best-effort name of a subscript base, for error messages.
    fn base_name(base: &Expression) -> String {
        match base {
            Expression::FieldRef(name) => name.clone(),
            _ => "<expr>".to_string(),
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
            // Bitwise operators
            BinaryOperator::BitwiseAnd => eval_bitwise_and(left, right),
            BinaryOperator::BitwiseOr => eval_bitwise_or(left, right),
            BinaryOperator::BitwiseXor => eval_bitwise_xor(left, right),
            BinaryOperator::ShiftLeft => eval_shift_left(left, right),
            BinaryOperator::ShiftRight => eval_shift_right(left, right),
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
            UnaryOperator::BitwiseNot => match val {
                EvalResult::Integer(n) => Ok(EvalResult::Integer(!n)),
                v => Err(ExpressionError::TypeError {
                    operator: "~".to_string(),
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
                EvalResult::String(s) => {
                    s.trim()
                        .parse::<i64>()
                        .map(EvalResult::Integer)
                        .map_err(|_| ExpressionError::TypeError {
                            operator: "to_i".to_string(),
                            operand_type: format!("String({})", s),
                        })
                }
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
            "strip" => match val {
                EvalResult::String(s) => Ok(EvalResult::String(s.trim().to_string())),
                v => Err(ExpressionError::TypeError {
                    operator: "strip".to_string(),
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
