use crate::id::NodeID;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimHalError {
    InvalidOperation,
    NodeRemoved(NodeID),
    OperationInPast,
    RadioBusy(NodeID),
    UnknwonNode(NodeID),
    Stopped,
}
