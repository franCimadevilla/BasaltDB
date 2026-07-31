//! Heap file (Phase 1, step 3).
//!
//! A heap file is a collection of fixed-size pages stored in a single
//! operating-system file. There is no structure linking pages together: the
//! file is just `page_count` consecutive [`Page`]s.
//!
//! Responsibilities:
//! - allocate new pages (reusing freed ones via an in-memory free list)
//! - read and write individual pages
//! - delete pages (zeroed on disk and returned to the free list)
//!
//! Note: the free list lives in memory, so it is not restored when the file is
//! reopened. Freed pages simply reappear as zeroed allocated pages, which is
//! safe (they contain no valid slotted-page header).

use std::fs::{File, OpenOptions};
use std::path::Path;

use super::page::{Page, PageId, PAGE_SIZE};
use crate::error::Error;
use crate::Result;

/// A file that stores a collection of fixed-size pages.
pub struct HeapFile {
    file: File,
    /// Number of pages in the file (both in memory and on disk).
    page_count: u32,
    /// Ids of pages that were deleted and can be reused.
    free_pages: Vec<PageId>,
}

impl HeapFile {
    /// Creates a new, empty heap file, truncating any existing file.
    pub fn create(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)?;
        Ok(Self { file, page_count: 0, free_pages: Vec::new() })
    }

    /// Opens an existing heap file.
    ///
    /// The page count is derived from the file length, so pages previously
    /// written are visible after a restart.
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let len = file.metadata()?.len();
        if len % PAGE_SIZE as u64 != 0 {
            return Err(Error::Corrupt(format!(
                "heap file length {len} is not a multiple of PAGE_SIZE ({PAGE_SIZE})"
            )));
        }
        let page_count = (len / PAGE_SIZE as u64) as u32;
        Ok(Self { file, page_count, free_pages: Vec::new() })
    }

    /// Number of pages currently stored in the file.
    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    /// Allocates a fresh, zeroed page.
    ///
    /// A freed page is reused when available; otherwise the file grows by one
    /// page at the end. The page is written to disk immediately so that the
    /// on-disk state always matches `page_count`.
    pub fn allocate_page(&mut self) -> Result<Page> {
        let id = self.free_pages.pop().unwrap_or(PageId(self.page_count));
        let page = Page::new(id);
        page.write_to(&mut self.file)?;
        self.page_count = self.page_count.max(id.0 + 1);
        Ok(page)
    }

    /// Reads the page with the given id from disk.
    pub fn read_page(&mut self, id: PageId) -> Result<Page> {
        if id.0 >= self.page_count {
            return Err(Error::InvalidPageId(id.0));
        }
        Page::read_from(&mut self.file, id)
    }

    /// Writes a page back to disk at its own offset.
    pub fn write_page(&mut self, page: &Page) -> Result<()> {
        page.write_to(&mut self.file)?;
        self.page_count = self.page_count.max(page.id.0 + 1);
        Ok(())
    }

    /// Deletes the page with the given id: zeroes it on disk and puts it on
    /// the free list so it can be reused by a later `allocate_page`.
    pub fn delete_page(&mut self, id: PageId) -> Result<()> {
        if id.0 >= self.page_count {
            return Err(Error::InvalidPageId(id.0));
        }
        let blank = Page::new(id);
        blank.write_to(&mut self.file)?;
        self.free_pages.push(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("heap.db");
        (dir, path)
    }

    #[test]
    fn page_count_matches_file_size_after_reopen() {
        let (_dir, path) = temp_path();
        {
            let mut heap = HeapFile::create(&path).unwrap();
            heap.allocate_page().unwrap();
            heap.allocate_page().unwrap();
            assert_eq!(heap.page_count(), 2);
        }
        let heap = HeapFile::open(&path).unwrap();
        assert_eq!(heap.page_count(), 2);
    }

    #[test]
    fn allocate_writes_page_to_disk_immediately() {
        let (_dir, path) = temp_path();
        let mut heap = HeapFile::create(&path).unwrap();
        let id = heap.allocate_page().unwrap().id;
        let read_back = heap.read_page(id).unwrap();
        assert!(read_back.as_bytes().iter().all(|&b| b == 0));
    }
}
