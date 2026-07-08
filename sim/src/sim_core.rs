use godot::prelude::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

use crate::{
    signal_bus::{SignalBus, SimState},
    simulation::TerrainType,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GarageSaveData {
    name: String,
    position: [f32; 3],
}

#[derive(Serialize, Deserialize)]
struct ProjectData {
    terrain: TerrainData,
    sensors: HashMap<String, [f32; 3]>,
    garage: Option<GarageSaveData>,
}

#[derive(Debug, Clone)]
pub struct SimObjectData {
    pub id: u32,
    pub name: String,
    pub position: Vector3,
    pub instance: Gd<Node3D>,
}

impl SimObjectData {
    pub fn new(id: u32, name: String, pos: Vector3, instance: Gd<Node3D>) -> Self {
        Self {
            position: pos,
            instance,
            id,
            name,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TerrainData {
    pub terrain_path: String,
    pub terrain_type: TerrainType,
    pub size_x: f64,
    pub size_y: f64,
    pub max_height: Option<f32>,
    pub min_height: Option<f32>,
}

impl Default for TerrainData {
    fn default() -> Self {
        Self {
            terrain_path: String::new(),
            terrain_type: TerrainType::None,
            size_x: 200.0,
            size_y: 200.0,
            max_height: None,
            min_height: None,
        }
    }
}

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
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SimCore {
    base: Base<Node>,
    signal_bus: Option<Gd<SignalBus>>,

    event_queue: Vec<SimEvent>,

    // global states
    sim_state: SimState,

    sensor_data: Vec<SimObjectData>,
    garage_data: Option<SimObjectData>,

    terrain_data: Option<TerrainData>,
    project_path: Option<String>,
    can_save: bool,
}

#[godot_api]
impl INode for SimCore {
    fn init(base: Base<Node>) -> Self {
        Self {
            sim_state: SimState::Idle,
            sensor_data: Vec::new(),
            garage_data: None,
            terrain_data: None,
            project_path: None,
            can_save: false,
            event_queue: Vec::new(),
            signal_bus: None,
            base,
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
        self.process_events();
    }
}

impl SimCore {
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
                    self.garage_data = None;
                    self.can_save = false;
                    self.project_path = None;
                    // Access Signal Bus and emit signal
                    if let Some(bus) = &self.signal_bus {
                        bus.signals().sim_state_changed().emit(SimState::Idle);
                        bus.signals().project_item_state_changed().emit(0, true);
                        bus.signals().project_item_state_changed().emit(1, true);
                        bus.signals().on_garage_ex_changed().emit(false);
                    }
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

                    self.sensor_data.push(sensor_data);
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
                        self.garage_data = Some(garage_data);
                    }
                }
                SimEvent::RemoveListItem(id) => {
                    if id == 0 {
                        // delete garage
                        if let Some(garage) = self.garage_data.take() {
                            let mut instance = garage.instance;
                            if instance.is_instance_valid() {
                                instance.queue_free();
                            }

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
}
