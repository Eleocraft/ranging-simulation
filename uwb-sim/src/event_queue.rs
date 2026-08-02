use uwb::RxConfig;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

use crate::{
    sim_frame::SimMacFrame,
    sim_time::{UWBSimDuration, UWBSimTimestamp},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PlaybackState {
    Stopped = 0,
    Playing = 1,
    Paused = 2,
}

impl PlaybackState {
    pub fn from_u32(state: u32) -> Self {
        match state {
            0 => PlaybackState::Stopped,
            1 => PlaybackState::Playing,
            2 => PlaybackState::Paused,
            _ => PlaybackState::Stopped,
        }
    }

    pub fn is_running(&self) -> bool {
        if *self == PlaybackState::Playing {
            return true;
        } else {
            return false;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimComEvent {
    pub time: UWBSimTimestamp,
    pub id: u32,
    pub event_type: ComType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComType {
    Transmit {
        sender_id: u32,
        data: SimMacFrame,
    },
    Receive {
        sender_id: u32,
        receiver_id: u32,
        rx_config: RxConfig,
        data: SimMacFrame,
    },
}

impl Ord for SimComEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.time.cmp(&self.time) {
            Ordering::Equal => self.id.cmp(&other.id), // use time first then id
            other => other,
        }
    }
}

impl PartialOrd for SimComEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerStep {
    Event(SimComEvent),
    WindowFinished,
    NotRunning,
}

pub struct SimScheduler {
    /// Simulated time window in us by one frame
    speed: u32,
    /// Current sim time
    current_sim_time: UWBSimTimestamp,
    /// End of currently active sim window
    active_window_end: Option<UWBSimTimestamp>,
    /// playback state
    playback_state: PlaybackState,
    /// Event queue
    event_queue: BinaryHeap<SimComEvent>,
}

impl SimScheduler {
    pub fn new() -> Self {
        Self {
            event_queue: BinaryHeap::new(),
            speed: 50,
            current_sim_time: UWBSimTimestamp::from_ticks(0),
            active_window_end: None,
            playback_state: PlaybackState::Stopped,
        }
    }

    pub fn next_step(&mut self) -> SchedulerStep {
        if !self.playback_state.is_running() {
            return SchedulerStep::NotRunning;
        }

        self.ensure_active_window();

        let Some(window_end) = self.active_window_end else {
            println!("[Event Queue] No window end set");
            return SchedulerStep::WindowFinished;
        };

        let event_time = {
            let Some(next_event) = self.peek_next_event() else {
                self.finish_active_window();
                return SchedulerStep::WindowFinished;
            };
            next_event.time
        };

        if !self.event_is_inside_window(event_time, window_end) {
            self.finish_active_window();
            return SchedulerStep::WindowFinished;
        }

        let event = self.pop_next_event().expect("peeked event must exist");
        self.current_sim_time = event.time;

        SchedulerStep::Event(event)
    }

    pub fn set_speed(&mut self, new_speed: u32) {
        self.speed = new_speed;
    }

    pub fn get_speed(&self) -> u32 {
        self.speed
    }

    pub fn set_playback_state(&mut self, new_state: PlaybackState) {
        self.playback_state = new_state;
    }

    pub fn get_playback_state(&self) -> PlaybackState {
        self.playback_state
    }

    pub fn get_current_sim_time(&self) -> UWBSimTimestamp {
        self.current_sim_time
    }

    pub fn peek_next_event(&mut self) -> Option<&SimComEvent> {
        self.event_queue.peek()
    }

    pub fn push_com_event(&mut self, event: SimComEvent) {
        self.event_queue.push(event);
    }

    pub fn pop_next_event(&mut self) -> Option<SimComEvent> {
        self.event_queue.pop()
    }

    pub fn ensure_active_window(&mut self) {
        if self.active_window_end.is_none() {
            let duration = UWBSimDuration::from_micros(self.speed as u64);
            self.active_window_end = Some(self.current_sim_time + duration);
        }
    }

    pub fn finish_active_window(&mut self) {
        if let Some(window_end) = self.active_window_end.take() {
            self.current_sim_time = window_end;
        }
    }

    pub fn event_is_inside_window(
        &self,
        event_time: UWBSimTimestamp,
        window_end: UWBSimTimestamp,
    ) -> bool {
        let time_till_event = event_time.duration_since(self.current_sim_time);

        let time_till_end = window_end.duration_since(self.current_sim_time);

        time_till_event <= time_till_end
    }
}
