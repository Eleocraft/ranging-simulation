use tokio::sync::oneshot;
use uwb::RxConfig;

use crate::{
    id::{EventID, NodeID},
    sim_frame::SimMacFrame,
    sim_time::UWBSimTimestamp,
};

/// Coammands sent from simulated HAL to sim engine
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HalCommand {
    /// Starts a transmission at the current simulation time
    TransmitNow {
        node_id: NodeID,
        frame: SimMacFrame,
        reply: oneshot::Sender<Result<(), SimHalError>>,
    },

    /// Starts transmission at specified timestamp
    TransmitAt {
        node_id: NodeID,
        frame: SimMacFrame,
        time: UWBSimTimestamp,
        reply: oneshot::Sender<Result<(), SimHalError>>,
    },

    /// Enables reception of one message
    BeginReceive {
        node_id: NodeID,
        config: RxConfig,
        reply: oneshot::Sender<Result<SimMacFrame, SimHalError>>,
    },

    /// Stops currently active operations
    Stop {
        node_id: NodeID,
        reply: oneshot::Sender<Result<(), SimHalError>>,
    },
}
