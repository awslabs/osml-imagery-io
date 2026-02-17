//! Lexer for tokenizing expression strings.

use std::iter::Peekable;
use std::str::Chars;

use crate::parser::error::ExpressionError;

/// Token types for the lexer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    // Literals
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    // Identifiers and keywords
    Ident(String),
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    Not,
    // Punctuation
    Dot,
    LParen,
    RParen,
    // End of input
    Eof,
}

/// Lexer for tokenizing expression strings.
pub(crate) struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    current_pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            current_pos: 0,
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.next();
        if c.is_some() {
            self.current_pos += 1;
        }
        c
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self, first: char) -> Result<Token, ExpressionError> {
        let mut num_str = String::new();
        num_str.push(first);
        let mut has_dot = false;

        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                num_str.push(self.next_char().unwrap());
            } else if c == '.' && !has_dot {
                // Check if this is a decimal point or a method call
                let mut temp_chars = self.chars.clone();
                temp_chars.next(); // consume the dot
                if let Some(next) = temp_chars.peek() {
                    if next.is_ascii_digit() {
                        has_dot = true;
                        num_str.push(self.next_char().unwrap());
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if has_dot {
            num_str
                .parse::<f64>()
                .map(Token::Float)
                .map_err(|_| ExpressionError::SyntaxError {
                    message: format!("Invalid float literal: {}", num_str),
                })
        } else {
            num_str
                .parse::<i64>()
                .map(Token::Integer)
                .map_err(|_| ExpressionError::SyntaxError {
                    message: format!("Invalid integer literal: {}", num_str),
                })
        }
    }

    fn read_string(&mut self, quote: char) -> Result<Token, ExpressionError> {
        let mut s = String::new();
        loop {
            match self.next_char() {
                Some(c) if c == quote => return Ok(Token::String(s)),
                Some('\\') => match self.next_char() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some(c) if c == quote => s.push(c),
                    Some(c) => s.push(c),
                    None => {
                        return Err(ExpressionError::SyntaxError {
                            message: "Unterminated string literal".to_string(),
                        })
                    }
                },
                Some(c) => s.push(c),
                None => {
                    return Err(ExpressionError::SyntaxError {
                        message: "Unterminated string literal".to_string(),
                    })
                }
            }
        }
    }

    fn read_identifier(&mut self, first: char) -> Token {
        let mut ident = String::new();
        ident.push(first);

        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                ident.push(self.next_char().unwrap());
            } else {
                break;
            }
        }

        // Check for keywords
        match ident.as_str() {
            "true" => Token::Boolean(true),
            "false" => Token::Boolean(false),
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            _ => Token::Ident(ident),
        }
    }

    pub fn next_token(&mut self) -> Result<Token, ExpressionError> {
        self.skip_whitespace();

        match self.next_char() {
            None => Ok(Token::Eof),
            Some(c) => match c {
                '+' => Ok(Token::Plus),
                '-' => Ok(Token::Minus),
                '*' => Ok(Token::Star),
                '/' => Ok(Token::Slash),
                '%' => Ok(Token::Percent),
                '.' => Ok(Token::Dot),
                '(' => Ok(Token::LParen),
                ')' => Ok(Token::RParen),
                '=' => {
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        Ok(Token::EqEq)
                    } else {
                        Err(ExpressionError::SyntaxError {
                            message: "Expected '==' but found single '='".to_string(),
                        })
                    }
                }
                '!' => {
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        Ok(Token::NotEq)
                    } else {
                        Ok(Token::Not)
                    }
                }
                '<' => {
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        Ok(Token::LtEq)
                    } else {
                        Ok(Token::Lt)
                    }
                }
                '>' => {
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        Ok(Token::GtEq)
                    } else {
                        Ok(Token::Gt)
                    }
                }
                '"' | '\'' => self.read_string(c),
                c if c.is_ascii_digit() => self.read_number(c),
                c if c.is_alphabetic() || c == '_' => Ok(self.read_identifier(c)),
                c => Err(ExpressionError::SyntaxError {
                    message: format!("Unexpected character: '{}'", c),
                }),
            },
        }
    }
}
