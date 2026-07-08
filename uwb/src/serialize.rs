use crate::error::*;
use crate::frame::{MacAddress, MacFrameBuilder, MacFrameControl};
use crate::time::UWBTimestamp;

pub trait UwbSerialize: Sized {
    fn serialize<const N: usize>(&self, buf: &mut heapless::Vec<u8, N>) -> Result<(), UwbError>;
    fn deserialize(bytes: &[u8], offset: &mut usize) -> Result<Self, UwbError>;
}

impl UwbSerialize for UWBTimestamp {
    fn serialize<const N: usize>(&self, buf: &mut heapless::Vec<u8, N>) -> Result<(), UwbError> {
        let bytes = self.ticks.to_le_bytes();
        buf.extend_from_slice(&bytes[0..5])
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))
    }

    fn deserialize(bytes: &[u8], offset: &mut usize) -> Result<Self, UwbError> {
        if bytes.len() < *offset + 5 {
            return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
        }
        let mut tmp = [0u8; 8];

        tmp[0..5].copy_from_slice(&bytes[*offset..*offset + 5]);
        let ticks = u64::from_le_bytes(tmp);

        *offset += 5;

        Ok(UWBTimestamp::from_ticks(ticks))
    }
}

impl UwbSerialize for u8 {
    fn serialize<const N: usize>(&self, buf: &mut heapless::Vec<u8, N>) -> Result<(), UwbError> {
        buf.push(*self)
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))
    }

    fn deserialize(bytes: &[u8], offset: &mut usize) -> Result<Self, UwbError> {
        if bytes.len() < *offset + 1 {
            return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
        }
        let val = bytes[*offset];
        *offset += 1;
        Ok(val)
    }
}

impl UwbSerialize for u16 {
    fn serialize<const N: usize>(&self, buf: &mut heapless::Vec<u8, N>) -> Result<(), UwbError> {
        let bytes = self.to_le_bytes();
        buf.extend_from_slice(&bytes)
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))
    }

    fn deserialize(bytes: &[u8], offset: &mut usize) -> Result<Self, UwbError> {
        if bytes.len() < *offset + 2 {
            return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
        }

        let mut tmp = [0u8; 2];
        tmp.copy_from_slice(&bytes[*offset..*offset + 2]);
        *offset += 2;
        Ok(u16::from_le_bytes(tmp))
    }
}
