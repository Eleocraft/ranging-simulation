#![allow(dead_code)]
use crate::time::UWBDuration;

/// Complete Configuration for a single UWB-Node
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UwbConfig {
    pub address: AddressConfig,
    pub phy: PhyConfig,
    pub mac: MacConfig,
    pub interrupt: InterruptConfig,
    pub antenna_delay: AntennaDelayConfig,
    pub tx_power: TxPowerConfig,
}

impl Default for UwbConfig {
    fn default() -> Self {
        Self {
            address: Default::default(),
            phy: Default::default(),
            mac: Default::default(),
            interrupt: Default::default(),
            antenna_delay: Default::default(),
            tx_power: Default::default(),
        }
    }
}

impl UwbConfig {
    pub const MAX_STANDARD_FRAME_SIZE: usize = 127;
    pub const MAX_EXTENDED_FRAME_SIZE: usize = 1023;
}
// ==========================================================================================================
// AddressConfig
// ==========================================================================================================

/// IEEE 802.15.4 addressing configuration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressConfig {
    /// Personal Area Network ID -> All nodes belonging to the same network should use the same ID
    pub pan_id: u16,
    /// Addresse of this node inside the PAN
    pub short_addr: u16,
}

impl Default for AddressConfig {
    fn default() -> Self {
        Self {
            pan_id: 0xAAAA,
            short_addr: 0xFFFF,
        }
    }
}

// ==========================================================================================================
// PhyConfig
// ==========================================================================================================

/// Physical layer configuration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PhyConfig {
    /// UWB RF channel (Center frequency band)
    pub channel: UwbChannel,
    /// Transmission data rate
    /// Note: Requires matching PreambleLength and SFD
    pub data_rate: DataRate,
    /// Pulse Repetition Frequency
    /// Note: 64 MHz requires PreambleCodes (9-24) compared to 16MHz (1-8)
    pub prf: Prf,
    /// Preable length
    /// Note: Dictated PAC size; Slower data rates require longer preamples
    pub preamble_length: PreambleLength,
    /// Preamble code for transmitter -> Must match channel and chosen PRF
    pub tx_preamble_code: PreambleCode,
    /// Preamble code for receiver -> Must match code of transmitter
    pub rx_preamble_code: PreambleCode,
    /// Start of Frame Delimiter -> Marks transition from preamble to physical Header
    /// Note: NonStandard improves 6.8Mbps tracking. 110kbps requires Standard 64-symbol SFD
    pub sfd_mode: SfdMode,
    /// Physical Header mode
    /// Note: Standard restricts payload to 127 bytes; Extended allows up to 1023 bytes
    pub phr_mode: PhrMode,
    /// Marks tansmitted frames as ranging franes to firce hight-precision timestamping
    pub ranging: bool,
}

impl Default for PhyConfig {
    fn default() -> Self {
        Self {
            channel: UwbChannel::Ch5,
            data_rate: DataRate::Kbps6800,
            prf: Prf::Mhz64,
            preamble_length: PreambleLength::Len128,
            tx_preamble_code: PreambleCode::Code9,
            rx_preamble_code: PreambleCode::Code9,
            sfd_mode: SfdMode::Standard,
            phr_mode: PhrMode::Standard,
            ranging: true,
        }
    }
}

/// Supported DW1000 UWB channels (only 500MHz bandwidth)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UwbChannel {
    Ch1,
    Ch2,
    Ch3,
    //Ch4,
    Ch5,
    //Ch7,
}

/// Supported DW1000 data rates
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataRate {
    /// 110 kbit/s
    /// Long airtime and usally longer SFD
    Kbps110,
    /// 850 kbit/s
    Kbps850,
    /// 6.8 Mbit/s
    /// Short airtime and shoert SFD
    Kbps6800,
}

/// Supported DW1000 Pulse Repetition Frequency
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prf {
    Mhz16,
    Mhz64,
}

/// Preamble length in symbols
/// Longer preambles improve detection robustness but increase airtime
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreambleLength {
    Len64,
    Len128,
    Len256,
    Len512,
    Len1024,
    Len1536,
    Len2048,
    Len4096,
}

impl PreambleLength {
    /// Return the numeric smybol count
    pub const fn symbols(self) -> u16 {
        match self {
            Self::Len64 => 64,
            Self::Len128 => 128,
            Self::Len256 => 256,
            Self::Len512 => 512,
            Self::Len1024 => 1024,
            Self::Len1536 => 1536,
            Self::Len2048 => 2048,
            Self::Len4096 => 4096,
        }
    }

    /// Return recommended PAC size for this preamble length
    pub const fn recommended_pac(self) -> PacSize {
        match self {
            PreambleLength::Len64 | PreambleLength::Len128 => PacSize::Pac8,
            PreambleLength::Len256 | PreambleLength::Len512 => PacSize::Pac16,
            PreambleLength::Len1024 | PreambleLength::Len1536 => PacSize::Pac32,
            PreambleLength::Len2048 | PreambleLength::Len4096 => PacSize::Pac64,
        }
    }
}

/// PAC (Preamble Acquisition Chunk) defines the block size (8/16/32/64 symbols)
/// used by the RX hardware to detect preambles
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacSize {
    Pac8,
    Pac16,
    Pac32,
    Pac64,
}

/// Preamble codes
/// Valid options depend on channel and PRF
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreambleCode {
    Code1 = 1,
    Code2 = 2,
    Code3 = 3,
    Code4 = 4,
    Code5 = 5,
    Code6 = 6,
    Code7 = 7,
    Code8 = 8,
    Code9 = 9,
    Code10 = 10,
    Code11 = 11,
    Code12 = 12,
    Code17 = 17,
    Code18 = 18,
    Code19 = 19,
    Code20 = 20,
}

/// Start Frame Delimiter mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfdMode {
    /// IEEE Standard
    Standard,

    /// DW1000 none-standard
    /// - can improve performance
    /// - both sides must be configured compatibly
    NonStandard,
}

/// PHY header mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhrMode {
    /// IEEE standard -> Physical Service Data Unit (PSDU) up to 127 bytes
    Standard,
    /// PSDU up to 1023 bytes
    Extended,
}

// ==========================================================================================================
// InterruptConfig
// ==========================================================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptConfig {
    /// IRQ output polarity
    pub irq_pol: IrqPolarity,
    /// Generate interrupt on completed Transmission
    pub tx_done: bool,
    /// Generate interrupt on successful frame reception
    pub rx_good: bool,
    /// Generate interrupt on RX timeout
    pub rx_timeout: bool,
    /// Generate interrupt on RX error
    pub rx_error: bool,
}

impl Default for InterruptConfig {
    fn default() -> Self {
        Self {
            irq_pol: IrqPolarity::ActiveHigh,
            tx_done: true,
            rx_good: true,
            rx_timeout: true,
            rx_error: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqPolarity {
    ActiveHigh,
    ActiveLow,
}

// ==========================================================================================================
// AntennaDelayConfig
// ==========================================================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntennaDelayConfig {
    /// TX antenna delay correction
    pub tx: UWBDuration,
    /// RX antemma delay correction
    pub rx: UWBDuration,
}

impl Default for AntennaDelayConfig {
    fn default() -> Self {
        Self {
            tx: UWBDuration::from_ticks(0),
            rx: UWBDuration::from_ticks(0),
        }
    }
}

// ==========================================================================================================
// TxPowerConfig
// ==========================================================================================================

/// Transmit power configuration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxPowerConfig {
    /// Whether smart TX power control is enabled
    /// - DW1000 can use higher power for short 6.8Mbps frames while stying within average
    ///   relulatory limits
    pub smart_power: bool,
}

impl Default for TxPowerConfig {
    fn default() -> Self {
        Self { smart_power: true }
    }
}

// ==========================================================================================================
// MacConfig
// ==========================================================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacConfig {
    /// Automatically discards incoming frames not matching PAN ID or address
    pub frame_filtering: bool,
    /// Allow beacon frames when frame filtering is enabled
    pub allow_beacon: bool,
    /// Allow data frames when frame filtering is enabled
    pub allow_data: bool,
    /// Allow acknowledgement frames when frame filtering is enabled
    pub allow_ack: bool,
    /// Allow MAC command frames when frame filtering is enabled
    pub allow_mac_command: bool,
    /// Enables automatic ack generation
    pub auto_ack: bool,
    /// Enables automatic crc generation
    pub crc_anabled: bool,
}

impl Default for MacConfig {
    fn default() -> Self {
        Self {
            frame_filtering: false,
            allow_beacon: false,
            allow_data: true,
            allow_ack: true,
            allow_mac_command: true,
            auto_ack: false,
            crc_anabled: true,
        }
    }
}

// ==========================================================================================================
// RxConfig
// ==========================================================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RxConfig {
    /// Max time tp wait for a complete frame
    pub frame_timeout: Option<UWBDuration>,
    /// Preamble detection timeout
    pub preamble_timeout: Option<UWBDuration>,
    /// SFD detection timeout
    pub sfd_timeout: Option<UWBDuration>,
}
