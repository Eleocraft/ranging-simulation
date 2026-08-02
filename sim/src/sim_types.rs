use godot::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::node::UwbNode;
use crate::simulation::TerrainType;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GarageSaveData {
    pub name: String,
    pub position: [f32; 3],
}

#[derive(Serialize, Deserialize)]
pub struct ProjectData {
    pub terrain: TerrainData,
    pub sensors: HashMap<String, [f32; 3]>,
    pub garage: Option<GarageSaveData>,
}

#[derive(Clone, Debug)]
pub enum GarageNode {
    Center,
    X,
    Z,
}

#[derive(Clone, Debug)]
pub enum NodeType {
    Normal,
    Garage(GarageNode),
}

#[derive(Clone, Debug)]
pub struct SimUwbNode {
    pub id: u32,
    pub position: Vector3,
    pub node_type: NodeType,
    pub instance: Gd<UwbNode>,
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
