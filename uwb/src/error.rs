#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UwbError {
    Hardware(HardwareError),
    Rx(RxError),
    Protocol(ProtocolError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareError {
    ChipIdMismatch,
    SpiError,
    OTPReadFailed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxError {
    FrameTimeout,
    PreambleTimeout,
    SfdTimeout,
    CrcCheckFailed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidFrameType,
    AddressMismatch,
    InvalidTimestamp,
    InvalidPayload,
    InvalidFrameForamt,
    BufferOverflow,
}
