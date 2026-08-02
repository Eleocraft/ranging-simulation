use std::ops::{Add, Sub};
use uwb::time::UWBTimestamp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UWBSimTimestamp {
    pub ticks: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UWBSimDuration {
    pub ticks: u64,
}

impl UWBSimTimestamp {
    /// Converts sim time into 40bit hardware time with 1tick = 15.6ps
    pub fn to_hw_timestamp(&self) -> UWBTimestamp {
        // hw_ticks = (sim_ticks * 63_897_600_000_000) / 1_000_000_000_000
        let hw_ticks = ((self.ticks as u128 * 63_897_600) / 1_000_000) as u64;
        UWBTimestamp::from_ticks(hw_ticks)
    }

    pub fn from_nanos(ns: u64) -> Self {
        Self { ticks: ns * 1_000 }
    }

    pub fn from_micros(us: u64) -> Self {
        Self {
            ticks: us * 1_000_000,
        }
    }

    pub fn from_ticks(ticks: u64) -> Self {
        Self { ticks }
    }

    pub fn duration_since(self, earlier: Self) -> UWBSimDuration {
        UWBSimDuration {
            ticks: self.ticks.saturating_sub(earlier.ticks),
        }
    }

    pub fn wrapping_add_duration(self, duration: UWBSimDuration) -> Self {
        Self {
            ticks: self.ticks.wrapping_add(duration.ticks),
        }
    }
}

impl UWBSimDuration {
    pub fn from_ticks(ticks: u64) -> Self {
        Self { ticks }
    }

    /// Converts nanoseconds to simulation ticks (1 ns = 1 000 ps)
    pub fn from_nanos(ns: u64) -> Self {
        Self { ticks: ns * 1_000 }
    }

    /// Converts microseconds to simulation ticks (1 us = 1 000 000 ps)
    pub fn from_micros(us: u64) -> Self {
        Self {
            ticks: us * 1_000_000,
        }
    }

    /// Converts milliseconds to simulation ticks
    pub fn from_millis(ms: u64) -> Self {
        Self::from_micros(ms * 1_000)
    }

    /// Converts simulation ticks back to microseconds
    pub fn to_micros(self) -> u64 {
        self.ticks / 1_000_000
    }

    /// Converts simulation ticks back to nanoseconds
    pub fn to_nanos(self) -> u64 {
        self.ticks / 1_000
    }
}

impl Add<UWBSimDuration> for UWBSimTimestamp {
    type Output = UWBSimTimestamp;

    fn add(self, rhs: UWBSimDuration) -> Self::Output {
        self.wrapping_add_duration(rhs)
    }
}

impl Sub<UWBSimTimestamp> for UWBSimTimestamp {
    type Output = UWBSimDuration;

    fn sub(self, rhs: UWBSimTimestamp) -> Self::Output {
        self.duration_since(rhs)
    }
}
