//! Row (tuple) serialization (Phase 1, step 4).
//!
//! A row is a vector of [`Value`]s. It is serialized into a single,
//! self-describing byte buffer using a length-prefixed format:
//!
//! ```text
//! [u32 total_len][u16 num_values][null bitmap][value...]
//! ```
//!
//! - `total_len` is the byte length of the whole buffer.
//! - The null bitmap uses one bit per value (LSB first); a set bit means NULL.
//! - Every non-NULL value is a type tag followed by its payload.
//! - All multi-byte integers are big-endian, matching the Phase 1 spec.
//!
//! Being self-describing means a row can be decoded without the table schema,
//! which keeps the storage layer independent of the catalog.

use crate::error::Error;
use crate::Result;

/// A single column value of a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// 64-bit signed integer.
    Integer(i64),
    /// UTF-8 text.
    Varchar(String),
    /// Boolean flag.
    Boolean(bool),
    /// SQL NULL.
    Null,
}

const TAG_INTEGER: u8 = 0x01;
const TAG_VARCHAR: u8 = 0x02;
const TAG_BOOLEAN: u8 = 0x03;

const LENGTH_HEADER: usize = 4 + 2;

/// Serializes `values` into the length-prefixed row format.
pub fn serialize(values: &[Value]) -> Vec<u8> {
    let bitmap_len = values.len().div_ceil(8);

    let mut bitmap = vec![0u8; bitmap_len];
    let mut payload = Vec::new();
    for (i, value) in values.iter().enumerate() {
        match value {
            Value::Null => {
                bitmap[i / 8] |= 1u8 << (i % 8);
            }
            Value::Integer(n) => {
                payload.push(TAG_INTEGER);
                payload.extend_from_slice(&n.to_be_bytes());
            }
            Value::Varchar(s) => {
                payload.push(TAG_VARCHAR);
                payload.extend_from_slice(&(s.len() as u32).to_be_bytes());
                payload.extend_from_slice(s.as_bytes());
            }
            Value::Boolean(b) => {
                payload.push(TAG_BOOLEAN);
                payload.push(u8::from(*b));
            }
        }
    }

    let total_len = LENGTH_HEADER + bitmap_len + payload.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&(total_len as u32).to_be_bytes());
    out.extend_from_slice(&(values.len() as u16).to_be_bytes());
    out.extend_from_slice(&bitmap);
    out.extend_from_slice(&payload);
    out
}

/// Decodes a buffer produced by [`serialize`] back into values.
pub fn deserialize(bytes: &[u8]) -> Result<Vec<Value>> {
    if bytes.len() < LENGTH_HEADER {
        return Err(Error::Deserialize("row shorter than header".into()));
    }

    let total_len = read_u32(bytes, 0) as usize;
    if total_len != bytes.len() {
        return Err(Error::Deserialize(format!(
            "length mismatch: header says {total_len}, buffer has {}",
            bytes.len()
        )));
    }

    let num_values = read_u16(bytes, 4) as usize;
    let bitmap_len = num_values.div_ceil(8);
    let mut pos = LENGTH_HEADER + bitmap_len;
    if pos > bytes.len() {
        return Err(Error::Deserialize("truncated null bitmap".into()));
    }
    let bitmap = &bytes[LENGTH_HEADER..pos];

    let mut values = Vec::with_capacity(num_values);
    for i in 0..num_values {
        let is_null = bitmap[i / 8] & (1u8 << (i % 8)) != 0;
        if is_null {
            values.push(Value::Null);
            continue;
        }

        let tag = take(bytes, &mut pos, 1)?[0];
        match tag {
            TAG_INTEGER => {
                let raw = take(bytes, &mut pos, 8)?;
                let n = i64::from_be_bytes(
                    raw.try_into().map_err(|_| {
                        Error::Deserialize("bad integer length".into())
                    })?,
                );
                values.push(Value::Integer(n));
            }
            TAG_VARCHAR => {
                let raw_len = take(bytes, &mut pos, 4)?;
                let len = u32::from_be_bytes(
                    raw_len
                        .try_into()
                        .map_err(|_| Error::Deserialize("bad varchar length".into()))?,
                ) as usize;
                let raw = take(bytes, &mut pos, len)?;
                let s = String::from_utf8(raw.to_vec())
                    .map_err(|e| Error::Deserialize(format!("invalid UTF-8: {e}")))?;
                values.push(Value::Varchar(s));
            }
            TAG_BOOLEAN => {
                let raw = take(bytes, &mut pos, 1)?[0];
                values.push(Value::Boolean(raw != 0));
            }
            other => return Err(Error::Deserialize(format!("unknown tag {other}"))),
        }
    }
    Ok(values)
}

/// Returns a slice of `n` bytes starting at `pos`, advancing `pos`.
fn take<'a>(bytes: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = pos
        .checked_add(n)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Deserialize("truncated row".into()))?;
    let slice = &bytes[*pos..end];
    *pos = end;
    Ok(slice)
}

fn read_u16(bytes: &[u8], pos: usize) -> u16 {
    u16::from_be_bytes([bytes[pos], bytes[pos + 1]])
}

fn read_u32(bytes: &[u8], pos: usize) -> u32 {
    u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_bitmap_flags_exact_null_positions() {
        // Bitmap length for 9 values is 2 bytes; bits 2 and 8 (0-indexed) set.
        let row = vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Null,
            Value::Integer(4),
            Value::Integer(5),
            Value::Integer(6),
            Value::Integer(7),
            Value::Integer(8),
            Value::Null,
        ];
        let bytes = serialize(&row);
        assert_eq!(bytes[4 + 2], 0b0000_0100);
        assert_eq!(bytes[4 + 2 + 1], 0b0000_0001);
    }

    #[test]
    fn round_trip_preserves_value_count() {
        let row = vec![Value::Null, Value::Boolean(true), Value::Varchar("x".into())];
        assert_eq!(deserialize(&serialize(&row)).unwrap().len(), 3);
    }
}
