//! Parser proof demo for BasaltDB Phase 2.
//!
//! Parses a fixed list of representative SQL statements with
//! [`basalt_db::parser::parse`] and prints each input together with the
//! produced [`basalt_db::parser::Statement`] (pretty `Debug`) or, for the
//! invalid samples, the resulting [`basalt_db::Error::Syntax`] message.
//!
//! The output of this program is captured verbatim as Part A of
//! `docs/phases/phase-2-proof.txt`. Run with:
//!
//! ```powershell
//! cargo run --example parse_demo
//! ```

fn main() {
    // (label, sql). Labels keep the transcript readable; the grammar itself
    // is the single source of truth in `src/parser/grammar.lalrpop`.
    let samples: &[(&str, &str)] = &[
        (
            "1. CREATE TABLE (all types + nullability forms)",
            "CREATE TABLE student (id INTEGER NOT NULL, name VARCHAR, active BOOLEAN)",
        ),
        (
            "2. INSERT without column list (multi-tuple, string escape, booleans)",
            "INSERT INTO student VALUES (1, 'O''Brien', TRUE), (2, 'Ann', FALSE)",
        ),
        (
            "3. INSERT with column list",
            "INSERT INTO student (id, name) VALUES (3, 'Bo')",
        ),
        (
            "4. SELECT with WHERE (canonical phase-doc example)",
            "SELECT name FROM student WHERE age > 20",
        ),
        (
            "5. SELECT * with AND/OR/NOT precedence",
            "SELECT * FROM student WHERE a = 1 OR NOT b <> 'x' AND c > 2",
        ),
        (
            "6. UPDATE with assignments + WHERE",
            "UPDATE student SET name = 'Bo' WHERE id = 1",
        ),
        (
            "7. DELETE with WHERE",
            "DELETE FROM student WHERE id = 1",
        ),
        (
            "8. Case-insensitive keywords, identifier case preserved",
            "select * from Student where Age >= 20",
        ),
        (
            "9. INVALID: unknown column type",
            "CREATE TABLE t (id FOO)",
        ),
        (
            "10. INVALID: missing table after FROM",
            "SELECT * FROM",
        ),
    ];

    for (label, sql) in samples {
        println!("=== {label} ===");
        println!("SQL: {sql}");
        match basalt_db::parser::parse(sql) {
            Ok(stmt) => println!("AST:\n{stmt:#?}"),
            Err(err) => println!("ERROR: {err}"),
        }
        println!();
    }
}
