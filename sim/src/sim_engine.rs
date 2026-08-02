use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

use tokio::sync::oneshot;

use uwb::{RxConfig, config::UwbConfig};
use uwb_sim::{
    event_queue::SimComEvent, id::EventID, sim_error::SimHalError, sim_frame::SimMacFrame,
    sim_time::UWBSimTimestamp,
};

use crate::{
    propagation::LinkInfo,
    sim_config::SimErrorModelConfig,
    sim_logic::{
        EventID, HalCommand, NodeID, PendingOperation, PlaybackState, SchedulerStep, SimScheduler,
    },
};

/// Represents internal state of a single simulated UWB node
#[derive(Clone, Copy, Debug)]
pub struct NodeState {
    pub id: NodeID,
    /// Operational State of the radio (Idle, Transmitting, Receiving)
    pub radio_state: RadioState,
    /// Active receive config when node is currently receivung
    pub rx_config: Option<RxConfig>,
    /// Timestamp when the current operation started
    pub operation_started: Option<UWBSimTimestamp>,
    /// Config which is currently used on the node
    pub operating_config: UwbConfig,
}

impl NodeState {
    pub fn new(id: NodeID) -> Self {
        Self {
            id,
            radio_state: RadioState::Idle,
            rx_config: None,
            operation_started: None,
            operating_config: UwbConfig::default(),
        }
    }

    pub fn set_config(&mut self, config: UwbConfig) {
        self.operating_config = config;
    }

    pub fn id_idle(&self) -> bool {
        self.radio_state == RadioState::Idle
    }

    pub fn set_idle(&mut self) {
        self.radio_state = RadioState::Idle;
        self.rx_config = None;
        self.operation_started = None;
    }

    pub fn active_event_id(&self) -> Option<EventID> {
        match self.radio_state {
            RadioState::Idle => None,
            RadioState::Receiving { event_id } | RadioState::Transmitting { event_id } => {
                Some(event_id)
            }
        }
    }
}

/// Defines possible hardware states for a simulated node
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioState {
    Idle,
    Transmitting { event_id: EventID },
    Receiving { event_id: EventID },
}

/// Runs time-based part of the sim
pub struct SimulationEngine {
    /// Com Event queue and scheduler
    scheduler: SimScheduler,
    /// Receiver for Commands from all simulated HAL instances
    command_rx: Receiver<HalCommand>,
    /// Sender for COmmands cloned into new HAL instances
    command_tx: Sender<HalCommand>,
    /// States of every node indexed by NodeID
    nodes: HashMap<NodeID, NodeState>,
    /// Current communication links
    links: HashMap<NodeID, HashMap<NodeID, LinkInfo>>,
    /// HAL operations waiting for completion
    pending_operations: HashMap<EventID, PendingOperation>,
    /// maximum cpu budget per frame
    cpu_budget: Duration,
    /// Error modeling config
    error_modeling_config: SimErrorModelConfig,
    /// Counter for generating unique event IDs
    next_event_id: EventID,
}

impl SimulationEngine {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel();

        Self {
            scheduler: SimScheduler::new(),
            command_rx,
            command_tx,
            nodes: HashMap::new(),
            links: HashMap::new(),
            pending_operations: HashMap::new(),
            next_event_id: 0,
            cpu_budget: Duration::from_millis(6),
            error_modeling_config: SimErrorModelConfig::default(),
        }
    }

    /// Updates error modeling configuration used to calculate TOF
    pub fn set_error_modeling_config(&mut self, config: SimErrorModelConfig) {
        self.error_modeling_config = config;
    }

    /// Drains and processes all commands sent from the HAL instances
    fn process_pending_commands(&mut self) {
        loop {
            match self.command_rx.try_recv() {
                Ok(command) => {
                    self.process_hal_command(command);
                }

                Err(TryRecvError::Empty) => {
                    break;
                }

                Err(TryRecvError::Disconnected) => {
                    eprintln!("[Engine] HAL command channel disconnected");
                    break;
                }
            }
        }
    }

    /// Routes incoming HAL commands to their respective handlers
    fn process_hal_command(&mut self, command: HalCommand) {
        match command {
            HalCommand::TransmitNow {
                node_id,
                frame,
                reply,
            } => {
                let time = self.get_current_sim_time();

                self.start_transmission(node_id, time, frame, reply);
            }

            HalCommand::TransmitAt {
                node_id,
                frame,
                time,
                reply,
            } => {
                self.start_transmission(node_id, time, frame, reply);
            }

            HalCommand::BeginReceive {
                node_id,
                config,
                reply,
            } => {
                self.start_receive(node_id, config, reply);
            }

            HalCommand::Stop { node_id, reply } => {
                let result = self.stop_node(node_id);
                let _ = reply.send(result);
            }
        }
    }

    /// Validates and schedules a transmission event
    fn start_transmission(
        &mut self,
        node_id: NodeID,
        time: UWBSimTimestamp,
        frame: SimMacFrame,
        reply: oneshot::Sender<Result<(), SimHalError>>,
    ) {
        let current_time = self.scheduler.get_current_sim_time();

        // Reject trnasmissions scheduled in the past
        if time < current_time {
            let _ = reply.send(Err(SimHalError::OperationInPast));
            return;
        }

        // Ensure Transmitting node exists
        let Some(node) = self.nodes.get_mut(&node_id) else {
            let _ = reply.send(Err(SimHalError::UnknwonNode(node_id)));
            return;
        };

        // Ensure the radio is free
        if !node.id_idle() {
            let _ = reply.send(Err(SimHalError::RadioBusy(node_id)));
            return;
        }

        let event_id = self.get_event_id();

        // Update node state to transmitting
        node.radio_state = RadioState::Transmitting { event_id };
        node.operation_started = Some(time);
        node.rx_config = None;

        // Register the pending operation to reply later when finished
        self.pending_operations
            .insert(event_id, PendingOperation::Transmit { node_id, reply });

        // Build and push com event in scheduler
        let event = SimComEvent {
            time,
            id: event_id,
            event_type: uwb_sim::event_queue::ComType::Transmit {
                sender_id: node_id,
                data: frame,
            },
        };
        self.scheduler.push_com_event(event);
    }

    /// Validate node state and puts radio in receiving mode
    fn start_receive(
        &mut self,
        node_id: NodeID,
        config: RxConfig,
        reply: oneshot::Sender<Result<SimMacFrame, SimHalError>>,
    ) {
        // Ensure the receivung node exists
        let Some(node) = self.nodes.get_mut(&node_id) else {
            let _ = reply.send(Err(SimHalError::UnknwonNode(node_id)));
            return;
        };

        // Ensure the radio is free
        if !node.id_idle() {
            let _ = reply.send(Err(SimHalError::RadioBusy(node_id)));
            return;
        }

        let event_id = self.get_event_id();
        let current_time = self.scheduler.get_current_sim_time();

        // Update node state to receiving and store config
        node.radio_state = RadioState::Receiving { event_id };
        node.rx_config = Some(config);
        node.operation_started = Some(current_time);

        // Register pending receive operation
        self.pending_operations
            .insert(event_id, PendingOperation::Receive { node_id, reply });
    }

    /// Stops any active operation on a node and forces its radio to idle
    fn stop_node(&mut self, node_id: NodeID) -> Result<(), SimHalError> {
        let Some(node) = self.nodes.get_mut(&node_id) else {
            return Err(SimHalError::UnknwonNode(node_id));
        };

        let active_event_id = node.active_event_id();
        node.set_idle();

        if let Some(event_id) = active_event_id {
            if let Some(operation) = self.pending_operations.remove(&event_id) {
                operation.quit(SimHalError::Stopped);
            }
        }

        Ok(())
    }

    /// Registers a new simulation node. Returns false if the ID already exists
    pub fn register_node(&mut self, node_id: NodeID) -> bool {
        if self.nodes.contains_key(&node_id) {
            return false;
        }

        self.nodes.insert(node_id, NodeState::new(node_id));
        true
    }

    /// Unregisters an existing node and stops all active operations
    pub fn unregister_node(&mut self, node_id: NodeID) -> bool {
        let Some(node) = self.nodes.remove(&node_id) else {
            return false;
        };

        if let Some(event_id) = node.active_event_id() {
            if let Some(operation) = self.pending_operations.remove(&event_id) {
                operation.quit(SimHalError::NodeRemoved(node_id));
            }
        }

        true
    }

    /// Returns next unique event id
    pub fn get_event_id(&mut self) -> EventID {
        let id = self.next_event_id;
        self.next_event_id += 1;
        id
    }

    /// Returns a cloned command sender endpoint to be given to HAL instances
    pub fn command_sender(&self) -> Sender<HalCommand> {
        self.command_tx.clone()
    }

    /// Updates the network link information
    pub fn update_links(&mut self, new_links: HashMap<NodeID, HashMap<NodeID, LinkInfo>>) {
        self.links = new_links;
    }

    /// Returns a copy of the current link state matrix
    pub fn get_links(&self) -> HashMap<NodeID, HashMap<NodeID, LinkInfo>> {
        self.links.clone()
    }

    pub fn tick(&mut self) {
        let cpu_start = Instant::now();
        self.process_pending_commands();

        if cpu_start.elapsed() >= self.cpu_budget {
            return;
        }

        if !self.scheduler.get_playback_state().is_running() {
            return;
        }

        loop {
            if cpu_start.elapsed() >= self.cpu_budget {
                break;
            }

            match self.scheduler.next_step() {
                SchedulerStep::NotRunning | SchedulerStep::WindowFinished => {
                    break;
                }

                SchedulerStep::Event(event) => {
                    self.process_com_event(event);
                }
            }
        }
    }

    pub fn get_current_sim_time(&self) -> UWBSimTimestamp {
        self.scheduler.get_current_sim_time()
    }

    pub fn set_playback_state(&mut self, state: PlaybackState) {
        self.scheduler.set_playback_state(state);
    }

    pub fn get_playback_state(&self) -> PlaybackState {
        self.scheduler.get_playback_state()
    }

    pub fn set_sim_speed(&mut self, speed: u32) {
        self.scheduler.set_speed(speed);
    }

    pub fn get_sim_speed(&self) -> u32 {
        self.scheduler.get_speed()
    }

    pub fn reset(&mut self) {
        self.scheduler = SimScheduler;
        self.nodes = HashMap::new();
        self.links = HashMap::new();
        self.pending_operations = HashMap::new();
        self.next_event_id = 0;
    }

    pub fn scheduler(&self) -> &SimScheduler {
        &self.scheduler
    }
}
