//! Parser for expression strings using recursive descent.

use super::lexer::{Lexer, Token};
use super::{BinaryOperator, Expression, Literal, SpecialVariable, UnaryOperator};
use crate::parser::error::ExpressionError;

/// Method names recognized as postfix method calls rather than field access.
const METHODS: [&str; 4] = ["to_i", "to_s", "length", "strip"];

/// Parser for expression strings using recursive descent.
pub(crate) struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self, ExpressionError> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    pub fn current_token(&self) -> &Token {
        &self.current
    }

    fn advance(&mut self) -> Result<(), ExpressionError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn expect(&mut self, expected: Token) -> Result<(), ExpressionError> {
        if self.current == expected {
            self.advance()
        } else {
            Err(ExpressionError::SyntaxError {
                message: format!("Expected {:?}, found {:?}", expected, self.current),
            })
        }
    }

    /// Parse a complete expression.
    pub fn parse_expression(&mut self) -> Result<Expression, ExpressionError> {
        self.parse_or()
    }

    /// Parse logical OR: expr 'or' expr
    fn parse_or(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_and()?;
        while self.current == Token::Or {
            self.advance()?;
            let right = self.parse_and()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse logical AND: expr 'and' expr
    fn parse_and(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_comparison()?;
        while self.current == Token::And {
            self.advance()?;
            let right = self.parse_comparison()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse comparison: expr ('==' | '!=' | '<' | '>' | '<=' | '>=') expr
    fn parse_comparison(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_bitwise_or()?;
        loop {
            let op = match &self.current {
                Token::EqEq => BinaryOperator::Eq,
                Token::NotEq => BinaryOperator::Ne,
                Token::Lt => BinaryOperator::Lt,
                Token::Gt => BinaryOperator::Gt,
                Token::LtEq => BinaryOperator::Le,
                Token::GtEq => BinaryOperator::Ge,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_bitwise_or()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse bitwise OR: expr '|' expr
    fn parse_bitwise_or(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_bitwise_xor()?;
        while self.current == Token::Pipe {
            self.advance()?;
            let right = self.parse_bitwise_xor()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::BitwiseOr,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse bitwise XOR: expr '^' expr
    fn parse_bitwise_xor(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_bitwise_and()?;
        while self.current == Token::Caret {
            self.advance()?;
            let right = self.parse_bitwise_and()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::BitwiseXor,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse bitwise AND: expr '&' expr
    fn parse_bitwise_and(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_additive()?;
        while self.current == Token::Ampersand {
            self.advance()?;
            let right = self.parse_additive()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::BitwiseAnd,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse additive: expr ('+' | '-') expr
    fn parse_additive(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match &self.current {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_multiplicative()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse multiplicative: expr ('*' | '/' | '%') expr
    fn parse_multiplicative(&mut self) -> Result<Expression, ExpressionError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match &self.current {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                Token::Percent => BinaryOperator::Mod,
                Token::ShiftLeft => BinaryOperator::ShiftLeft,
                Token::ShiftRight => BinaryOperator::ShiftRight,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_unary()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse unary: ('not' | '-' | '~') expr | postfix
    fn parse_unary(&mut self) -> Result<Expression, ExpressionError> {
        match &self.current {
            Token::Not => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Not,
                    operand: Box::new(operand),
                })
            }
            Token::Minus => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Neg,
                    operand: Box::new(operand),
                })
            }
            Token::Tilde => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::BitwiseNot,
                    operand: Box::new(operand),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// Parse postfix: primary ('.' method_or_field | '[' index ']')*
    ///
    /// Both `.member` access and `[index]` subscript are left-associative and
    /// interleave freely: `A[i].F`, `MAP[KEY]`, `A[_index - 1].F` all parse into
    /// a chain of `Index` nodes composing with member access.
    fn parse_postfix(&mut self) -> Result<Expression, ExpressionError> {
        let mut expr = self.parse_primary()?;
        loop {
            match &self.current {
                Token::Dot => {
                    self.advance()?;
                    expr = self.parse_member_or_method(expr)?;
                }
                Token::LBracket => {
                    self.advance()?;
                    let index = self.parse_expression()?;
                    self.expect(Token::RBracket)?;
                    expr = Expression::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    /// Parse the `.member` or `.method` that follows a `.` in postfix position.
    fn parse_member_or_method(&mut self, expr: Expression) -> Result<Expression, ExpressionError> {
        match &self.current {
            Token::Ident(name) => {
                let name = name.clone();
                self.advance()?;
                if METHODS.contains(&name.as_str()) {
                    Ok(Expression::MethodCall {
                        target: Box::new(expr),
                        method: name,
                    })
                } else {
                    // Field access. On a bare/dotted field reference we keep the
                    // existing path-string lowering (`parent.child`). After a
                    // subscript the base is an `Index`, not a `FieldRef`, so the
                    // member deref lowers to a string-keyed `Index` — the same
                    // navigation the map-lookup arm uses, resolved against a
                    // struct node.
                    match expr {
                        Expression::FieldRef(mut path) => {
                            path.push('.');
                            path.push_str(&name);
                            Ok(Expression::FieldRef(path))
                        }
                        base => Ok(Expression::Index {
                            base: Box::new(base),
                            index: Box::new(Expression::Literal(Literal::String(name))),
                        }),
                    }
                }
            }
            _ => Err(ExpressionError::SyntaxError {
                message: "Expected identifier after '.'".to_string(),
            }),
        }
    }

    /// Parse primary: literal | identifier | special_var | '(' expr ')'
    fn parse_primary(&mut self) -> Result<Expression, ExpressionError> {
        match &self.current {
            Token::Integer(n) => {
                let n = *n;
                self.advance()?;
                Ok(Expression::Literal(Literal::Integer(n)))
            }
            Token::Float(f) => {
                let f = *f;
                self.advance()?;
                Ok(Expression::Literal(Literal::Float(f)))
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance()?;
                Ok(Expression::Literal(Literal::String(s)))
            }
            Token::Boolean(b) => {
                let b = *b;
                self.advance()?;
                Ok(Expression::Literal(Literal::Boolean(b)))
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.advance()?;
                // Check for special variables. `_root`/`_parent`/`_io`
                // navigators were removed from the language; only `_index`
                // remains. Any other identifier — including a stray `_root`
                // token — parses as a plain field reference and fails with
                // `UnknownField` at eval time, since no field carries that name.
                match name.as_str() {
                    "_index" => Ok(Expression::SpecialVar(SpecialVariable::Index)),
                    _ => Ok(Expression::FieldRef(name)),
                }
            }
            Token::LParen => {
                self.advance()?;
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            _ => Err(ExpressionError::SyntaxError {
                message: format!("Unexpected token: {:?}", self.current),
            }),
        }
    }
}
