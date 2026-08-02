use godot::{classes::ClassDb, prelude::*};

mod cam3d;
mod crosshair;
mod list_item;
mod menu;
mod node;
mod side_menu;
mod signal_bus;
mod sim_config;
mod sim_core;
mod sim_engine;
mod sim_speed;
mod sim_types;
mod simulation;

mod propagation;

mod sim_logic {
    pub use uwb_sim::event_queue::{PlaybackState, SchedulerStep, SimComEvent, SimScheduler};
    pub use uwb_sim::hal_command::*;
    pub use uwb_sim::id::{EventID, NodeID};
    pub use uwb_sim::pending_operation::*;
    pub use uwb_sim::sim_error::SimHalError;
    pub use uwb_sim::sim_frame::SimMacFrame;
    pub use uwb_sim::sim_time::{UWBSimDuration, UWBSimTimestamp};
}

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
