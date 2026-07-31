//! System catalog (Phase 1, step 5).
//!
//! The catalog stores table and column metadata in two system tables,
//! `_tables` and `_columns`, each persisted as its own heap file:
//!
//! - `_tables`:  `(table_id, name, root_page)`
//! - `_columns`: `(table_id, column_id, name, type, nullable)`
//!
//! Creating a user table inserts rows into both system tables, mirroring how
//! production databases (e.g. PostgreSQL's `pg_class`/`pg_attribute`) manage
//! metadata. Reopening a database re-reads the catalog to learn which tables
//! exist. This is "CREATE TABLE at the storage level"; the SQL front-end is a
//! later phase.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::heap_file::HeapFile;
use super::page::PageId;
use super::row::{deserialize, serialize, Value};
use super::slotted_page::SlottedPage;
use crate::error::Error;
use crate::Result;

/// File names of the two system-table heap files.
const TABLES_FILE: &str = "_tables.db";
const COLUMNS_FILE: &str = "_columns.db";

/// Supported column types (a subset of SQL types, per the project scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Integer,
    Varchar,
    Boolean,
}

impl ColumnType {
    /// The type name stored in the catalog.
    pub fn name(self) -> &'static str {
        match self {
            ColumnType::Integer => "INTEGER",
            ColumnType::Varchar => "VARCHAR",
            ColumnType::Boolean => "BOOLEAN",
        }
    }

    /// Resolves a type name stored in the catalog.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "INTEGER" => Some(ColumnType::Integer),
            "VARCHAR" => Some(ColumnType::Varchar),
            "BOOLEAN" => Some(ColumnType::Boolean),
            _ => None,
        }
    }
}

/// Description of a column given by the user when creating a table.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub column_type: ColumnType,
    pub nullable: bool,
}

impl ColumnDef {
    pub fn new(name: &str, column_type: ColumnType, nullable: bool) -> Self {
        Self { name: name.to_string(), column_type, nullable }
    }
}

/// Catalog metadata of a single column.
#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub table_id: u32,
    /// 1-based position of the column within its table.
    pub column_id: u32,
    pub name: String,
    pub column_type: ColumnType,
    pub nullable: bool,
}

/// Catalog metadata of a single table.
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub table_id: u32,
    pub name: String,
    /// First page of the table's heap file (placeholder until data files are
    /// wired up in a later phase).
    pub root_page: PageId,
    pub columns: Vec<ColumnSchema>,
}

/// Manages the `_tables` and `_columns` system tables.
pub struct Catalog {
    dir: PathBuf,
    tables_heap: HeapFile,
    columns_heap: HeapFile,
}

impl Catalog {
    /// Creates a fresh database directory and empty system tables.
    pub fn create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let tables_heap = HeapFile::create(&dir.join(TABLES_FILE))?;
        let columns_heap = HeapFile::create(&dir.join(COLUMNS_FILE))?;
        Ok(Self { dir: dir.to_path_buf(), tables_heap, columns_heap })
    }

    /// Opens an existing database directory and loads its system tables.
    pub fn open(dir: &Path) -> Result<Self> {
        let tables_heap = HeapFile::open(&dir.join(TABLES_FILE))?;
        let columns_heap = HeapFile::open(&dir.join(COLUMNS_FILE))?;
        Ok(Self { dir: dir.to_path_buf(), tables_heap, columns_heap })
    }

    /// The database directory of this catalog.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Creates a user table, writing catalog rows into the system tables.
    pub fn create_table(&mut self, name: &str, columns: &[ColumnDef]) -> Result<TableSchema> {
        let existing = self.read_tables()?;
        if existing.iter().any(|(t, _, _)| t == name) {
            return Err(Error::Duplicate(format!("table `{name}` already exists")));
        }

        let table_id = existing.iter().map(|(_, id, _)| *id).max().unwrap_or(0) + 1;
        let root_page = PageId(0);

        let table_row = vec![
            Value::Integer(table_id as i64),
            Value::Varchar(name.to_string()),
            Value::Integer(root_page.0 as i64),
        ];
        Self::insert_row(&mut self.tables_heap, &table_row)?;
        for (index, column) in columns.iter().enumerate() {
            let column_row = vec![
                Value::Integer(table_id as i64),
                Value::Integer(index as i64 + 1),
                Value::Varchar(column.name.clone()),
                Value::Varchar(column.column_type.name().to_string()),
                Value::Boolean(column.nullable),
            ];
            Self::insert_row(&mut self.columns_heap, &column_row)?;
        }

        Ok(TableSchema {
            table_id,
            name: name.to_string(),
            root_page,
            columns: columns
                .iter()
                .enumerate()
                .map(|(index, column)| ColumnSchema {
                    table_id,
                    column_id: index as u32 + 1,
                    name: column.name.clone(),
                    column_type: column.column_type,
                    nullable: column.nullable,
                })
                .collect(),
        })
    }

    /// Returns the schema of the table with the given name.
    pub fn get_table(&mut self, name: &str) -> Result<TableSchema> {
        self.get_all_tables()?
            .into_iter()
            .find(|table| table.name == name)
            .ok_or_else(|| Error::NotFound(format!("table `{name}`")))
    }

    /// Returns the schemas of every known table.
    pub fn get_all_tables(&mut self) -> Result<Vec<TableSchema>> {
        let mut columns_by_table: HashMap<u32, Vec<ColumnSchema>> = HashMap::new();
        for row in Self::read_all_rows(&mut self.columns_heap)? {
            if let [Value::Integer(table_id), Value::Integer(column_id), Value::Varchar(name), Value::Varchar(type_name), Value::Boolean(nullable)] =
                row.as_slice()
            {
                let column_type = ColumnType::from_name(type_name)
                    .ok_or_else(|| Error::Corrupt(format!("unknown column type `{type_name}`")))?;
                columns_by_table.entry(*table_id as u32).or_default().push(ColumnSchema {
                    table_id: *table_id as u32,
                    column_id: *column_id as u32,
                    name: name.clone(),
                    column_type,
                    nullable: *nullable,
                });
            } else {
                return Err(Error::Corrupt("malformed `_columns` row".into()));
            }
        }
        for columns in columns_by_table.values_mut() {
            columns.sort_by_key(|column| column.column_id);
        }

        let mut tables = Vec::new();
        for row in Self::read_all_rows(&mut self.tables_heap)? {
            if let [Value::Integer(table_id), Value::Varchar(name), Value::Integer(root_page)] =
                row.as_slice()
            {
                tables.push(TableSchema {
                    table_id: *table_id as u32,
                    name: name.clone(),
                    root_page: PageId(*root_page as u32),
                    columns: columns_by_table.remove(&(*table_id as u32)).unwrap_or_default(),
                });
            } else {
                return Err(Error::Corrupt("malformed `_tables` row".into()));
            }
        }
        Ok(tables)
    }

    /// Reads `(name, table_id, root_page)` for every row of `_tables`.
    fn read_tables(&mut self) -> Result<Vec<(String, u32, u32)>> {
        Self::read_all_rows(&mut self.tables_heap)?
            .into_iter()
            .map(|row| match row.as_slice() {
                [Value::Integer(id), Value::Varchar(name), Value::Integer(root)] => {
                    Ok((name.clone(), *id as u32, *root as u32))
                }
                _ => Err(Error::Corrupt("malformed `_tables` row".into())),
            })
            .collect()
    }

    /// Inserts a serialized row into `heap`, growing the file if needed.
    fn insert_row(heap: &mut HeapFile, values: &[Value]) -> Result<()> {
        let encoded = serialize(values);
        for page_id in 0..heap.page_count() {
            let page = heap.read_page(PageId(page_id))?;
            if SlottedPage::is_empty_page(&page) {
                continue;
            }
            let mut slotted = SlottedPage::from_page(page)?;
            if slotted.has_space_for(encoded.len()) {
                slotted.insert_record(&encoded)?;
                heap.write_page(&slotted.into_page())?;
                return Ok(());
            }
        }
        let page = heap.allocate_page()?;
        let mut slotted = SlottedPage::new(page);
        slotted.insert_record(&encoded)?;
        heap.write_page(&slotted.into_page())?;
        Ok(())
    }

    /// Reads every non-deleted row stored in `heap`.
    fn read_all_rows(heap: &mut HeapFile) -> Result<Vec<Vec<Value>>> {
        let mut rows = Vec::new();
        for page_id in 0..heap.page_count() {
            let page = heap.read_page(PageId(page_id))?;
            if SlottedPage::is_empty_page(&page) {
                continue;
            }
            let slotted = SlottedPage::from_page(page)?;
            for slot in 0..slotted.num_slots() {
                if slotted.is_deleted(slot) {
                    continue;
                }
                let record = slotted.get_record(slot)?.to_vec();
                rows.push(deserialize(&record)?);
            }
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("a", ColumnType::Integer, false),
            ColumnDef::new("b", ColumnType::Varchar, true),
        ]
    }

    #[test]
    fn table_ids_increment_from_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::create(dir.path()).unwrap();
        let first = catalog.create_table("one", &columns()).unwrap();
        let second = catalog.create_table("two", &columns()).unwrap();
        assert_eq!(first.table_id, 1);
        assert_eq!(second.table_id, 2);
    }

    #[test]
    fn type_names_round_trip() {
        for t in [ColumnType::Integer, ColumnType::Varchar, ColumnType::Boolean] {
            assert_eq!(ColumnType::from_name(t.name()), Some(t));
        }
        assert_eq!(ColumnType::from_name("BLOB"), None);
    }
}
