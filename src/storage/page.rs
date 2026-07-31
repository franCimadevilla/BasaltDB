//! Fixed-size disk pages (Phase 1, step 1).
//!
//! A page is the unit of I/O: the database file is divided into
//! [`PAGE_SIZE`]-byte pages, each identified by a [`PageId`].
//!
//! This module only deals with raw bytes and file offsets. Higher-level
//! structure (slots, records) is layered on top by `super::slotted_page`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use crate::Result;

/// Size of every page in bytes (4 KiB, matching the Phase 1 spec).
pub const PAGE_SIZE: usize = 4096;

/// Identifies a page within a database file. Pages are numbered from 0
/// (unlike SQLite, which starts at 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageId(pub u32);

/// A single fixed-size page of raw bytes.
pub struct Page {
    /// The page's position in the file (see [`PageId`]).
    pub id: PageId,
    /// The raw page contents. Allocated on the heap to avoid blowing the stack.
    data: Box<[u8; PAGE_SIZE]>,
}

impl Page {
    /// Creates a new, zero-filled page with the given id.
    pub fn new(id: PageId) -> Self {
        Self { id, data: Box::new([0u8; PAGE_SIZE]) }
    }

    /// Returns the raw page bytes (read-only).
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..]
    }

    /// Returns the raw page bytes (mutable).
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data[..]
    }

    /// Reads the page with the given id from `file`.
    ///
    /// The page is located at byte offset `id * PAGE_SIZE`. Reading a page
    /// beyond the end of the file fails with an I/O error.
    pub fn read_from(file: &mut File, id: PageId) -> Result<Self> {
        let mut page = Self::new(id);
        file.seek(SeekFrom::Start(byte_offset(id)))?;
        file.read_exact(page.as_bytes_mut())?;
        Ok(page)
    }

    /// Writes this page to `file` at byte offset `id * PAGE_SIZE`.
    ///
    /// Pages are independent: writing one page never disturbs its neighbours,
    /// which is exactly what a database needs for in-place updates.
    pub fn write_to(&self, file: &mut File) -> Result<()> {
        file.seek(SeekFrom::Start(byte_offset(self.id)))?;
        file.write_all(self.as_bytes())?;
        file.flush()?;
        Ok(())
    }
}

/// Computes the byte offset of a page inside the database file.
fn byte_offset(id: PageId) -> u64 {
    id.0 as u64 * PAGE_SIZE as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_is_multiple_of_page_size() {
        assert_eq!(byte_offset(PageId(0)), 0);
        assert_eq!(byte_offset(PageId(3)), 3 * PAGE_SIZE as u64);
    }
}
