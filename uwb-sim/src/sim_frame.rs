use uwb::{
    error::{ProtocolError, UwbError},
    frame::{AddressMode, MacAddress, MacFrameControl},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimMacFrame {
    pub frame_control: MacFrameControl,
    pub sequence_number: u8,
    pub dest_pan_id: u16,
    pub dest_addr: MacAddress,
    pub src_addr: MacAddress,
    pub payload: Vec<u8>,
}

impl SimMacFrame {
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

        let mut payload = Vec::new();
        payload.extend_from_slice(&bytes[offset..]);

        Ok(Self {
            frame_control,
            sequence_number,
            dest_pan_id,
            dest_addr,
            src_addr,
            payload,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, UwbError> {
        let mut buf = Vec::new();

        let fcf_raw = self.frame_control.to_u16();
        buf.extend_from_slice(&fcf_raw.to_le_bytes());

        buf.push(self.sequence_number);

        buf.extend_from_slice(&self.dest_pan_id.to_le_bytes());

        match self.dest_addr {
            MacAddress::None => {}
            MacAddress::Short(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes());
            }
            MacAddress::Extended(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes());
            }
        }

        match self.src_addr {
            MacAddress::None => {}
            MacAddress::Short(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes());
            }
            MacAddress::Extended(addr) => {
                buf.extend_from_slice(&addr.to_le_bytes());
            }
        }

        buf.extend_from_slice(&self.payload);

        Ok(buf)
    }
}
