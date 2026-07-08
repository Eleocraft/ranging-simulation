use std::{
    iter::Sum,
    ops::{Add, Sub},
};

// Maximum bit-widrh of the DW1000 system time register
const MAX_40_BIT: u64 = (1u64 << 40) - 1;

// Represents an absolut point in time via tick count
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UWBTimestamp {
    pub ticks: u64,
}

// Represents a relative duration in ticks
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UWBDuration {
    pub ticks: u64,
}

impl UWBTimestamp {
    // Creates a timestamp from raw ticks
    pub fn from_ticks(ticks: u64) -> Self {
        Self {
            ticks: ticks & MAX_40_BIT,
        }
    }

    // Adds relative duration to an absolut timestamp
    pub fn wrapping_add_duration(self, duration: UWBDuration) -> Self {
        Self {
            ticks: (self.ticks + duration.ticks) & MAX_40_BIT,
        }
    }

    // Calcs the duration between two timestamps
    pub fn duration_since(self, earlier: Self) -> UWBDuration {
        UWBDuration {
            ticks: self.ticks.wrapping_sub(earlier.ticks) & MAX_40_BIT,
        }
    }
}

impl UWBDuration {
    // Creates relative duration directly from raw hardware ticks
    pub fn from_ticks(ticks: u64) -> Self {
        Self { ticks }
    }

    // Converts us into hardware ticks
    // - DW1000 master clock frequency: 499.2 MHz
    // - For higher accuracy in mm-level ranging, an internal phase locked loop (PPL) multiplies the
    //   base clock by 128
    // -> 499 200 000 Hz * 128 = 63 897 600 000 000 Hz
    // -> (63 897 600 000 000 ticks*s)/(1 000 000 us*s) = 63 897 ticks/us
    pub fn from_micros(us: u64) -> Self {
        Self { ticks: us * 63_897 }
    }

    // Converts ms to ticks
    pub fn from_millis(ms: u64) -> Self {
        Self::from_micros(ms * 1000)
    }

    // Converts ticks into us
    pub fn to_micros(self) -> u64 {
        self.ticks / 63_897
    }
}

impl Add<UWBDuration> for UWBTimestamp {
    type Output = UWBTimestamp;

    fn add(self, rhs: UWBDuration) -> Self::Output {
        self.wrapping_add_duration(rhs)
    }
}

impl Sub<UWBTimestamp> for UWBTimestamp {
    type Output = UWBDuration;

    fn sub(self, rhs: UWBTimestamp) -> Self::Output {
        self.duration_since(rhs)
    }
}
