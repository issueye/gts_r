//! Lexical analysis for GoScript.

mod lexer;
mod token;

pub use lexer::Lexer;
pub use token::{is_keyword, lookup_ident, Token, TokenKind};
