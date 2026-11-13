//! Lexer, parser, and AST for Terra.
//!
//! Statements end with `;`. Braces close `func`, `class`, `if`, `while`,
//! `for`, `loop`, `match`, and `try` without a required semicolon after `}`.
//! An extra `;` is an empty statement, so `};` inside a block is legal.

pub mod ast;
mod lexer;
mod parser;
mod token;

pub use ast::*;
pub use lexer::lex;
pub use parser::parse;
pub use token::{Token, TokenKind};
