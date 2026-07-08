use crate::config::UwbConfig;
use crate::frame::{AddressMode, MacAddress, MacFrameControl};

// =======================================================================================================
// MacFrameBuilder
// =======================================================================================================
pub struct MacFrameFactory;

pub struct MacFrameBuilder<S, const N: usize> {
    pub(crate) state: S,
    pub(crate) sequence_number: u8,
    pub(crate) dest_pan_id: u16,
    pub(crate) dest_addr: MacAddress,
    pub(crate) src_addr: MacAddress,
    pub(crate) frame_control: MacFrameControl,
}

impl<S, const N: usize> MacFrameBuilder<S, N> {
    pub fn with_sequence(mut self, seq: u8) -> Self {
        self.sequence_number = seq;
        self
    }

    pub fn with_network(mut self, pan_id: u16, src: MacAddress, dest: MacAddress) -> Self {
        self.dest_pan_id = pan_id;
        self.src_addr = src;
        self.dest_addr = dest;
        self
    }

    pub fn with_src_address(mut self, src: MacAddress) -> Self {
        match src {
            MacAddress::None => self.frame_control.src_addr_mode = AddressMode::None,
            MacAddress::Short(_) => self.frame_control.src_addr_mode = AddressMode::Short,
            MacAddress::Extended(_) => self.frame_control.src_addr_mode = AddressMode::Extended,
        }
        self.src_addr = src;
        self
    }

    pub fn with_dest_address(mut self, dest: MacAddress) -> Self {
        match dest {
            MacAddress::None => self.frame_control.dst_addr_mode = AddressMode::None,
            MacAddress::Short(_) => self.frame_control.dst_addr_mode = AddressMode::Short,
            MacAddress::Extended(_) => self.frame_control.dst_addr_mode = AddressMode::Extended,
        }
        self.dest_addr = dest;
        self
    }
}

impl<S> MacFrameBuilder<S, { UwbConfig::MAX_STANDARD_FRAME_SIZE }> {
    pub fn with_extended_format(
        self,
    ) -> MacFrameBuilder<S, { UwbConfig::MAX_EXTENDED_FRAME_SIZE }> {
        MacFrameBuilder {
            state: self.state,
            sequence_number: self.sequence_number,
            dest_pan_id: self.dest_pan_id,
            dest_addr: self.dest_addr,
            src_addr: self.src_addr,
            frame_control: self.frame_control,
        }
    }
}

impl<S> MacFrameBuilder<S, { UwbConfig::MAX_EXTENDED_FRAME_SIZE }> {
    pub fn with_standard_format(
        self,
    ) -> MacFrameBuilder<S, { UwbConfig::MAX_STANDARD_FRAME_SIZE }> {
        MacFrameBuilder {
            state: self.state,
            sequence_number: self.sequence_number,
            dest_pan_id: self.dest_pan_id,
            dest_addr: self.dest_addr,
            src_addr: self.src_addr,
            frame_control: self.frame_control,
        }
    }
}
