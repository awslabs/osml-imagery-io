//! Parser for expression strings using recursive descent.

use super::lexer::{Lexer, Token};
use super::{BinaryOperator, Expression, Literal, SpecialVariable, UnaryOperator};
use crate::parser::error::ExpressionError;

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
        let mut left = self.parse_additive()?;
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
            let right = self.parse_additive()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
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

    /// Parse unary: ('not' | '-') expr | postfix
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
            _ => self.parse_postfix(),
        }
    }

    /// Parse postfix: primary ('.' method_or_field)*
    fn parse_postfix(&mut self) -> Result<Expression, ExpressionError> {
        let mut expr = self.parse_primary()?;
        while self.current == Token::Dot {
            self.advance()?;
            match &self.current {
                Token::Ident(name) => {
                    let name = name.clone();
                    self.advance()?;
                    // Check if this is a method call
                    if name == "to_i" || name == "to_s" || name == "length" {
                        expr = Expression::MethodCall {
                            target: Box::new(expr),
                            method: name,
                        };
                    } else {
                        // It's a field access - append to path
                        match expr {
                            Expression::FieldRef(ref mut path) => {
                                path.push('.');
                                path.push_str(&name);
                            }
                            Expression::SpecialVar(var) => {
                                // Convert special var to field ref with path
                                let var_name = match var {
                                    SpecialVariable::Root => "_root",
                                    SpecialVariable::Parent => "_parent",
                                    SpecialVariable::Index => "_index",
                                    SpecialVariable::Io => "_io",
                                };
                                expr = Expression::FieldRef(format!("{}.{}", var_name, name));
                            }
                            _ => {
                                return Err(ExpressionError::SyntaxError {
                                    message: format!(
                                        "Cannot access field '{}' on non-field expression",
                                        name
                                    ),
                                });
                            }
                        }
                    }
                }
                _ => {
                    return Err(ExpressionError::SyntaxError {
                        message: "Expected identifier after '.'".to_string(),
                    });
                }
            }
        }
        Ok(expr)
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
                // Check for special variables
                match name.as_str() {
                    "_index" => Ok(Expression::SpecialVar(SpecialVariable::Index)),
                    "_root" => Ok(Expression::SpecialVar(SpecialVariable::Root)),
                    "_parent" => Ok(Expression::SpecialVar(SpecialVariable::Parent)),
                    "_io" => Ok(Expression::SpecialVar(SpecialVariable::Io)),
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
