pub const PAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageId(pub u32);

pub struct Page {
    pub id: PageId,
    pub data: Box<[u8; PAGE_SIZE]>,
}

impl Page {
    pub fn new(id: PageId) -> Self {
        Self { id, data: Box::new([0u8; PAGE_SIZE]) }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn as_bytes_mut(&self) -> &mut [u8] {
        &mut self.data[..]
    }
}