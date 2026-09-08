//! Integration tests for the Phase 2 SQL parser (LALRPOP).
//!
//! Each module mirrors the grammar in `src/parser/grammar.lalrpop` and the
//! spec in `docs/phases/phase-2.md`:
//! 1. CREATE TABLE
//! 2. INSERT
//! 3. SELECT
//! 4. UPDATE
//! 5. DELETE
//! 6. Expressions (WHERE precedence)
//! 7. Lexing (case-insensitivity, identifiers, strings, `;`)
//! 8. Error reporting (line/column diagnostics)
//!
//! Naming follows the phase doc: `valid_*` asserts full AST equality via
//! `PartialEq`; `invalid_*` asserts `Error::Syntax` plus a message fragment.

use basalt_db::parser::{
    BinaryOp, ColumnDef, DataType, Expr, Literal, SelectItem, Statement,
};
use basalt_db::Result;

fn parse(sql: &str) -> Result<Statement> {
    basalt_db::parser::parse(sql)
}

/// Assert `sql` fails with `Error::Syntax` and return the message so tests
/// can check for `line/column` fragments.
fn expect_syntax_error(sql: &str) -> String {
    match parse(sql) {
        Err(basalt_db::Error::Syntax(msg)) => msg,
        Err(other) => panic!("expected Syntax error for {sql:?}, got: {other:?}"),
        Ok(stmt) => panic!("expected Syntax error for {sql:?}, got: {stmt:?}"),
    }
}

// Small AST builders to keep deep `Expr` trees readable.
fn col(name: &str) -> Expr {
    Expr::Column(name.to_string())
}

fn int(n: i64) -> Expr {
    Expr::Literal(Literal::Integer(n))
}

fn str_lit(s: &str) -> Expr {
    Expr::Literal(Literal::String(s.to_string()))
}

fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn not(e: Expr) -> Expr {
    Expr::Not(Box::new(e))
}

// ---------------------------------------------------------------------------
// 1. CREATE TABLE
// ---------------------------------------------------------------------------

mod create_table_tests {
    use super::*;
    use basalt_db::parser::CreateTableStatement;

    fn create(sql: &str) -> CreateTableStatement {
        match parse(sql).unwrap() {
            Statement::CreateTable(c) => c,
            other => panic!("expected CreateTable, got: {other:?}"),
        }
    }

    #[test]
    fn valid_single_column() {
        assert_eq!(
            create("CREATE TABLE t (id INTEGER)"),
            CreateTableStatement {
                table_name: "t".to_string(),
                columns: vec![ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: true,
                }],
            }
        );
    }

    #[test]
    fn valid_nullability_forms() {
        let c = create(
            "CREATE TABLE s (a INTEGER NOT NULL, b VARCHAR NULL, c BOOLEAN, d VARCHAR NOT NULL)",
        );
        assert_eq!(c.table_name, "s");
        assert_eq!(c.columns.len(), 4);
        assert!(!c.columns[0].nullable);
        assert!(c.columns[1].nullable);
        assert!(c.columns[2].nullable); // omitted => nullable
        assert!(!c.columns[3].nullable);
        assert_eq!(c.columns[1].data_type, DataType::Varchar);
        assert_eq!(c.columns[2].data_type, DataType::Boolean);
    }

    #[test]
    fn valid_keywords_case_insensitive_idents_preserved() {
        let c = create("create table Student (Id integer not null)");
        assert_eq!(c.table_name, "Student");
        assert_eq!(c.columns[0].name, "Id");
    }

    #[test]
    fn valid_trailing_semicolon() {
        let c = create("CREATE TABLE t (id INTEGER);");
        assert_eq!(c.table_name, "t");
    }

    #[test]
    fn invalid_unknown_type() {
        let msg = expect_syntax_error("CREATE TABLE t (id FOO)");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_missing_rparen() {
        let msg = expect_syntax_error("CREATE TABLE t (id INTEGER");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_missing_table_name() {
        let msg = expect_syntax_error("CREATE TABLE (id INTEGER)");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_empty_column_list() {
        let msg = expect_syntax_error("CREATE TABLE t ()");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_trailing_comma() {
        let msg = expect_syntax_error("CREATE TABLE t (id INTEGER,)");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 2. INSERT
// ---------------------------------------------------------------------------

mod insert_tests {
    use super::*;
    use basalt_db::parser::InsertStatement;

    fn insert(sql: &str) -> InsertStatement {
        match parse(sql).unwrap() {
            Statement::Insert(i) => i,
            other => panic!("expected Insert, got: {other:?}"),
        }
    }

    #[test]
    fn valid_without_column_list() {
        assert_eq!(
            insert("INSERT INTO student VALUES (1, 'Ann', TRUE)"),
            InsertStatement {
                table_name: "student".to_string(),
                columns: None,
                values: vec![vec![
                    Literal::Integer(1),
                    Literal::String("Ann".to_string()),
                    Literal::Boolean(true),
                ]],
            }
        );
    }

    #[test]
    fn valid_with_column_list_and_tuples() {
        let i = insert("INSERT INTO t (a, b) VALUES (1, NULL), (2, FALSE)");
        assert_eq!(i.table_name, "t");
        assert_eq!(
            i.columns,
            Some(vec!["a".to_string(), "b".to_string()])
        );
        assert_eq!(i.values.len(), 2);
        assert_eq!(i.values[0][1], Literal::Null);
        assert_eq!(i.values[1], vec![Literal::Integer(2), Literal::Boolean(false)]);
    }

    #[test]
    fn valid_string_escape_and_negative_int() {
        let i = insert("INSERT INTO t VALUES (-5, 'O''Brien')");
        assert_eq!(
            i.values[0],
            vec![
                Literal::Integer(-5),
                Literal::String("O'Brien".to_string()),
            ]
        );
    }

    #[test]
    fn valid_lowercase_with_semicolon() {
        let i = insert("insert into Student values (1);");
        assert_eq!(i.table_name, "Student");
    }

    #[test]
    fn invalid_missing_values() {
        let msg = expect_syntax_error("INSERT INTO t (a, b)");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_unbalanced_paren() {
        let msg = expect_syntax_error("INSERT INTO t VALUES (1, 2");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_unterminated_string() {
        let msg = expect_syntax_error("INSERT INTO t VALUES ('abc)");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 3. SELECT
// ---------------------------------------------------------------------------

mod select_tests {
    use super::*;
    use basalt_db::parser::SelectStatement;

    fn select(sql: &str) -> SelectStatement {
        match parse(sql).unwrap() {
            Statement::Select(s) => s,
            other => panic!("expected Select, got: {other:?}"),
        }
    }

    #[test]
    fn valid_wildcard_no_where() {
        assert_eq!(
            select("SELECT * FROM student"),
            SelectStatement {
                columns: vec![SelectItem::Wildcard],
                table: "student".to_string(),
                predicate: None,
            }
        );
    }

    #[test]
    fn valid_column_list_with_where() {
        assert_eq!(
            select("SELECT name, age FROM student WHERE age > 20"),
            SelectStatement {
                columns: vec![
                    SelectItem::Expr(col("name")),
                    SelectItem::Expr(col("age")),
                ],
                table: "student".to_string(),
                predicate: Some(binary(BinaryOp::Gt, col("age"), int(20))),
            }
        );
    }

    #[test]
    fn valid_trailing_semicolon() {
        let s = select("SELECT * FROM t;");
        assert_eq!(s.table, "t");
        assert!(s.predicate.is_none());
    }

    #[test]
    fn invalid_missing_from() {
        let msg = expect_syntax_error("SELECT * student");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_missing_table() {
        let msg = expect_syntax_error("SELECT * FROM");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_bare_select() {
        let msg = expect_syntax_error("SELECT");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 4. UPDATE
// ---------------------------------------------------------------------------

mod update_tests {
    use super::*;
    use basalt_db::parser::UpdateStatement;

    fn update(sql: &str) -> UpdateStatement {
        match parse(sql).unwrap() {
            Statement::Update(u) => u,
            other => panic!("expected Update, got: {other:?}"),
        }
    }

    #[test]
    fn valid_single_assignment_no_where() {
        assert_eq!(
            update("UPDATE student SET name = 'Bo'"),
            UpdateStatement {
                table: "student".to_string(),
                assignments: vec![("name".to_string(), str_lit("Bo"))],
                predicate: None,
            }
        );
    }

    #[test]
    fn valid_multiple_assignments_with_where() {
        assert_eq!(
            update("UPDATE t SET a = 1, b = c WHERE id <> 0"),
            UpdateStatement {
                table: "t".to_string(),
                assignments: vec![
                    ("a".to_string(), int(1)),
                    ("b".to_string(), col("c")),
                ],
                predicate: Some(binary(BinaryOp::Ne, col("id"), int(0))),
            }
        );
    }

    #[test]
    fn invalid_missing_set() {
        let msg = expect_syntax_error("UPDATE t a = 1");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_missing_eq() {
        let msg = expect_syntax_error("UPDATE t SET a 1");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_empty_assignments() {
        let msg = expect_syntax_error("UPDATE t SET WHERE a = 1");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 5. DELETE
// ---------------------------------------------------------------------------

mod delete_tests {
    use super::*;
    use basalt_db::parser::DeleteStatement;

    fn delete(sql: &str) -> DeleteStatement {
        match parse(sql).unwrap() {
            Statement::Delete(d) => d,
            other => panic!("expected Delete, got: {other:?}"),
        }
    }

    #[test]
    fn valid_without_where() {
        assert_eq!(
            delete("DELETE FROM student"),
            DeleteStatement {
                table: "student".to_string(),
                predicate: None,
            }
        );
    }

    #[test]
    fn valid_with_where_and_semicolon() {
        assert_eq!(
            delete("DELETE FROM t WHERE a <= 1;"),
            DeleteStatement {
                table: "t".to_string(),
                predicate: Some(binary(BinaryOp::Le, col("a"), int(1))),
            }
        );
    }

    #[test]
    fn invalid_missing_from() {
        let msg = expect_syntax_error("DELETE student");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_missing_table() {
        let msg = expect_syntax_error("DELETE FROM WHERE a = 1");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 6. Expressions: precedence OR < AND < NOT < comparison
// ---------------------------------------------------------------------------

mod expr_tests {
    use super::*;
    use basalt_db::parser::SelectStatement;

    fn predicate(sql: &str) -> Expr {
        match parse(sql).unwrap() {
            Statement::Select(SelectStatement {
                predicate: Some(p),
                ..
            }) => p,
            other => panic!("expected Select with WHERE, got: {other:?}"),
        }
    }

    #[test]
    fn valid_or_binds_loosest() {
        // a=1 OR (b=2 AND c=3)
        assert_eq!(
            predicate("SELECT * FROM t WHERE a = 1 OR b = 2 AND c = 3"),
            binary(
                BinaryOp::Or,
                binary(BinaryOp::Eq, col("a"), int(1)),
                binary(
                    BinaryOp::And,
                    binary(BinaryOp::Eq, col("b"), int(2)),
                    binary(BinaryOp::Eq, col("c"), int(3)),
                ),
            )
        );
    }

    #[test]
    fn valid_not_binds_tighter_than_and() {
        // (NOT a=1) AND b=2
        assert_eq!(
            predicate("SELECT * FROM t WHERE NOT a = 1 AND b = 2"),
            binary(
                BinaryOp::And,
                not(binary(BinaryOp::Eq, col("a"), int(1))),
                binary(BinaryOp::Eq, col("b"), int(2)),
            )
        );
    }

    #[test]
    fn valid_all_comparison_ops() {
        let cases = [
            ("=", BinaryOp::Eq),
            ("<", BinaryOp::Lt),
            (">", BinaryOp::Gt),
            ("<=", BinaryOp::Le),
            (">=", BinaryOp::Ge),
            ("<>", BinaryOp::Ne),
        ];
        for (op, expected) in cases {
            assert_eq!(
                predicate(&format!("SELECT * FROM t WHERE a {op} 1")),
                binary(expected, col("a"), int(1)),
                "operator {op}"
            );
        }
    }

    #[test]
    fn valid_parens_override_precedence() {
        // (a=1 OR b=2) AND c=3
        assert_eq!(
            predicate("SELECT * FROM t WHERE (a = 1 OR b = 2) AND c = 3"),
            binary(
                BinaryOp::And,
                binary(
                    BinaryOp::Or,
                    binary(BinaryOp::Eq, col("a"), int(1)),
                    binary(BinaryOp::Eq, col("b"), int(2)),
                ),
                binary(BinaryOp::Eq, col("c"), int(3)),
            )
        );
    }

    #[test]
    fn valid_left_associative_and() {
        // (a=1 AND b=2) AND c=3
        assert_eq!(
            predicate("SELECT * FROM t WHERE a = 1 AND b = 2 AND c = 3"),
            binary(
                BinaryOp::And,
                binary(
                    BinaryOp::And,
                    binary(BinaryOp::Eq, col("a"), int(1)),
                    binary(BinaryOp::Eq, col("b"), int(2)),
                ),
                binary(BinaryOp::Eq, col("c"), int(3)),
            )
        );
    }

    #[test]
    fn valid_mixed_operand_kinds() {
        // literals on both sides, boolean and NULL included
        assert_eq!(
            predicate("SELECT * FROM t WHERE TRUE AND x = NULL"),
            binary(
                BinaryOp::And,
                Expr::Literal(Literal::Boolean(true)),
                binary(
                    BinaryOp::Eq,
                    col("x"),
                    Expr::Literal(Literal::Null),
                ),
            )
        );
    }

    #[test]
    fn invalid_dangling_operator() {
        let msg = expect_syntax_error("SELECT * FROM t WHERE a = ");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_leading_and() {
        let msg = expect_syntax_error("SELECT * FROM t WHERE AND a = 1");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_empty_parens() {
        let msg = expect_syntax_error("SELECT * FROM t WHERE ()");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 7. Lexing: keywords, identifiers, strings, semicolons, whitespace
// ---------------------------------------------------------------------------

mod lexing_tests {
    use super::*;

    #[test]
    fn valid_mixed_case_keywords_everywhere() {
        // If any keyword were case-sensitive, one of these would fail.
        for sql in [
            "cReAtE tAbLe T (A iNtEgEr NoT nUlL)",
            "iNsErT iNtO T VALUES (1)",
            "sElEcT * FrOm T wHeRe A = 1",
            "uPdAtE T SeT A = 2 wHeRe A = 1",
            "dElEtE FrOm T wHeRe A = 2",
        ] {
            assert!(parse(sql).is_ok(), "failed: {sql}");
        }
    }

    #[test]
    fn valid_identifier_case_preserved() {
        match parse("SELECT Name FROM Student").unwrap() {
            Statement::Select(s) => {
                assert_eq!(s.table, "Student");
                assert_eq!(s.columns, vec![SelectItem::Expr(col("Name"))]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn valid_keyword_prefix_is_identifier() {
        // Longest match: `SELECTED` is one IDENT, not SELECT + ED.
        match parse("SELECT SELECTED FROM t").unwrap() {
            Statement::Select(s) => {
                assert_eq!(s.columns, vec![SelectItem::Expr(col("SELECTED"))]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn valid_whitespace_variants() {
        let a = parse("SELECT * FROM t WHERE a=1").unwrap();
        let b = parse("  SELECT\t*\nFROM\r\nt\nWHERE\n a = 1  ").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn invalid_double_semicolon() {
        let msg = expect_syntax_error("SELECT * FROM t;;");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }

    #[test]
    fn invalid_two_statements() {
        let msg = expect_syntax_error("SELECT * FROM t; DELETE FROM t");
        assert!(msg.contains("line 1, column"), "msg: {msg}");
    }
}

// ---------------------------------------------------------------------------
// 8. Error reporting: variant + line/column diagnostics
// ---------------------------------------------------------------------------

mod error_tests {
    use super::*;

    #[test]
    fn invalid_token_reports_position() {
        // `@` cannot start any token.
        let msg = expect_syntax_error("SELECT @ FROM t");
        assert!(msg.contains("line 1, column 8"), "msg: {msg}");
    }

    #[test]
    fn unrecognized_token_names_expectations() {
        let msg = expect_syntax_error("SELECT * FROM");
        assert!(msg.contains("line 1"), "msg: {msg}");
        assert!(msg.contains("expected"), "msg: {msg}");
    }

    #[test]
    fn multiline_error_reports_line_2() {
        let msg = expect_syntax_error("CREATE TABLE t (\n  id FOO\n)");
        assert!(msg.contains("line 2"), "msg: {msg}");
    }

    #[test]
    fn empty_input_is_syntax_error() {
        let msg = expect_syntax_error("");
        assert!(msg.contains("line 1, column 1"), "msg: {msg}");
    }

    #[test]
    fn error_display_mentions_syntax() {
        let err = parse("SELECT FROM WHERE").unwrap_err();
        assert!(matches!(err, basalt_db::Error::Syntax(_)));
        assert!(err.to_string().contains("syntax"), "err: {err}");
    }
}
