#![allow(dead_code, unused)]

use crate::config::{RxConfig, UwbConfig};
use crate::error::UwbError;
use crate::frame::MacFrame;
use crate::time::{UWBDuration, UWBTimestamp};

/// Information returned after successful transmission
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxInfo {
    pub tx_timestamp: UWBTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RxFrame<const N: usize> {
    /// Raw received frame bytes
    pub data: heapless::Vec<u8, N>,

    /// Timestamp of the received frame
    pub rx_timestamp: UWBTimestamp,
    // later maybe some quality information
}

impl RxFrame<{ UwbConfig::MAX_STANDARD_FRAME_SIZE }> {
    /// Constructor for standard frame
    pub fn new_standard(rx_timestamp: UWBTimestamp) -> Self {
        Self {
            data: heapless::Vec::new(),
            rx_timestamp,
        }
    }
}

impl RxFrame<{ UwbConfig::MAX_EXTENDED_FRAME_SIZE }> {
    /// Constructor for extended frame
    pub fn new_extended(rx_timestamp: UWBTimestamp) -> Self {
        Self {
            data: heapless::Vec::new(),
            rx_timestamp,
        }
    }
}

/// Main UWB HAL trait
pub trait UwbHal<const MAX_FRAME_SIZE: usize> {
    /// configure Uwb
    async fn configure(&mut self, config: UwbConfig) -> Result<(), UwbError>;

    /// Returns current UWB timestamp
    async fn now(&mut self) -> Result<UWBTimestamp, UwbError>;

    /// Transmit a frame immediately
    async fn transmit(&mut self, message: &[u8]) -> Result<TxInfo, UwbError>;

    /// Transmit a frame at an exact future UWB timestamp
    async fn transmit_at(
        &mut self,
        data: &[u8],
        timestamp: UWBTimestamp,
    ) -> Result<TxInfo, UwbError>;

    /// Wait for one received frame
    async fn receive(&mut self, rx_config: RxConfig) -> Result<RxFrame<MAX_FRAME_SIZE>, UwbError>;

    /// Abort current TX/RX operation and return to idle
    async fn stop(&mut self) -> Result<(), UwbError>;
}
