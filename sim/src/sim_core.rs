use godot::prelude::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use uwb_sim::sim_error::SimHalError;

use crate::{
    node::UwbNode,
    propagation::{LinkInfo, LinkQuality, PathInfo, calc_direct_path, debug_print_connectivity},
    signal_bus::{SignalBus, SimState},
    sim_engine::{NodeId, SimulationEngine},
    sim_logic::{PlaybackState, SimCom, SimComEvent},
    sim_types::*,
    simulation::TerrainType,
};

#[derive(Debug)]
pub enum SimEvent {
    SetSimState(SimState),
    SaveConfigFile(String),
    LoadConfigFile(String),
    ResetSimulation,
    AddSensor(SimObjectData),
    AddGarage(SimObjectData),
    RemoveListItem(u32),
    ItemNameChanged(u32, String),
    SetSimSpeed(u32),
    SetPlaybackState(PlaybackState),
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SimCore {
    base: Base<Node>,

    /// Cached access to signal bus
    signal_bus: Option<Gd<SignalBus>>,

    /// not time based events
    event_queue: Vec<SimEvent>,

    /// time bases events and scheduler
    engine: SimulationEngine,

    // global states
    sim_state: SimState,

    /// Obejct data
    sensor_data: Vec<SimObjectData>,
    garage_data: Option<SimObjectData>,

    /// Node data
    uwb_nodes: Vec<SimUwbNode>,

    /// Terrain configuration
    terrain_data: Option<TerrainData>,

    project_path: Option<String>,
    can_save: bool,

    /// Connectivity data calculated through godot physics world
    connectivity_dirty: bool,
}

#[godot_api]
impl INode for SimCore {
    fn init(base: Base<Node>) -> Self {
        Self {
            sim_state: SimState::Idle,
            sensor_data: Vec::new(),
            garage_data: None,
            terrain_data: None,
            uwb_nodes: Vec::new(),
            project_path: None,
            can_save: false,
            event_queue: Vec::new(),
            signal_bus: None,
            connectivity_dirty: true,
            base,
            engine: SimulationEngine::new(),
        }
    }

    fn ready(&mut self) {
        if let Some(bus) = self
            .base()
            .try_get_node_as::<SignalBus>("/root/GlobalSignalBus")
        {
            self.signal_bus = Some(bus);
            godot_print!("[SimCore] SignalBus successfully cached");
        } else {
            godot_error!("[SimCore] SignalBus not found in Autoload");
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        // 1) Commands from UI and scene objects
        self.process_events();

        // 2) step in sim
        self.engine.tick();

        // 3) Rebuild connectivity if necessary and send to engine
        if self.connectivity_dirty {
            let graph = self.rebuild_connectivity_graph();
            self.engine.update_links(graph);
            self.connectivity_dirty = false;
        }
    }
}

impl SimCore {
    pub fn get_playback_state(&self) -> PlaybackState {
        self.engine.get_playback_state()
    }

    pub fn get_sim_speed(&self) -> u32 {
        self.engine.get_sim_speed()
    }

    pub fn get_engine_mut(&mut self) -> &mut SimulationEngine {
        &mut self.engine
    }

    pub fn get_engine(&self) -> &SimulationEngine {
        &self.engine
    }

    pub fn get_link_info(&self, from_id: u32, to_id: u32) -> Option<&LinkInfo> {
        self.connectivity_graph
            .get(&from_id)
            .and_then(|target| target.get(&to_id))
    }

    pub fn get_terrain_data(&self) -> Option<TerrainData> {
        self.terrain_data.clone()
    }

    pub fn push_sim_event(&mut self, event: SimEvent) {
        self.event_queue.push(event);
    }

    pub fn process_events(&mut self) {
        if self.event_queue.is_empty() {
            return;
        }

        let events = std::mem::take(&mut self.event_queue);

        for event in events {
            match event {
                SimEvent::SetSimState(state) => {
                    if self.sim_state != state {
                        self.sim_state = state;
                        godot_print!("[SimCore] SimState: {:?}", state);

                        // Access Signal Bus and emit signal
                        if let Some(bus) = &self.signal_bus {
                            bus.signals().sim_state_changed().emit(state);
                        }
                    }
                }
                SimEvent::SaveConfigFile(path) => {
                    self.save_project_to_json(path);
                }
                SimEvent::LoadConfigFile(path) => {
                    self.load_project_from_json(path);
                }
                SimEvent::ResetSimulation => {
                    self.clear_sim_objects();
                    self.terrain_data = None;
                    self.sensor_data.clear();
                    self.uwb_nodes.clear();
                    self.garage_data = None;
                    self.can_save = false;
                    self.project_path = None;
                    self.engine.reset();
                    self.connectivity_dirty = true;
                    self.connectivity_graph.clear();
                    // Access Signal Bus and emit signal
                    if let Some(bus) = &self.signal_bus {
                        bus.signals().sim_state_changed().emit(SimState::Idle);
                        bus.signals().project_item_state_changed().emit(0, true);
                        bus.signals().project_item_state_changed().emit(1, true);
                        bus.signals().on_garage_ex_changed().emit(false);
                    }
                }
                SimEvent::SetPlaybackState(state) => {
                    godot_print!("[SimCore] state: {}", state as u32);
                    self.engine.set_playback_state(state);
                }
                SimEvent::AddSensor(sensor_data) => {
                    godot_print!(
                        "[SimCore] New Sensor '{}' with id {} registered {}",
                        &sensor_data.name,
                        &sensor_data.id,
                        &sensor_data.position
                    );

                    if let Some(bus) = &self.signal_bus {
                        bus.signals().new_object_spawned().emit(
                            true,
                            sensor_data.name.clone(),
                            sensor_data.id,
                        );
                    }

                    let mut uwb_node = sensor_data.instance.get_node_as::<UwbNode>("UWB/UwbNode");

                    uwb_node.bind_mut().set_id(sensor_data.id);

                    self.uwb_nodes.push(SimUwbNode {
                        id: sensor_data.id,
                        position: uwb_node.get_global_position(),
                        node_type: NodeType::Normal,
                        instance: uwb_node,
                    });

                    self.sensor_data.push(sensor_data);
                    self.connectivity_dirty = true;
                }
                SimEvent::AddGarage(garage_data) => {
                    if self.garage_data.is_none() {
                        godot_print!("[SimCore] New Garage registered {}", &garage_data.position);

                        if let Some(bus) = &self.signal_bus {
                            bus.signals().on_garage_ex_changed().emit(true);
                            bus.signals().sim_state_changed().emit(SimState::NodeEditor);
                            bus.signals().new_object_spawned().emit(
                                false,
                                garage_data.name.clone(),
                                garage_data.id,
                            );
                        }

                        let mut uwb_node_center = garage_data
                            .instance
                            .get_node_as::<UwbNode>("UWB_Center/UwbNode");
                        let mut uwb_node_x =
                            garage_data.instance.get_node_as::<UwbNode>("UWB_X/UwbNode");
                        let mut uwb_node_z =
                            garage_data.instance.get_node_as::<UwbNode>("UWB_Z/UwbNode");

                        uwb_node_center.bind_mut().set_id(garage_data.id);
                        uwb_node_x.bind_mut().set_id(garage_data.id + 1);
                        uwb_node_z.bind_mut().set_id(garage_data.id + 2);

                        self.uwb_nodes.push(SimUwbNode {
                            id: garage_data.id,
                            position: uwb_node_center.get_global_position(),
                            node_type: NodeType::Garage(GarageNode::Center),
                            instance: uwb_node_center,
                        });
                        self.uwb_nodes.push(SimUwbNode {
                            id: garage_data.id + 1,
                            position: uwb_node_x.get_global_position(),
                            node_type: NodeType::Garage(GarageNode::X),
                            instance: uwb_node_x,
                        });
                        self.uwb_nodes.push(SimUwbNode {
                            id: garage_data.id + 2,
                            position: uwb_node_z.get_global_position(),
                            node_type: NodeType::Garage(GarageNode::Z),
                            instance: uwb_node_z,
                        });

                        self.garage_data = Some(garage_data);
                        self.connectivity_dirty = true;
                    }
                }
                SimEvent::RemoveListItem(id) => {
                    self.connectivity_dirty = true;
                    if id == 0 {
                        // delete garage
                        if let Some(garage) = self.garage_data.take() {
                            let mut instance = garage.instance;
                            if instance.is_instance_valid() {
                                instance.queue_free();
                            }

                            // delete 3 uwb nodes on the garage
                            let looking_for_ids = vec![0, 1, 2];
                            self.uwb_nodes.retain(|s| !looking_for_ids.contains(&s.id));

                            if let Some(bus) = &self.signal_bus {
                                bus.signals().on_garage_ex_changed().emit(false);
                            }

                            godot_print!("[SimCore] Garage successfully deleted");
                        }
                    } else {
                        // delete uwb node
                        if let Some(index) = self.sensor_data.iter().position(|s| s.id == id) {
                            let removed_sensor = self.sensor_data.remove(index);
                            let mut instance = removed_sensor.instance;
                            if instance.is_instance_valid() {
                                instance.queue_free();
                            }
                            if let Some(index) = self.uwb_nodes.iter().position(|s| s.id == id) {
                                let _ = self.uwb_nodes.remove(index);
                            }
                            godot_print!("[SimCore] Sensor (ID: {}) successfully deleted", id);
                        }
                    }
                }
                SimEvent::ItemNameChanged(id, new_name) => {
                    if id == 0 {
                        if let Some(ref mut garage) = self.garage_data {
                            garage.name = new_name.clone();
                            godot_print!("[SimCore] Garage renamed to {}", &new_name);
                        }
                    } else {
                        if let Some(sensor) = self.sensor_data.iter_mut().find(|s| s.id == id) {
                            sensor.name = new_name.clone();

                            godot_print!("[SimCore] Sensor (ID: {}) renamed to {}", id, &new_name);
                        }
                    }
                }
                SimEvent::SetSimSpeed(speed) => {
                    self.engine.set_sim_speed(speed);
                    if let Some(bus) = &self.signal_bus {
                        bus.signals().sim_speed_changed().emit(speed);
                    }
                    godot_print!("[SimCore] Speed set to {}", speed);
                }
            }
        }
    }

    pub fn set_terrain(&mut self, terrain_data: TerrainData) {
        godot_print!(
            "[SimCore] Terrain successfully registered:\n\
             -> Type: {:?}\n\
             -> Path: {}\n\
             -> Size: {}x{}m\n\
             -> Height Limits: Min: {}, Max: {}",
            terrain_data.terrain_type,
            terrain_data.terrain_path,
            terrain_data.size_x,
            terrain_data.size_y,
            terrain_data
                .min_height
                .map(|h| h.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            terrain_data
                .max_height
                .map(|h| h.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        );

        if terrain_data.terrain_type == TerrainType::None || terrain_data.terrain_path.is_empty() {
            godot_error!("[SimCore] Invalid TerrainData");
            return;
        }
        if let Some(bus) = &self.signal_bus {
            bus.signals().project_item_state_changed().emit(0, false);
        }
        self.terrain_data = Some(terrain_data);
    }

    pub fn get_sensor_count(&self) -> usize {
        self.sensor_data.len()
    }

    fn clear_sim_objects(&mut self) {
        for mut node in self.sensor_data.drain(..) {
            if node.instance.is_instance_valid() {
                node.instance.queue_free();
            }
        }

        if let Some(mut garage) = self.garage_data.take() {
            garage.instance.queue_free();
        }

        godot_print!("[SimCore] All sensors cleared");
    }

    fn load_project_from_json(&mut self, absolute_path: String) {
        if absolute_path.is_empty() {
            godot_error!("[SimCore] Cant load configuration from empty path");
            return;
        }

        godot_print!("[SimCore] Loading Config from {}", &absolute_path);

        match std::fs::read_to_string(&absolute_path) {
            Ok(string) => match serde_json::from_str::<ProjectData>(&string) {
                Ok(loaded_data) => {
                    self.project_path = Some(absolute_path.clone());
                    self.can_save = true;

                    self.terrain_data = Some(loaded_data.terrain);

                    let mut sensor_dict: Dictionary<GString, Vector3> = Dictionary::new();
                    for (name, pos) in &loaded_data.sensors {
                        let godot_pos = Vector3::new(pos[0], pos[1], pos[2]);
                        sensor_dict.set(&GString::from(name), godot_pos);
                    }

                    let garage_varian: Variant =
                        loaded_data.garage.as_ref().map_or(Variant::nil(), |g| {
                            let mut g_dict: Dictionary<GString, Variant> = Dictionary::new();
                            let godot_pos =
                                Vector3::new(g.position[0], g.position[1], g.position[2]);
                            g_dict.set("name", g.name.clone());
                            g_dict.set("position", godot_pos);
                            g_dict.to_variant()
                        });

                    godot_print!(
                        "[SimCore] Project successfully loaded. Restored {} sensors. Garage present: {}",
                        sensor_dict.len(),
                        loaded_data.garage.is_some()
                    );

                    if let Some(bus) = &self.signal_bus {
                        bus.signals().project_item_state_changed().emit(1, false);

                        // send signal deferred
                        bus.clone().call_deferred(
                            "emit_signal",
                            &[
                                "sim_config_loaded".to_variant(),
                                sensor_dict.to_variant(),
                                garage_varian,
                            ],
                        );
                    }
                }
                Err(err) => {
                    godot_error!("[SimCore] Deserialization failed. Error: {:?}", err);
                }
            },
            Err(err) => {
                godot_error!(
                    "[SimCore] Failed to read file from disk at '{}'. Error: {:?}",
                    absolute_path,
                    err
                );
            }
        }
    }

    fn save_project_to_json(&mut self, absolute_path: String) {
        if self.terrain_data.is_none() {
            godot_error!("[SimCore] No TerrainData available to save");
        }

        let abs_path = if absolute_path.is_empty() {
            if let Some(ref path) = self.project_path {
                path.clone()
            } else {
                godot_error!("[SimCore] No valid path to save file");
                return;
            }
        } else {
            self.project_path = Some(absolute_path.clone());
            absolute_path
        };

        if !self.can_save {
            self.can_save = true;
            if let Some(bus) = &self.signal_bus {
                bus.signals().project_item_state_changed().emit(1, false);
            }
        }

        godot_print!("[SimCore] Saving SimConfig to {}", &abs_path);
        // Convert Godot Vector3 into standard array
        let sensors: HashMap<String, [f32; 3]> = self
            .sensor_data
            .iter()
            .map(|obj| {
                (
                    obj.name.clone(),
                    [obj.position.x, obj.position.y, obj.position.z],
                )
            })
            .collect();

        let garage: Option<GarageSaveData> = self.garage_data.as_ref().map(|data| GarageSaveData {
            name: data.name.clone(),
            position: [data.position.x, data.position.y, data.position.z],
        });

        // Assemble the current sim state containing only raw config parameter
        let save_data = ProjectData {
            terrain: self.terrain_data.clone().unwrap(),
            sensors: sensors,
            garage: garage,
        };

        // Convert data to json format
        match serde_json::to_string(&save_data) {
            Ok(json_string) => {
                // Open or create target File
                match File::create(&abs_path) {
                    Ok(mut file) => {
                        // Write unified string into the file system stream
                        if let Err(err) = file.write_all(json_string.as_bytes()) {
                            godot_error!(
                                "[SimCore] Failed to write bytes into file stream at '{}'. Error: {:?}",
                                abs_path,
                                err
                            );
                        } else {
                            godot_print!(
                                "[SimCore] Configuration file successfully saved -> {}",
                                abs_path
                            );
                        }
                    }
                    Err(err) => {
                        godot_error!(
                            "[SimCore] Unable write file target at '{}'. Error: {:?}",
                            abs_path,
                            err
                        );
                    }
                }
            }
            Err(err) => {
                godot_error!("[SimCore] Serialization failed. Error: {:?}", err);
            }
        }
    }

    fn rebuild_connectivity_graph(&self) -> HashMap<NodeId, HashMap<NodeId, LinkInfo>> {
        const MASK: u32 = 1;

        let mut graph = HashMap::new();

        let Some(viewport) = self.base().get_viewport() else {
            godot_error!("[SimCore] No viewport available");
            return graph;
        };

        let Some(world_3d) = viewport.get_world_3d() else {
            godot_error!("[SimCore] No World3D available");
            return graph;
        };

        let Some(space_state) = world_3d.get_direct_space_state() else {
            godot_error!("[SimCore] No SpaceState available");
            return graph;
        };

        for node in &self.uwb_nodes {
            if node.instance.is_instance_valid() {
                graph.insert(node.id, HashMap::new());
            }
        }

        for i in 0..self.uwb_nodes.len() {
            for j in (i + 1)..self.uwb_nodes.len() {
                let a = &self.uwb_nodes[i];
                let b = &self.uwb_nodes[j];

                if !a.instance.is_instance_valid() || !b.instance.is_instance_valid() {
                    continue;
                }

                let Some(path_info) =
                    calc_direct_path(space_state.clone(), a.position, b.position, MASK)
                else {
                    continue;
                };

                let link_info = LinkInfo {
                    path: path_info,
                    quality: LinkQuality {},
                };

                if let Some(target) = graph.get_mut(&a.id) {
                    target.insert(b.id, link_info.clone());
                }

                if let Some(target) = graph.get_mut(&b.id) {
                    target.insert(a.id, link_info.clone());
                }
            }
        }

        godot_print!(
            "[SimCore] Connectivity graph rebuilt for {} UWB nodes",
            self.uwb_nodes.len()
        );
        debug_print_connectivity(&graph);

        graph
    }
}
