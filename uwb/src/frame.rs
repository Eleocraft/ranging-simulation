use crate::config::UwbConfig;
use crate::define_payload;
use crate::error::{ProtocolError, UwbError};
use crate::time::UWBTimestamp;

// =======================================================================================================
// Payload
// =======================================================================================================

define_payload! {
    Ping     = 0x01,
    Poll     = 0x02,
    Response = 0x03,
    Final    = 0x04, {
        poll_tx_time: UWBTimestamp,
        response_rx_time: UWBTimestamp,
        final_tx_time: UWBTimestamp,
    },
}

// =======================================================================================================
// MacFrameControl
// =======================================================================================================

/// Represents the 16-bit Frame Control Field (FCF) using the newer IEEE 802.15.4-2011 with pan-id
/// compression because both sender and receiver are in the same network
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacFrameControl {
    /// Defines the function of the packet
    pub frame_type: MacFrameType,
    /// Enables Link-Layer encryption
    pub security_enabled: bool,
    /// Indicates if the sender has more data pending
    pub frame_pending: bool,
    /// Requests an immediate hardware ack-frame from recipient
    pub ack_request: bool,
    /// Specifies the format and length of the destination address
    pub dst_addr_mode: AddressMode,
    /// Specifies the format and length of the source address
    pub src_addr_mode: AddressMode,
}

impl Default for MacFrameControl {
    fn default() -> Self {
        Self {
            frame_type: MacFrameType::Data,
            security_enabled: false,
            frame_pending: false,
            ack_request: false,
            dst_addr_mode: AddressMode::None,
            src_addr_mode: AddressMode::None,
        }
    }
}

/// Identifies the MAC frame type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MacFrameType {
    Beacon = 0b000,
    Data = 0b001,
    Ack = 0b010,
    MacCom = 0b011,
}

/// Defines the addressing modes for source and destination endpoints
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AddressMode {
    /// Address field is omitted entirely
    None = 0b00,
    /// 16-bit short address format
    Short = 0b10,
    /// 64-bit extended unique address format
    Extended = 0b11,
}

impl MacFrameControl {
    pub fn new(
        frame_type: MacFrameType,
        security_enabled: bool,
        frame_pending: bool,
        ack_request: bool,
        dst_addr_mode: AddressMode,
        src_addr_mode: AddressMode,
    ) -> Self {
        Self {
            frame_type,
            security_enabled,
            frame_pending,
            ack_request,
            dst_addr_mode,
            src_addr_mode,
        }
    }

    pub fn to_u16(&self) -> u16 {
        let mut bits = 0u16;

        bits |= self.frame_type as u16;
        bits |= (self.security_enabled as u16) << 3;
        bits |= (self.frame_pending as u16) << 4;
        bits |= (self.ack_request as u16) << 5;
        bits |= 0b1 << 6;
        bits |= (self.dst_addr_mode as u16) << 10;
        bits |= 0b01 << 12; // 
        bits |= (self.src_addr_mode as u16) << 14;

        bits
    }

    pub fn from_u16(v: u16) -> Option<Self> {
        let frame_type = match v & 0b111 {
            0b000 => MacFrameType::Beacon,
            0b001 => MacFrameType::Data,
            0b010 => MacFrameType::Ack,
            0b011 => MacFrameType::MacCom,
            _ => return None,
        };

        let dst_addr_mode = match (v >> 10) & 0b11 {
            0b00 => AddressMode::None,
            0b10 => AddressMode::Short,
            0b11 => AddressMode::Extended,
            _ => return None,
        };

        let src_addr_mode = match (v >> 14) & 0b11 {
            0b00 => AddressMode::None,
            0b10 => AddressMode::Short,
            0b11 => AddressMode::Extended,
            _ => return None,
        };

        Some(Self {
            frame_type,
            security_enabled: ((v >> 3) & 1) != 0,
            frame_pending: ((v >> 4) & 1) != 0,
            ack_request: ((v >> 5) & 1) != 0,
            dst_addr_mode,
            src_addr_mode,
        })
    }
}

// =======================================================================================================
// MacFrame
// =======================================================================================================

pub struct MacFrame<const N: usize> {
    pub frame_control: MacFrameControl,
    pub sequence_number: u8,
    pub dest_pan_id: u16,
    pub dest_addr: MacAddress,
    pub src_addr: MacAddress,
    pub payload: heapless::Vec<u8, N>,
}

impl<const N: usize> MacFrame<N> {
    /// Encodes MAC Frame into a flat byte buffer
    pub fn to_bytes(&self) -> Result<heapless::Vec<u8, N>, UwbError> {
        let mut buf = heapless::Vec::new();

        let fcf_raw = self.frame_control.to_u16();
        buf.extend_from_slice(&fcf_raw.to_le_bytes())
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;

        buf.push(self.sequence_number)
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;

        buf.extend_from_slice(&self.dest_pan_id.to_le_bytes())
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;

        match self.dest_addr {
            MacAddress::None => {}
            MacAddress::Short(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes())
                    .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;
            }
            MacAddress::Extended(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes())
                    .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;
            }
        }

        match self.src_addr {
            MacAddress::None => {}
            MacAddress::Short(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes())
                    .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;
            }
            MacAddress::Extended(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes())
                    .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;
            }
        }

        buf.extend_from_slice(&self.payload)
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;

        Ok(buf)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, UwbError> {
        if bytes.len() < 5 {
            return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
        }

        let mut offset = 0;

        let fcf_raw = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let frame_control = MacFrameControl::from_u16(fcf_raw)
            .ok_or(UwbError::Protocol(ProtocolError::InvalidFrameType))?;
        offset += 2;

        let sequence_number = bytes[offset];
        offset += 1;

        let dest_pan_id = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        let dest_addr = match frame_control.dst_addr_mode {
            AddressMode::None => MacAddress::None,
            AddressMode::Short => {
                if bytes.len() < offset + 2 {
                    return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
                }
                let addr = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                offset += 2;
                MacAddress::Short(addr)
            }
            AddressMode::Extended => {
                if bytes.len() < offset + 8 {
                    return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
                }
                let mut tmp = [0u8; 8];
                tmp.copy_from_slice(&bytes[offset..offset + 8]);
                offset += 8;
                MacAddress::Extended(u64::from_le_bytes(tmp))
            }
        };

        let src_addr = match frame_control.src_addr_mode {
            AddressMode::None => MacAddress::None,
            AddressMode::Short => {
                if bytes.len() < offset + 2 {
                    return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
                }
                let addr = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                offset += 2;
                MacAddress::Short(addr)
            }
            AddressMode::Extended => {
                if bytes.len() < offset + 8 {
                    return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
                }
                let mut tmp = [0u8; 8];
                tmp.copy_from_slice(&bytes[offset..offset + 8]);
                offset += 8;
                MacAddress::Extended(u64::from_le_bytes(tmp))
            }
        };

        if bytes.len() < offset {
            return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
        }

        let mut payload = heapless::Vec::new();
        payload
            .extend_from_slice(&bytes[offset..])
            .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;

        Ok(Self {
            frame_control,
            sequence_number,
            dest_pan_id,
            dest_addr,
            src_addr,
            payload,
        })
    }

    /// Constructor for standard 16-bit short address
    pub fn new_short(
        fcf: FrameControlSource,
        sequence_number: u8,
        dest_pan_id: u16,
        dest_addr: Option<u16>,
        src_addr: Option<u16>,
        payload: heapless::Vec<u8, N>,
    ) -> Result<Self, UwbError> {
        let dest = dest_addr.map(MacAddress::Short).unwrap_or(MacAddress::None);
        let src = src_addr.map(MacAddress::Short).unwrap_or(MacAddress::None);

        let mut frame_control = match fcf {
            FrameControlSource::Full(custom_fcf) => custom_fcf,
            FrameControlSource::Preset {
                frame_type,
                security_enabled,
                frame_pending,
                ack_request,
            } => MacFrameControl {
                frame_type,
                security_enabled,
                frame_pending,
                ack_request,
                dst_addr_mode: AddressMode::None,
                src_addr_mode: AddressMode::None,
            },
        };

        frame_control.src_addr_mode = src.mode();
        frame_control.dst_addr_mode = dest.mode();

        let mut header_len = 2 + 1 + 2;
        header_len += match dest {
            MacAddress::Short(_) => 2,
            _ => 0,
        };
        header_len += match src {
            MacAddress::Short(_) => 2,
            _ => 0,
        };

        if header_len + payload.len() > N {
            return Err(UwbError::Protocol(ProtocolError::InvalidPayload));
        }

        Ok(Self {
            frame_control,
            sequence_number,
            dest_pan_id,
            dest_addr: dest,
            src_addr: src,
            payload,
        })
    }

    /// Constructor for 64-bit extended address
    pub fn new_extended(
        fcf: FrameControlSource,
        sequence_number: u8,
        dest_pan_id: u16,
        dest_addr: Option<u64>,
        src_addr: Option<u64>,
        payload: heapless::Vec<u8, N>,
    ) -> Result<Self, UwbError> {
        let dest = dest_addr
            .map(MacAddress::Extended)
            .unwrap_or(MacAddress::None);
        let src = src_addr
            .map(MacAddress::Extended)
            .unwrap_or(MacAddress::None);

        let mut frame_control = match fcf {
            FrameControlSource::Full(custom_fcf) => custom_fcf,
            FrameControlSource::Preset {
                frame_type,
                security_enabled,
                frame_pending,
                ack_request,
            } => MacFrameControl {
                frame_type,
                security_enabled,
                frame_pending,
                ack_request,
                dst_addr_mode: AddressMode::None,
                src_addr_mode: AddressMode::None,
            },
        };

        frame_control.src_addr_mode = src.mode();
        frame_control.dst_addr_mode = dest.mode();

        let mut header_len = 2 + 1 + 2;
        header_len += match dest {
            MacAddress::Extended(_) => 8,
            _ => 0,
        };
        header_len += match src {
            MacAddress::Extended(_) => 8,
            _ => 0,
        };

        if header_len + payload.len() > N {
            return Err(UwbError::Protocol(ProtocolError::InvalidPayload));
        }

        Ok(Self {
            frame_control,
            sequence_number,
            dest_pan_id,
            dest_addr: dest,
            src_addr: src,
            payload,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameControlSource {
    Full(MacFrameControl),
    Preset {
        frame_type: MacFrameType,
        security_enabled: bool,
        frame_pending: bool,
        ack_request: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacAddress {
    None,
    Short(u16),
    Extended(u64),
}

impl MacAddress {
    /// Returns corresponding address mode
    pub fn mode(&self) -> AddressMode {
        match self {
            MacAddress::None => AddressMode::None,
            MacAddress::Short(_) => AddressMode::Short,
            MacAddress::Extended(_) => AddressMode::Extended,
        }
    }
}
