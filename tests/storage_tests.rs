//! Integration tests for the Phase 1 storage engine.
//!
//! Each module mirrors one step of `docs/phases/phase-1.md`:
//! 1. Fixed-size page + I/O
//! 2. Slotted page
//! 3. Heap file
//! 4. Row serialization
//! 5. Catalog

use std::fs::OpenOptions;

use basalt_db::storage::catalog::{Catalog, ColumnDef, ColumnType, TableSchema};
use basalt_db::storage::heap_file::HeapFile;
use basalt_db::storage::page::{Page, PageId, PAGE_SIZE};
use basalt_db::storage::row::{deserialize, serialize, Value};
use basalt_db::storage::slotted_page::SlottedPage;

fn temp_db_file(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    (dir, path)
}

// ---------------------------------------------------------------------------
// Step 1: fixed-size page + I/O
// ---------------------------------------------------------------------------

mod page_tests {
    use super::*;

    #[test]
    fn new_page_is_zeroed() {
        let page = Page::new(PageId(7));
        assert_eq!(page.id, PageId(7));
        assert!(page.as_bytes().iter().all(|&b| b == 0));
        assert_eq!(page.as_bytes().len(), PAGE_SIZE);
    }

    #[test]
    fn write_and_read_round_trip() {
        let (_dir, path) = temp_db_file("page.db");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        let mut page = Page::new(PageId(0));
        let pattern: Vec<u8> = (0..PAGE_SIZE).map(|i| (i % 251) as u8).collect();
        page.as_bytes_mut().copy_from_slice(&pattern);
        page.write_to(&mut file).unwrap();

        let mut reopened = OpenOptions::new().read(true).write(true).open(&path).unwrap();
        let loaded = Page::read_from(&mut reopened, PageId(0)).unwrap();
        assert_eq!(loaded.id, PageId(0));
        assert_eq!(loaded.as_bytes(), pattern.as_slice());
    }

    #[test]
    fn pages_are_written_at_their_offset() {
        let (_dir, path) = temp_db_file("offset.db");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        let mut p0 = Page::new(PageId(0));
        p0.as_bytes_mut().fill(0xAA);
        p0.write_to(&mut file).unwrap();

        let mut p3 = Page::new(PageId(3));
        p3.as_bytes_mut().fill(0xBB);
        p3.write_to(&mut file).unwrap();

        // Page 3 must live at byte offset 3 * PAGE_SIZE, not after page 0.
        let file_len = std::fs::metadata(&path).unwrap().len();
        assert_eq!(file_len, 4 * PAGE_SIZE as u64);

        let loaded = Page::read_from(&mut file, PageId(3)).unwrap();
        assert!(loaded.as_bytes().iter().all(|&b| b == 0xBB));

        let loaded0 = Page::read_from(&mut file, PageId(0)).unwrap();
        assert!(loaded0.as_bytes().iter().all(|&b| b == 0xAA));
    }
}

// ---------------------------------------------------------------------------
// Step 2: slotted page
// ---------------------------------------------------------------------------

mod slotted_page_tests {
    use super::*;

    #[test]
    fn insert_and_get_single_record() {
        let page = Page::new(PageId(0));
        let mut sp = SlottedPage::new(page);

        let slot = sp.insert_record(b"hello").unwrap();
        assert_eq!(sp.get_record(slot).unwrap(), b"hello");
        assert_eq!(sp.num_slots(), 1);
    }

    #[test]
    fn insert_multiple_records_of_varying_sizes() {
        let mut sp = SlottedPage::new(Page::new(PageId(0)));

        let records: Vec<Vec<u8>> = vec![
            b"a".to_vec(),
            b"".to_vec(),
            b"record with a longer payload".to_vec(),
            vec![0u8; 100],
            vec![7u8; 2048],
        ];
        let mut slots = Vec::new();
        for record in &records {
            slots.push(sp.insert_record(record).unwrap());
        }

        assert_eq!(sp.num_slots(), records.len() as u16);
        for (slot, expected) in slots.iter().zip(&records) {
            assert_eq!(sp.get_record(*slot).unwrap(), expected.as_slice());
        }
    }

    #[test]
    fn invalid_slot_is_rejected() {
        let mut sp = SlottedPage::new(Page::new(PageId(0)));
        assert!(sp.insert_record(b"x").is_ok());
        assert!(sp.get_record(99).is_err());
    }

    #[test]
    fn record_larger_than_page_is_rejected() {
        let mut sp = SlottedPage::new(Page::new(PageId(0)));
        let huge = vec![0u8; PAGE_SIZE];
        assert!(sp.insert_record(&huge).is_err());
        assert_eq!(sp.num_slots(), 0);
    }

    #[test]
    fn page_fills_up_and_then_rejects() {
        let mut sp = SlottedPage::new(Page::new(PageId(0)));
        // 116 bytes of data + 2-byte length prefix + 2-byte slot = 120 bytes.
        // The initial free space (4096 - 16 header bytes = 4080) divides evenly
        // by 120, so the page fills to exactly zero free bytes.
        let record = vec![0xAB; 116];

        let mut inserted = 0;
        while sp.insert_record(&record).is_ok() {
            inserted += 1;
        }
        assert_eq!(inserted, 34);
        assert_eq!(sp.free_space(), 0);
        // Records already inserted must remain readable.
        assert_eq!(sp.get_record(0).unwrap().len(), 116);
    }

    #[test]
    fn slotted_page_persists_across_reload() {
        let (_dir, path) = temp_db_file("slotted.db");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        {
            let mut sp = SlottedPage::new(Page::new(PageId(2)));
            sp.insert_record(b"first record").unwrap();
            sp.insert_record(b"second").unwrap();
            sp.insert_record(b"third record with more bytes").unwrap();
            sp.page().write_to(&mut file).unwrap();
        }

        let mut reopened = OpenOptions::new().read(true).write(true).open(&path).unwrap();
        let loaded_page = Page::read_from(&mut reopened, PageId(2)).unwrap();
        let sp = SlottedPage::from_page(loaded_page).unwrap();
        assert_eq!(sp.num_slots(), 3);
        assert_eq!(sp.get_record(0).unwrap(), b"first record");
        assert_eq!(sp.get_record(1).unwrap(), b"second");
        assert_eq!(sp.get_record(2).unwrap(), b"third record with more bytes");
    }

    #[test]
    fn delete_marks_slot_as_unreadable() {
        let mut sp = SlottedPage::new(Page::new(PageId(0)));
        let slot = sp.insert_record(b"to be deleted").unwrap();
        sp.delete_record(slot).unwrap();
        assert!(sp.get_record(slot).is_err());
        assert_eq!(sp.num_slots(), 1);
    }
}

// ---------------------------------------------------------------------------
// Step 3: heap file
// ---------------------------------------------------------------------------

mod heap_file_tests {
    use super::*;

    #[test]
    fn allocate_and_write_pages_persists() {
        let (_dir, path) = temp_db_file("heap.db");
        {
            let mut heap = HeapFile::create(&path).unwrap();
            assert_eq!(heap.page_count(), 0);

            let mut p0 = heap.allocate_page().unwrap();
            p0.as_bytes_mut().fill(0x10);
            heap.write_page(&p0).unwrap();

            let mut p1 = heap.allocate_page().unwrap();
            p1.as_bytes_mut().fill(0x20);
            heap.write_page(&p1).unwrap();

            assert_eq!(heap.page_count(), 2);
        }

        let mut heap = HeapFile::open(&path).unwrap();
        assert_eq!(heap.page_count(), 2);
        let p0 = heap.read_page(PageId(0)).unwrap();
        assert!(p0.as_bytes().iter().all(|&b| b == 0x10));
        let p1 = heap.read_page(PageId(1)).unwrap();
        assert!(p1.as_bytes().iter().all(|&b| b == 0x20));
    }

    #[test]
    fn newly_allocated_page_is_zeroed() {
        let (_dir, path) = temp_db_file("zeroed.db");
        let mut heap = HeapFile::create(&path).unwrap();
        let page = heap.allocate_page().unwrap();
        assert!(page.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn reading_past_the_end_fails() {
        let (_dir, path) = temp_db_file("end.db");
        let mut heap = HeapFile::create(&path).unwrap();
        assert!(heap.read_page(PageId(5)).is_err());
    }

    #[test]
    fn deleted_pages_are_reused() {
        let (_dir, path) = temp_db_file("reuse.db");
        let mut heap = HeapFile::create(&path).unwrap();

        let p0 = heap.allocate_page().unwrap();
        heap.write_page(&p0).unwrap();
        let p1 = heap.allocate_page().unwrap();
        heap.write_page(&p1).unwrap();
        let p2 = heap.allocate_page().unwrap();
        heap.write_page(&p2).unwrap();

        heap.delete_page(PageId(1)).unwrap();
        let reused = heap.allocate_page().unwrap();
        assert_eq!(reused.id, PageId(1));
        assert!(reused.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn deleting_zeroes_the_page_on_disk() {
        let (_dir, path) = temp_db_file("deleted.db");
        let mut heap = HeapFile::create(&path).unwrap();

        let mut p0 = heap.allocate_page().unwrap();
        p0.as_bytes_mut().fill(0xFF);
        heap.write_page(&p0).unwrap();

        heap.delete_page(PageId(0)).unwrap();
        let p0 = heap.read_page(PageId(0)).unwrap();
        assert!(p0.as_bytes().iter().all(|&b| b == 0));
    }
}

// ---------------------------------------------------------------------------
// Step 4: row serialization
// ---------------------------------------------------------------------------

mod row_tests {
    use super::*;

    #[test]
    fn round_trip_simple_values() {
        let row = vec![
            Value::Integer(42),
            Value::Varchar("hello".to_string()),
            Value::Boolean(true),
        ];
        let bytes = serialize(&row);
        assert_eq!(deserialize(&bytes).unwrap(), row);
    }

    #[test]
    fn round_trip_with_nulls() {
        let row = vec![
            Value::Integer(-7),
            Value::Null,
            Value::Varchar(String::new()),
            Value::Null,
            Value::Boolean(false),
        ];
        let bytes = serialize(&row);
        assert_eq!(deserialize(&bytes).unwrap(), row);
    }

    #[test]
    fn round_trip_empty_row() {
        let bytes = serialize(&[]);
        assert_eq!(deserialize(&bytes).unwrap(), Vec::<Value>::new());
    }

    #[test]
    fn round_trip_unicode_varchars() {
        let row = vec![
            Value::Varchar("¡hola! π≈3.14159".to_string()),
            Value::Varchar("emoji: 🪨".to_string()),
        ];
        let bytes = serialize(&row);
        assert_eq!(deserialize(&bytes).unwrap(), row);
    }

    #[test]
    fn big_endian_layout_places_high_byte_first() {
        let row = vec![Value::Integer(0x0102_0304_0506_0708)];
        let bytes = serialize(&row);

        // Layout: [u32 total_len][u16 count][null bitmap][tag][i64 big-endian]
        assert_eq!(bytes.len(), 4 + 2 + 1 + 1 + 8);
        let high_byte_index = bytes.len() - 8;
        assert_eq!(bytes[high_byte_index], 0x01);
        assert_eq!(bytes[high_byte_index + 7], 0x08);
    }

    #[test]
    fn deserializing_garbage_fails() {
        assert!(deserialize(&[0u8; 4]).is_err());
        assert!(deserialize(&[0xFF, 0x00, 0x00, 0x00]).is_err());
    }
}

// ---------------------------------------------------------------------------
// Step 5: catalog
// ---------------------------------------------------------------------------

mod catalog_tests {
    use super::*;

    fn user_columns() -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("id", ColumnType::Integer, false),
            ColumnDef::new("name", ColumnType::Varchar, false),
            ColumnDef::new("active", ColumnType::Boolean, true),
        ]
    }

    #[test]
    fn create_table_and_query_schema() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::create(dir.path()).unwrap();

        let table = catalog
            .create_table("users", &user_columns())
            .unwrap();
        assert_eq!(table.table_id, 1);
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 3);

        let fetched = catalog.get_table("users").unwrap();
        assert_eq!(fetched.name, "users");
        assert_eq!(fetched.columns[0].name, "id");
        assert_eq!(fetched.columns[0].column_type, ColumnType::Integer);
        assert!(!fetched.columns[0].nullable);
        assert_eq!(fetched.columns[2].column_type, ColumnType::Boolean);
        assert!(fetched.columns[2].nullable);
    }

    #[test]
    fn catalog_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut catalog = Catalog::create(dir.path()).unwrap();
            catalog
                .create_table("users", &user_columns())
                .unwrap();
            catalog
                .create_table("orders", &[ColumnDef::new("amount", ColumnType::Integer, false)])
                .unwrap();
        }

        let mut catalog = Catalog::open(dir.path()).unwrap();
        let tables = catalog.get_all_tables().unwrap();
        assert_eq!(tables.len(), 2);

        let users = catalog.get_table("users").unwrap();
        assert_eq!(users.columns.len(), 3);
        let orders = catalog.get_table("orders").unwrap();
        assert_eq!(orders.table_id, 2);
        assert_eq!(orders.columns.len(), 1);
    }

    #[test]
    fn duplicate_table_name_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::create(dir.path()).unwrap();
        catalog.create_table("users", &user_columns()).unwrap();
        assert!(catalog.create_table("users", &user_columns()).is_err());
    }

    #[test]
    fn unknown_table_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::create(dir.path()).unwrap();
        assert!(catalog.get_table("missing").is_err());
    }

    #[test]
    fn freshly_created_catalog_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::create(dir.path()).unwrap();
        assert!(catalog.get_all_tables().unwrap().is_empty());
    }

    // Keep `TableSchema` imported in scope to avoid unused warnings.
    fn _assert_schema_type(_: TableSchema) {}
}
