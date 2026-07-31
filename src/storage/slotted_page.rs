//! Slotted-page layout (Phase 1, step 2).
//!
//! Each 4 KiB page is organized as follows (mirroring the Phase 1 spec):
//!
//! ```text
//! +------------------+  offset 0
//! | Page Header      |  magic, page_id, num_slots, free_start, free_end
//! +------------------+
//! | Slot Directory   |  one u16 offset per record (grows downward)
//! | slot[0]          |
//! | slot[1]          |
//! | ...              |
//! +------------------+  <- free_start
//! |   Free Space     |
//! +------------------+  <- free_end
//! | Record N         |  length-prefixed records (grow upward)
//! | ...              |
//! | Record 0         |
//! +------------------+  offset 4096
//! ```
//!
//! Free space management is the core database concept here: the free region
//! sits between the slot directory (`free_start`) and the record region
//! (`free_end`). A record only fits when
//! `free_end - free_start >= record_len + 2 (prefix) + 2 (slot)`.
//!
//! All multi-byte integers are stored big-endian.

use super::page::{Page, PageId, PAGE_SIZE};
use crate::error::Error;
use crate::Result;

/// Identifies a slot within a page.
pub type SlotId = u16;

/// Magic number written at the start of every slotted page. Used to detect
/// corruption and to tell initialized pages apart from zeroed ones.
pub const MAGIC: u32 = 0xBA5A_5D70;

/// Fixed header size in bytes.
const HEADER_SIZE: usize = 16;
/// Every slot is a single u16 offset.
const SLOT_SIZE: usize = 2;
/// Bytes reserved in front of each record for its length.
const LENGTH_PREFIX_SIZE: usize = 2;
/// A slot holding this offset is considered deleted.
const DELETED: u16 = 0;

const MAGIC_OFFSET: usize = 0;
const PAGE_ID_OFFSET: usize = 4;
const NUM_SLOTS_OFFSET: usize = 8;
const FREE_START_OFFSET: usize = 10;
const FREE_END_OFFSET: usize = 12;

/// A [`Page`] interpreted with a slotted-page layout.
pub struct SlottedPage {
    page: Page,
}

impl SlottedPage {
    /// Wraps a (zeroed) page and initializes its header.
    ///
    /// The page is expected to be freshly created or blank; existing contents
    /// are overwritten.
    pub fn new(page: Page) -> Self {
        let mut sp = SlottedPage { page };
        sp.write_u32(MAGIC_OFFSET, MAGIC);
        sp.write_u32(PAGE_ID_OFFSET, sp.page.id.0);
        sp.write_u16(NUM_SLOTS_OFFSET, 0);
        sp.write_u16(FREE_START_OFFSET, HEADER_SIZE as u16);
        sp.write_u16(FREE_END_OFFSET, PAGE_SIZE as u16);
        sp
    }

    /// Interprets an existing page as a slotted page, validating its header.
    pub fn from_page(page: Page) -> Result<Self> {
        let sp = SlottedPage { page };
        let magic = sp.read_u32(MAGIC_OFFSET);
        if magic != MAGIC {
            return Err(Error::Corrupt(format!(
                "page {}: bad magic 0x{magic:08x}",
                sp.page.id.0
            )));
        }
        let header_page_id = sp.read_u32(PAGE_ID_OFFSET);
        if header_page_id != sp.page.id.0 {
            return Err(Error::Corrupt(format!(
                "page {}: header claims page {header_page_id}",
                sp.page.id.0
            )));
        }
        let num_slots = sp.read_u16(NUM_SLOTS_OFFSET) as usize;
        let free_start = sp.read_u16(FREE_START_OFFSET) as usize;
        let free_end = sp.read_u16(FREE_END_OFFSET) as usize;
        if !(HEADER_SIZE..=PAGE_SIZE).contains(&free_start) {
            return Err(Error::Corrupt(format!(
                "page {}: free_start {free_start} out of range",
                sp.page.id.0
            )));
        }
        if !(free_start..=PAGE_SIZE).contains(&free_end) {
            return Err(Error::Corrupt(format!(
                "page {}: free_end {free_end} out of range",
                sp.page.id.0
            )));
        }
        if num_slots * SLOT_SIZE > free_start - HEADER_SIZE {
            return Err(Error::Corrupt(format!(
                "page {}: slot directory overlaps header",
                sp.page.id.0
            )));
        }
        Ok(sp)
    }

    /// Returns `true` if the page is all zeros (never written to disk).
    pub fn is_empty_page(page: &Page) -> bool {
        page.as_bytes()[MAGIC_OFFSET..MAGIC_OFFSET + 4] == [0u8; 4]
    }

    /// Borrows the underlying raw page.
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// Consumes the wrapper, returning the raw page (for persistence).
    pub fn into_page(self) -> Page {
        self.page
    }

    /// The id of the underlying page.
    pub fn page_id(&self) -> PageId {
        self.page.id
    }

    /// Number of slots currently in the directory (including deleted ones).
    pub fn num_slots(&self) -> u16 {
        self.read_u16(NUM_SLOTS_OFFSET)
    }

    /// Bytes currently available for new records.
    pub fn free_space(&self) -> usize {
        let start = self.read_u16(FREE_START_OFFSET) as usize;
        let end = self.read_u16(FREE_END_OFFSET) as usize;
        end - start
    }

    /// Whether a record of `data_len` bytes fits without a page-full error.
    pub fn has_space_for(&self, data_len: usize) -> bool {
        self.free_space() >= data_len + LENGTH_PREFIX_SIZE + SLOT_SIZE
    }

    /// Inserts `data` and returns the id of the new slot.
    pub fn insert_record(&mut self, data: &[u8]) -> Result<SlotId> {
        let needed = data.len() + LENGTH_PREFIX_SIZE + SLOT_SIZE;
        let free = self.free_space();
        if needed > free {
            return Err(Error::PageFull { needed, free });
        }

        let free_end = self.read_u16(FREE_END_OFFSET) as usize;
        let record_start = free_end - data.len() - LENGTH_PREFIX_SIZE;

        // Write the length-prefixed record into the free space.
        self.write_u16(record_start, data.len() as u16);
        let data_start = record_start + LENGTH_PREFIX_SIZE;
        self.page.as_bytes_mut()[data_start..data_start + data.len()].copy_from_slice(data);

        // Append a new slot to the directory.
        let slot_index = self.read_u16(NUM_SLOTS_OFFSET);
        let slot_pos = HEADER_SIZE + slot_index as usize * SLOT_SIZE;
        self.write_u16(slot_pos, record_start as u16);

        self.write_u16(NUM_SLOTS_OFFSET, slot_index + 1);
        self.write_u16(FREE_END_OFFSET, record_start as u16);
        self.write_u16(FREE_START_OFFSET, (slot_pos + SLOT_SIZE) as u16);

        Ok(slot_index)
    }

    /// Returns the record stored in `slot`.
    ///
    /// Fails if the slot is out of range or was deleted.
    pub fn get_record(&self, slot: SlotId) -> Result<&[u8]> {
        if slot >= self.num_slots() {
            return Err(Error::InvalidSlot(slot));
        }
        let slot_pos = HEADER_SIZE + slot as usize * SLOT_SIZE;
        let record_start = self.read_u16(slot_pos) as usize;
        if record_start == DELETED as usize {
            return Err(Error::InvalidSlot(slot));
        }
        if record_start < HEADER_SIZE || record_start + LENGTH_PREFIX_SIZE > PAGE_SIZE {
            return Err(Error::Corrupt(format!(
                "page {}: slot {slot} points outside the page",
                self.page.id.0
            )));
        }
        let len = self.read_u16(record_start) as usize;
        let end = record_start + LENGTH_PREFIX_SIZE + len;
        if end > PAGE_SIZE {
            return Err(Error::Corrupt(format!(
                "page {}: slot {slot} length out of bounds",
                self.page.id.0
            )));
        }
        Ok(&self.page.as_bytes()[record_start + LENGTH_PREFIX_SIZE..end])
    }

    /// Returns `true` if `slot` exists but has been deleted.
    pub fn is_deleted(&self, slot: SlotId) -> bool {
        if slot >= self.num_slots() {
            return false;
        }
        let slot_pos = HEADER_SIZE + slot as usize * SLOT_SIZE;
        self.read_u16(slot_pos) == DELETED
    }

    /// Marks `slot` as deleted. The space is not reclaimed (Phase 1
    /// simplification); a future compaction pass can reuse it.
    pub fn delete_record(&mut self, slot: SlotId) -> Result<()> {
        if slot >= self.num_slots() {
            return Err(Error::InvalidSlot(slot));
        }
        let slot_pos = HEADER_SIZE + slot as usize * SLOT_SIZE;
        self.write_u16(slot_pos, DELETED);
        Ok(())
    }

    fn read_u16(&self, offset: usize) -> u16 {
        let bytes = self.page.as_bytes();
        u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn read_u32(&self, offset: usize) -> u32 {
        let bytes = self.page.as_bytes();
        u32::from_be_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
    }

    fn write_u16(&mut self, offset: usize, value: u16) {
        let bytes = value.to_be_bytes();
        let data = self.page.as_bytes_mut();
        data[offset] = bytes[0];
        data[offset + 1] = bytes[1];
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        let bytes = value.to_be_bytes();
        let data = self.page.as_bytes_mut();
        for (i, byte) in bytes.iter().enumerate() {
            data[offset + i] = *byte;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_page() -> SlottedPage {
        SlottedPage::new(Page::new(PageId(0)))
    }

    #[test]
    fn empty_page_starts_with_zero_slots_and_full_free_space() {
        let sp = new_page();
        assert_eq!(sp.num_slots(), 0);
        assert_eq!(sp.free_space(), PAGE_SIZE - HEADER_SIZE);
    }

    #[test]
    fn inserting_reclaims_free_space_exactly() {
        let mut sp = new_page();
        let free_before = sp.free_space();
        sp.insert_record(b"four").unwrap();
        // 4 data bytes + 2 prefix + 2 slot.
        assert_eq!(sp.free_space(), free_before - 8);
    }

    #[test]
    fn records_do_not_overlap() {
        let mut sp = new_page();
        let a = sp.insert_record(b"aaaa").unwrap();
        let b = sp.insert_record(b"bbbbbb").unwrap();
        assert_eq!(sp.get_record(a).unwrap(), b"aaaa");
        assert_eq!(sp.get_record(b).unwrap(), b"bbbbbb");
    }

    #[test]
    fn deleted_slot_does_not_affect_others() {
        let mut sp = new_page();
        let a = sp.insert_record(b"keep").unwrap();
        let b = sp.insert_record(b"drop").unwrap();
        sp.delete_record(b).unwrap();
        assert!(sp.is_deleted(b));
        assert!(!sp.is_deleted(a));
        assert_eq!(sp.get_record(a).unwrap(), b"keep");
    }

    #[test]
    fn corrupted_magic_is_detected() {
        let mut page = Page::new(PageId(0));
        page.as_bytes_mut().fill(0xAB);
        assert!(SlottedPage::from_page(page).is_err());
    }

    #[test]
    fn round_trip_through_page_bytes() {
        let mut sp = new_page();
        sp.insert_record(b"first").unwrap();
        sp.insert_record(b"second record").unwrap();

        let page = sp.into_page();
        let reloaded = SlottedPage::from_page(page).unwrap();
        assert_eq!(reloaded.num_slots(), 2);
        assert_eq!(reloaded.get_record(1).unwrap(), b"second record");
    }
}
