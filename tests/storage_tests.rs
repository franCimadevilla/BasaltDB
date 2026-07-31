#[test]
fn write_and_read_page_persists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    // ... escribir página, drop, reabrir, leer y assert
}