pub(crate) mod ast;
mod lexer;
mod parser;

use crate::diagnostic::Result;

pub(crate) fn parse(source: &str) -> Result<ast::File> {
    parser::parse(lexer::tokenize(source)?)
}
