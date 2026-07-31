//! Storage engine (Phase 1).
//!
//! Implements the storage layer from the bottom up, following
//! `docs/phases/phase-1.md`:
//!
//! - [`page`]: fixed-size disk pages and raw I/O
//! - [`slotted_page`]: slotted-page layout on top of a raw page
//! - [`heap_file`]: a collection of pages in a single file
//! - [`row`]: length-prefixed tuple serialization
//! - [`catalog`]: `_tables` / `_columns` system tables stored as heap files
//!
//! This module must never depend on SQL.

pub mod catalog;
pub mod heap_file;
pub mod page;
pub mod row;
pub mod slotted_page;

pub use catalog::{Catalog, ColumnDef, ColumnSchema, ColumnType, TableSchema};
pub use heap_file::HeapFile;
pub use page::{Page, PageId, PAGE_SIZE};
pub use row::{deserialize, serialize, Value};
pub use slotted_page::SlottedPage;
