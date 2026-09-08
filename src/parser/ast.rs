//! Abstract syntax tree for the Phase 2 SQL subset (see `docs/phases/phase-2.md`).
//!
//! These types intentionally mirror nothing from `storage`; the planner
//! phase will convert them into storage-level types.

/// A complete SQL statement (exactly one per `parse` call).
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    CreateTable(CreateTableStatement),
    Insert(InsertStatement),
    Select(SelectStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,      // name, data_type, nullable
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,               // true si "NULL" u omitido; false si "NOT NULL"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType { Integer, Varchar, Boolean }   // espeja storage::ColumnType

#[derive(Debug, Clone, PartialEq)]
pub struct InsertStatement {
    pub table_name: String,
    pub columns: Option<Vec<String>>, // None => sin lista de columnas
    pub values: Vec<Vec<Literal>>,    // un Vec<Literal> por tupla
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub columns: Vec<SelectItem>,     // '*' o expresiones
    pub table: String,
    pub predicate: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem { Wildcard, Expr(Expr) }

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatement {
    pub table: String,
    pub assignments: Vec<(String, Expr)>,
    pub predicate: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStatement {
    pub table: String,
    pub predicate: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal { Integer(i64), String(String), Boolean(bool), Null }

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Column(String),
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    Not(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp { Eq, Lt, Gt, Le, Ge, Ne, And, Or }
