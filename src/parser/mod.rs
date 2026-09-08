//! SQL parser (Phase 2): LALRPOP-generated parser over `ast.rs`.
//!
//! The grammar in `grammar.lalrpop` is the single source of truth for the
//! SQL subset. Syntactic rules are lowercase, lexemes are UPPERCASE.
//! This module re-exports the AST and exposes `parse`, mapping LALRPOP
//! errors to [`crate::Error::Syntax`] with line/column information.

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(
    #[allow(clippy::all)]
    #[allow(clippy::ptr_arg)]
    #[rustfmt::skip]
    #[allow(unknown_lints)]
    #[allow(unused_parens)]
    pub grammar,
    "/parser/grammar.rs"
);

pub mod ast;

pub use ast::{
    BinaryOp, ColumnDef, CreateTableStatement, DataType, DeleteStatement, Expr,
    InsertStatement, Literal, SelectItem, SelectStatement, Statement,
    UpdateStatement,
};

/// Parse exactly one SQL statement (optional trailing `;` allowed).
pub fn parse(sql: &str) -> crate::Result<Statement> {
    grammar::statementParser::new()
        .parse(sql)
        .map_err(|err| crate::Error::Syntax(describe_parse_error(sql, err)))
}

fn offset_to_line_col(input: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(input.len());
    let prefix = &input[..offset];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().map_or(1, |l| l.len() + 1);
    (line, column)
}

fn describe_parse_error<Tok: std::fmt::Debug, Err: std::fmt::Debug>(
    input: &str,
    err: lalrpop_util::ParseError<usize, Tok, Err>,
) -> String {
    use lalrpop_util::ParseError::*;
    match err {
        InvalidToken { location } => {
            let (line, col) = offset_to_line_col(input, location);
            format!("Syntax error at line {line}, column {col}: invalid token")
        }
        UnrecognizedToken {
            token: (l, tok, _),
            expected,
        } => {
            let (line, col) = offset_to_line_col(input, l);
            format!(
                "Syntax error at line {line}, column {col}: unexpected token {tok:?}; expected one of: {}",
                expected.join(", ")
            )
        }
        UnrecognizedEof { location, expected } => {
            let (line, col) = offset_to_line_col(input, location);
            format!(
                "Syntax error at line {line}, column {col}: unexpected end of input; expected one of: {}",
                expected.join(", ")
            )
        }
        ExtraToken { token: (l, tok, _) } => {
            let (line, col) = offset_to_line_col(input, l);
            format!("Syntax error at line {line}, column {col}: extra token {tok:?}")
        }
        User { error } => format!("Syntax error: {error:?}"),
    }
}
