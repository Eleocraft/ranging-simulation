use tokio::sync::oneshot;

use crate::{id::NodeID, sim_error::SimHalError, sim_frame::SimMacFrame};

/// HAL operation that has startet but is waiting to be finished
pub enum PendingOperation {
    Transmit {
        node_id: NodeID,
        reply: oneshot::Sender<Result<(), SimHalError>>,
    },

    Receive {
        node_id: NodeID,
        reply: oneshot::Sender<Result<SimMacFrame, SimHalError>>,
    },
}

impl PendingOperation {
    /// Returns id of the node owning this operation
    pub fn node_id(&self) -> NodeID {
        match self {
            PendingOperation::Receive { node_id, .. }
            | PendingOperation::Transmit { node_id, .. } => *node_id,
        }
    }

    /// Cancel this operation with an error
    pub fn quit(self, error: SimHalError) {
        match self {
            PendingOperation::Transmit { reply, .. } => {
                let _ = reply.send(Err(error));
            }

            PendingOperation::Receive { reply, .. } => {
                let _ = reply.send(Err(error));
            }
        }
    }

    /// Complete a transmit successfully
    pub fn complete_transmit(self) -> Result<(), SimHalError> {
        match self {
            PendingOperation::Transmit { reply, .. } => {
                let _ = reply.send(Ok(()));
                Ok(())
            }

            PendingOperation::Receive { .. } => Err(SimHalError::InvalidOperation),
        }
    }

    /// Complete a receive operation successfully
    pub fn complete_receive(self, frame: SimMacFrame) -> Result<(), SimHalError> {
        match self {
            PendingOperation::Receive { reply, .. } => {
                let _ = reply.send(Ok(frame));
                Ok(())
            }

            PendingOperation::Transmit { .. } => Err(SimHalError::InvalidOperation),
        }
    }
}
