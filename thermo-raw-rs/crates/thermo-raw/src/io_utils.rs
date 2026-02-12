//! Binary reading utilities for parsing Thermo RAW structures.

use crate::RawError;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// A cursor wrapper for reading binary data from a byte slice.
pub struct BinaryReader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> BinaryReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    /// Create a reader starting at a specific offset.
    pub fn at_offset(data: &'a [u8], offset: u64) -> Self {
        let mut cursor = Cursor::new(data);
        cursor.set_position(offset);
        Self { cursor }
    }

    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    pub fn set_position(&mut self, pos: u64) {
        self.cursor.set_position(pos);
    }

    pub fn remaining(&self) -> usize {
        let pos = self.cursor.position() as usize;
        let len = self.cursor.get_ref().len();
        if pos >= len {
            0
        } else {
            len - pos
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, RawError> {
        self.cursor.read_u8().map_err(RawError::Io)
    }

    pub fn read_u16(&mut self) -> Result<u16, RawError> {
        self.cursor.read_u16::<LittleEndian>().map_err(RawError::Io)
    }

    pub fn read_u32(&mut self) -> Result<u32, RawError> {
        self.cursor.read_u32::<LittleEndian>().map_err(RawError::Io)
    }

    pub fn read_i32(&mut self) -> Result<i32, RawError> {
        self.cursor.read_i32::<LittleEndian>().map_err(RawError::Io)
    }

    pub fn read_u64(&mut self) -> Result<u64, RawError> {
        self.cursor.read_u64::<LittleEndian>().map_err(RawError::Io)
    }

    pub fn read_f32(&mut self) -> Result<f32, RawError> {
        self.cursor.read_f32::<LittleEndian>().map_err(RawError::Io)
    }

    pub fn read_f64(&mut self) -> Result<f64, RawError> {
        self.cursor.read_f64::<LittleEndian>().map_err(RawError::Io)
    }

    /// Read N bytes into a new Vec.
    pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, RawError> {
        let pos = self.cursor.position() as usize;
        let data = self.cursor.get_ref();
        if pos + n > data.len() {
            return Err(RawError::CorruptedData(format!(
                "read_bytes: tried to read {} bytes at offset {}, but only {} available",
                n,
                pos,
                data.len() - pos
            )));
        }
        let result = data[pos..pos + n].to_vec();
        self.cursor.set_position((pos + n) as u64);
        Ok(result)
    }

    /// Skip N bytes.
    pub fn skip(&mut self, n: usize) -> Result<(), RawError> {
        let new_pos = self.cursor.position() + n as u64;
        if new_pos > self.cursor.get_ref().len() as u64 {
            return Err(RawError::CorruptedData(format!(
                "skip: tried to skip to offset {}, but file is only {} bytes",
                new_pos,
                self.cursor.get_ref().len()
            )));
        }
        self.cursor.set_position(new_pos);
        Ok(())
    }

    /// Read a fixed-size UTF-16LE string (size in bytes, not chars).
    pub fn read_utf16_fixed(&mut self, byte_len: usize) -> Result<String, RawError> {
        let bytes = self.read_bytes(byte_len)?;
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&u16s)
            .trim_end_matches('\0')
            .to_string())
    }

    /// Read a PascalStringWin32: i32 length prefix, then length * 2 bytes of UTF-16LE.
    pub fn read_pascal_string(&mut self) -> Result<String, RawError> {
        let len = self.read_i32()?;
        if len < 0 {
            return Err(RawError::CorruptedData(format!(
                "PascalString with negative length: {}",
                len
            )));
        }
        if len == 0 {
            return Ok(String::new());
        }
        let byte_len = (len as usize) * 2;
        self.read_utf16_fixed(byte_len)
    }

    /// Read an array of f32 values.
    pub fn read_f32_array(&mut self, count: usize) -> Result<Vec<f32>, RawError> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            result.push(self.read_f32()?);
        }
        Ok(result)
    }

    /// Read an array of f64 values.
    pub fn read_f64_array(&mut self, count: usize) -> Result<Vec<f64>, RawError> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            result.push(self.read_f64()?);
        }
        Ok(result)
    }

    /// Get a slice of the underlying data at the current position.
    pub fn slice(&self, len: usize) -> Result<&'a [u8], RawError> {
        let pos = self.cursor.position() as usize;
        let data = self.cursor.get_ref();
        if pos + len > data.len() {
            return Err(RawError::CorruptedData(format!(
                "slice: requested {} bytes at {}, only {} available",
                len,
                pos,
                data.len() - pos
            )));
        }
        Ok(&data[pos..pos + len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_primitives() {
        let data: Vec<u8> = vec![
            0x01, 0xA1, // u16: 0xA101
            0x39, 0x00, 0x00, 0x00, // u32: 57
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x59, 0x40, // f64: 100.0
        ];
        let mut reader = BinaryReader::new(&data);
        assert_eq!(reader.read_u16().unwrap(), 0xA101);
        assert_eq!(reader.read_u32().unwrap(), 57);
        assert_eq!(reader.read_f64().unwrap(), 100.0);
    }

    #[test]
    fn test_read_pascal_string() {
        // PascalStringWin32: i32 length=3, then "abc" in UTF-16LE
        let data: Vec<u8> = vec![
            0x03, 0x00, 0x00, 0x00, // length: 3
            0x61, 0x00, // 'a'
            0x62, 0x00, // 'b'
            0x63, 0x00, // 'c'
        ];
        let mut reader = BinaryReader::new(&data);
        assert_eq!(reader.read_pascal_string().unwrap(), "abc");
    }

    #[test]
    fn test_read_utf16_fixed_with_nulls() {
        // "Hi" followed by null padding (8 bytes total = 4 UTF-16 chars)
        let data: Vec<u8> = vec![
            0x48, 0x00, // 'H'
            0x69, 0x00, // 'i'
            0x00, 0x00, // null
            0x00, 0x00, // null
        ];
        let mut reader = BinaryReader::new(&data);
        assert_eq!(reader.read_utf16_fixed(8).unwrap(), "Hi");
    }

    #[test]
    fn test_at_offset() {
        let data: Vec<u8> = vec![0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00];
        let mut reader = BinaryReader::at_offset(&data, 4);
        assert_eq!(reader.read_u32().unwrap(), 0x42);
    }

    #[test]
    fn test_skip_and_remaining() {
        let data: Vec<u8> = vec![0; 100];
        let mut reader = BinaryReader::new(&data);
        assert_eq!(reader.remaining(), 100);
        reader.skip(50).unwrap();
        assert_eq!(reader.remaining(), 50);
        assert_eq!(reader.position(), 50);
    }
}
