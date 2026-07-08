use godot::classes::base_material_3d::TextureParam;
use godot::classes::{
    CharacterBody3D, CollisionShape3D, ConfirmationDialog, FileDialog, GltfDocument, GltfState,
    HeightMapShape3D, IStaticBody3D, Image, LineEdit, Material, Mesh, MeshInstance3D, Node, Node3D,
    Shape3D, StandardMaterial3D, StaticBody3D, SurfaceTool, Texture2D, mesh,
};
use godot::prelude::*;
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, create_dir_all};
use std::io::{Cursor, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tiff::decoder::{Decoder, DecodingResult};

use crate::signal_bus::{SignalBus, SimState};
use crate::sim_core::{SimCore, SimEvent, TerrainData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainType {
    None,
    OpenTopography,
    GeoTiff,
    Glb,
    ImageHeightmap,
}

#[derive(GodotClass)]
#[class(base=StaticBody3D)]
pub struct TerrainSimulation {
    sim_core: Option<Gd<SimCore>>,
    api_key: String,
    center_latitude: f64,
    center_longitude: f64,
    #[export]
    pub area_size_x: f64,
    #[export]
    pub area_size_y: f64,

    temp_file_path: String,

    pending_terrain_id: Option<i64>,

    terrain_data: TerrainData,

    base: Base<StaticBody3D>,
}

#[godot_api]
impl IStaticBody3D for TerrainSimulation {
    fn init(base: Base<StaticBody3D>) -> Self {
        Self {
            sim_core: None,
            api_key: String::new(),      // OpenTopography API key
            center_latitude: 49.7847904, // Latitude of fields center point
            center_longitude: 9.8742604, // Longitude of fields center point
            area_size_x: 200.0,
            area_size_y: 200.0,
            temp_file_path: String::new(),
            terrain_data: TerrainData::default(),
            pending_terrain_id: None,
            base,
        }
    }

    fn ready(&mut self) {
        if let Some(sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            self.sim_core = Some(sim_core);
        } else {
            godot_error!("[Terrain] Couldnt find GlobalSimCore-Node in root");
        }

        if let Some(bus) = self
            .base()
            .try_get_node_as::<SignalBus>("/root/GlobalSignalBus")
        {
            bus.signals()
                .sim_config_loaded()
                .connect_other(&self.to_gd(), Self::on_sim_config_loaded);
        }
    }
}

#[godot_api]
impl TerrainSimulation {
    fn on_sim_config_loaded(&mut self, _sensor: Dictionary<GString, Vector3>, _garage: Variant) {
        if let Some(sim_core) = self.sim_core.clone() {
            let terrain_data = sim_core.bind().get_terrain_data();
            if let Some(data) = terrain_data {
                self.area_size_x = data.size_x;
                self.area_size_y = data.size_y;

                self.terrain_data = data.clone();

                match &data.terrain_type {
                    TerrainType::Glb => {
                        self.laod_glb_mesh(&data.terrain_path);
                    }
                    TerrainType::GeoTiff => {
                        self.laod_tiff_heightmap(&data.terrain_path);
                    }
                    TerrainType::ImageHeightmap => {
                        self.load_generic_image_heightmap(
                            &data.terrain_path,
                            data.max_height.unwrap(),
                            data.min_height.unwrap(),
                        );
                    }

                    _ => {}
                }
            }
        }
    }

    fn cache_external_file(&self, original_path_str: &str) -> Option<String> {
        let original_path = Path::new(original_path_str);

        if !original_path.exists() {
            godot_error!(
                "[Terrain] original file does not exist: {}",
                original_path_str
            );
            return None;
        }

        let cache_dir = "terrain_cache/.imported";
        if let Err(e) = fs::create_dir_all(cache_dir) {
            godot_error!("[Terrain] Couldnt create cash folder: {:?}", e);
            return None;
        }

        let extension = original_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("data");

        let file_name = original_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let new_filename = format!("{}_{}.{}", file_name, timestamp, extension);
        let full_cache_path = format!("{}/{}", cache_dir, new_filename);

        match fs::copy(original_path, &full_cache_path) {
            Ok(bytes) => {
                if let Ok(absolute_path) = Path::new(&full_cache_path).canonicalize() {
                    let absolute_path_str = absolute_path.to_str().unwrap();

                    godot_print!(
                        "[Terrain] Succesfully copied {} bytes to internal project cache -> {}",
                        bytes,
                        absolute_path_str
                    );

                    return Some(absolute_path_str.to_string());
                }

                Some(full_cache_path)
            }
            Err(err) => {
                godot_error!(
                    "[Terrain] Failed to copy file from '{}' to '{}'. Error: {:?}",
                    original_path_str,
                    full_cache_path,
                    err
                );
                None
            }
        }
    }

    fn update_terrain_geometry(&mut self, width: u32, height: u32, elevations: Vec<f32>) {
        // Fetch references to the required scene nodes and the heighmap shape
        let mut shape_node = self.base().get_node_as::<CollisionShape3D>("TerrainShape");
        let mut visual_node = self.base().get_node_as::<MeshInstance3D>("TerrainVisual");
        let mut heightmap = HeightMapShape3D::new_gd();

        // Center the grids height using the middle pixel
        let center = (width * height / 2) as usize;
        let base_height = elevations[center];
        godot_print!("[Terrain] Base Height is {}", base_height);

        // Fill native Godot float array with localized offsets relative to center point
        let mut map_data = PackedFloat32Array::new();

        // Initialize SurfaceTool for mesh generation
        let mut st = SurfaceTool::new_gd();
        st.begin(mesh::PrimitiveType::TRIANGLES);

        // max width and depth for UV normalization
        let w_f = (width - 1) as f32;
        let h_f = (height - 1) as f32;

        // Generatr all unique vertices and their UVs
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let h = elevations[idx] - base_height;
                map_data.push(h);

                // Calculate 3D-Pos (Vertex) and 2D texture coordinate (UV)
                let vertex = Vector3::new(x as f32, h, y as f32);
                let uv = Vector2::new(x as f32 / w_f, y as f32 / h_f);

                st.set_uv(uv);
                st.add_vertex(vertex);
            }
        }

        // Pass dimensions and height data to physics shape
        heightmap.set_map_width(width as i32);
        heightmap.set_map_depth(height as i32);
        heightmap.set_map_data(&map_data);

        // Override old shape
        shape_node.set_shape(&heightmap.upcast::<Shape3D>());

        // Calculate offset and allign collisionShape with visual_shape (terrain)
        let offset_x = (self.area_size_x as f32) / 2.0;
        let offset_y = (self.area_size_y as f32) / 2.0;
        shape_node.set_position(Vector3::new(offset_x, 0.0, offset_y));

        // Connect vertices to triangles using IDs
        for y in 0..(height - 1) {
            for x in 0..(width - 1) {
                // Calculate the continous IDs of the 4 corners of a quad
                let top_left = (y * width + x) as i32;
                let top_right = (y * width + (x + 1)) as i32;
                let bottom_left = ((y + 1) * width + x) as i32;
                let bottom_right = ((y + 1) * width + (x + 1)) as i32;

                // Triangle 1
                st.add_index(top_left);
                st.add_index(top_right);
                st.add_index(bottom_left);

                // Triangle 2
                st.add_index(top_right);
                st.add_index(bottom_right);
                st.add_index(bottom_left);
            }
        }

        st.generate_normals();
        st.generate_tangents();

        // Finalize mesh and assign it to the MeshInstance3D
        let mesh = st.commit().unwrap();
        visual_node.set_mesh(&mesh.upcast::<Mesh>());

        // Calculate scaling
        let s_x = (self.area_size_x / (width - 1) as f64) as f32;
        let s_y = (self.area_size_y / (height - 1) as f64) as f32;
        let scaling_vector = Vector3::new(s_x, 1.0, s_y);

        godot_print!(
            "[Terrain] Width={}, Height={}, Scaling Vector={:?}",
            width,
            height,
            scaling_vector
        );

        // Apply scaling on visual and physical component
        visual_node.set_scale(scaling_vector);
        shape_node.set_scale(scaling_vector);

        // Remove old, external mesh nodes if there
        if let Some(mut old_node) = self.base().try_get_node_as::<Node>("TerrainVisualExtern") {
            old_node.queue_free();
        }
        visual_node.set_visible(true);
        godot_print!("[Terrain] Terrain geometry generation completed");

        // Position camera above terrain center
        if let Some(mut camera) = self
            .base()
            .try_get_node_as::<CharacterBody3D>("../PhysicalFlyCam")
        {
            camera.set_global_position(Vector3::new(
                self.area_size_x as f32 / 2.0,
                35.0,
                self.area_size_y as f32 / 2.0,
            ));
        }

        // Send signal with area_size via SignalBus
        if let Some(bus) = self
            .base()
            .try_get_node_as::<SignalBus>("/root/GlobalSignalBus")
        {
            bus.signals().cam_limit_changed().emit(
                self.area_size_x as f32,
                self.area_size_y as f32,
                0.0,
                0.0,
            );
        } else {
            godot_error!("Couldnt find SignalBus-Node");
        }

        self.terrain_data.size_x = self.area_size_x;
        self.terrain_data.size_y = self.area_size_y;

        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetSimState(SimState::Spectator));

            sim_core.bind_mut().set_terrain(self.terrain_data.clone());
            // reset terrain data
            self.terrain_data = TerrainData::default();
        } else {
            godot_error!("[Terrain] Could not push event");
        }
    }

    fn execute_opentopo_download(&mut self) {
        // Latitude calculation
        let meters_per_lat_deg = 111132.0;
        let lat_offset = (self.area_size_y / 2.0) / meters_per_lat_deg;
        let south = self.center_latitude - lat_offset;
        let north = self.center_latitude + lat_offset;

        // Longitude calculation<F2>
        let lat_rad = self.center_latitude.to_radians();
        let meters_per_lon_deg = 111320.0 * lat_rad.cos();
        let lon_offset = (self.area_size_x / 2.0) / meters_per_lon_deg;
        let west = self.center_longitude - lon_offset;
        let east = self.center_longitude + lon_offset;

        // Create a unique filename for this specific area and size
        let cache_filename = format!(
            "terrain_{:.4}_{:.4}_{}x{}m.tif",
            self.center_latitude,
            self.center_longitude,
            self.area_size_x as i32,
            self.area_size_y as i32
        );

        // Define cache path relative to project root
        let cache_dir = "terrain_cache/open_topo";
        let full_cache_path = format!("{}/{}", cache_dir, cache_filename);
        let target_path = Path::new(&full_cache_path);

        let mut file_payload: Option<Vec<u8>> = None;

        // Check if the file already exists in the local cache folder
        if target_path.exists() {
            godot_print!(
                "[Terrain] Cache hit: Laoding terrain data locally from {}",
                full_cache_path
            );
            if let Ok(bytes) = std::fs::read(target_path) {
                file_payload = Some(bytes);
            }
        } else {
            // Fetch it from API if file wasnt found
            // ensure the API key is provided
            if self.api_key.is_empty() {
                godot_error!("[Terrain] Pls enter OpenTopography API-Key");
                return;
            }

            // Construct the API URL for Copernicus 30m global DEM dataset
            let url = format!(
                "https://portal.opentopography.org/API/globaldem?demtype=COP30&south={:.6}&north={:.6}&west={:.6}&east={:.6}&outputFormat=GTiff&API_Key={}",
                south, north, west, east, self.api_key
            );

            godot_print!("[Terrain] Downloading GeoTIFF raster from OpenTopography...");

            let client = Client::new();

            match client.get(&url).send() {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        godot_error!(
                            "[Terrain] API Error: Server returned status {}",
                            &response.status()
                        );
                        if let Ok(text) = response.text() {
                            godot_error!("[Terrain] API Message: {}", text);
                        }
                        return;
                    }

                    if let Ok(bytes) = response.bytes() {
                        let bytes_vec = bytes.to_vec();

                        // Ensire the cache dir exists
                        let _ = create_dir_all(cache_dir);

                        // Create and write downloaded data to local cache file
                        if let Ok(mut file) = File::create(&full_cache_path) {
                            if file.write_all(&bytes_vec).is_ok() {
                                godot_print!(
                                    "[Terrain] Saved downloaded file to {}",
                                    &full_cache_path
                                );
                            }
                        }

                        file_payload = Some(bytes_vec);
                    }
                }
                Err(e) => {
                    godot_error!(
                        "[Terrain] Network Error: Failed to download terrain data: {:?}",
                        e
                    );
                    return;
                }
            }
        }
        // Process binary data
        if let Some(data_stream) = file_payload {
            // wraps raw memoty bytes into virtual file stream with a pointer
            let cursor = Cursor::new(data_stream);

            // Feeds data stram into the TIFF engine to parse the data
            if let Ok(mut tiff_decoder) = Decoder::new(cursor) {
                let (w, h) = tiff_decoder.dimensions().unwrap();
                // Decode Pixel data
                let elevation_data: Vec<f32> = match tiff_decoder.read_image().unwrap() {
                    DecodingResult::F32(vector) => vector,
                    DecodingResult::I16(vector) => {
                        vector.into_iter().map(|val| val as f32).collect()
                    }
                    DecodingResult::U16(vector) => {
                        vector.into_iter().map(|val| val as f32).collect()
                    }
                    _ => {
                        godot_error!(
                            "[Terrain] Parser fault: Image color bit depth configuration is unsupported."
                        );
                        return;
                    }
                };
                // Get absolute path from target cache file path
                let abs_path_str = if let Ok(abs_path) = target_path.canonicalize() {
                    abs_path.to_str().unwrap().to_string()
                } else {
                    full_cache_path.clone()
                };
                self.terrain_data.terrain_path = abs_path_str;
                self.terrain_data.terrain_type = TerrainType::GeoTiff;

                self.update_terrain_geometry(w, h, elevation_data);
            }
        }
    }

    fn laod_tiff_heightmap(&mut self, system_path: &str) {
        let binary_data = match std::fs::read(system_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                godot_error!("Unable to acces path {:?}", e);
                return;
            }
        };

        let cursor = Cursor::new(binary_data);
        if let Ok(mut tiff_decoder) = Decoder::new(cursor) {
            let (w, h) = tiff_decoder.dimensions().unwrap();
            let elevation_data: Vec<f32> = match tiff_decoder.read_image().unwrap() {
                DecodingResult::F32(vector) => vector,
                DecodingResult::I16(vector) => vector.into_iter().map(|val| val as f32).collect(),
                DecodingResult::U16(vector) => vector.into_iter().map(|val| val as f32).collect(),
                _ => {
                    godot_error!(
                        "[Terrain] Parser fault: Image color bit depth configuration is unsupported."
                    );
                    return;
                }
            };

            self.terrain_data.terrain_type = TerrainType::GeoTiff;
            self.terrain_data.terrain_path = system_path.to_string();

            self.update_terrain_geometry(w, h, elevation_data);
            godot_print!("[Terrain] heightmap file succesfully loaded");
        } else {
            godot_error!(
                "[Terrain] File decoding failed. Ensure files matches 32-bit float gryscale standard"
            );
        }
    }

    fn laod_glb_mesh(&mut self, system_path: &str) {
        // Check if older node exists and delete it
        if let Some(mut old_node) = self.base().try_get_node_as::<Node>("TerrainVisualExtern") {
            old_node.queue_free();
        }

        // Hide nodes for procedural grid to avoid overlapping
        let mut visual_node = self.base().get_node_as::<MeshInstance3D>("TerrainVisual");
        visual_node.set_visible(false);

        let mut gltf_document = GltfDocument::new_gd();
        let mut gltf_state = GltfState::new_gd();

        let parser_error = gltf_document.append_from_file(&GString::from(system_path), &gltf_state);

        if parser_error != godot::global::Error::OK {
            godot_error!(
                "[Terrain] GLTF runtime parser failed to append file. Code: {:?}",
                parser_error
            );
            return;
        }

        if let Some(scene_instance) = gltf_document.generate_scene(&gltf_state) {
            let mut mesh_node = scene_instance
                .try_cast::<Node3D>()
                .expect("[Terrain] Root must be Node3D");
            mesh_node.set_name(&StringName::from("TerrainVisualExtern"));

            self.base_mut().add_child(&mesh_node);

            let mut aabb = Aabb::new(Vector3::ZERO, Vector3::ZERO);
            let mut glb_mesh: Option<Gd<Mesh>> = None;
            let mut mesh_inst_glb: Option<Gd<MeshInstance3D>> = None;
            let mut found_mesh = false;

            // check if the root node it self is a MeshInstance3D otherwise search children
            if let Ok(mesh_inst) = mesh_node.clone().try_cast::<MeshInstance3D>() {
                if let Some(mesh) = mesh_inst.get_mesh() {
                    aabb = mesh.get_aabb();
                    glb_mesh = Some(mesh);
                    mesh_inst_glb = Some(mesh_inst);
                    found_mesh = true;
                }
            } else {
                for child in mesh_node.get_children().iter_shared() {
                    if let Ok(mesh_inst) = child.try_cast::<MeshInstance3D>() {
                        if let Some(mesh) = mesh_inst.get_mesh() {
                            aabb = mesh.get_aabb();
                            glb_mesh = Some(mesh);
                            mesh_inst_glb = Some(mesh_inst);
                            found_mesh = true;
                            break;
                        }
                    }
                }
            }

            // Default values for bounding box
            let mut min_x = 0.0;
            let mut max_x = 200.0;
            let mut min_z = 0.0;
            let mut max_z = 200.0;
            let mut max_y = 35.0;

            // Extract size if mesh was succesfully located
            if found_mesh {
                let size = aabb.size;
                let pos = aabb.position;

                min_x = pos.x;
                max_x = pos.x + size.x;

                min_z = pos.z;
                max_z = pos.z + size.z;

                max_y = pos.y + size.y;

                self.area_size_x = (max_x - min_x) as f64;
                self.area_size_y = (max_z - min_z) as f64;
                self.terrain_data.size_x = self.area_size_x;
                self.terrain_data.size_y = self.area_size_y;

                // Create CollisionShape from mesh
                if let Some(mesh) = glb_mesh {
                    if let Some(concave_shape) = mesh.create_trimesh_shape() {
                        let mut shape_node =
                            self.base().get_node_as::<CollisionShape3D>("TerrainShape");

                        shape_node.set_shape(&concave_shape.upcast::<Shape3D>());
                        shape_node.set_position(Vector3::ZERO);
                        shape_node.set_scale(Vector3::ONE);
                    }

                    // Load Texture and Material
                    if let Some(mut mesh_instance) = mesh_inst_glb {
                        let mut material = StandardMaterial3D::new_gd();
                        let texture_res: Gd<Texture2D> = load("res://assets/art/grass.png");

                        material.set_texture(TextureParam::ALBEDO, &texture_res);
                        material.set_roughness(0.8);

                        mesh_instance.set_material_override(&material.upcast::<Material>());
                    }
                }
            } else {
                godot_warn!("[Terrain] No MeshInstance3D found in GLB -> Default Limits");
            }

            self.terrain_data.terrain_path = system_path.to_string();
            self.terrain_data.terrain_type = TerrainType::Glb;

            // Position camera above terrain center
            if let Some(mut camera) = self
                .base()
                .try_get_node_as::<CharacterBody3D>("../PhysicalFlyCam")
            {
                let center_x = (min_x + max_x) / 2.0;
                let center_z = (min_z + max_z) / 2.0;

                camera.set_global_position(Vector3::new(center_x, max_y, center_z));
            }

            // Send Signal: SimState = Running
            if let Some(bus) = self
                .base()
                .try_get_node_as::<SignalBus>("/root/GlobalSignalBus")
            {
                bus.signals()
                    .cam_limit_changed()
                    .emit(max_x, max_z, min_x, min_z);
            } else {
                godot_error!("[Terrain] Couldnt find SignalBus-Node");
            }

            if let Some(mut sim_core) = self.sim_core.clone() {
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::Spectator));

                sim_core.bind_mut().set_terrain(self.terrain_data.clone());
                // reset terrain data
                self.terrain_data = TerrainData::default();
            }
            godot_print!("GLB loaded");
        } else {
        }
    }

    // Process item choice from MenuBar
    #[func]
    fn _on_terrain_menu_selected(&mut self, id: i64) {
        if let Some(mut sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            let sensor_count = sim_core.bind().get_sensor_count();
            if sensor_count != 0 {
                let mut reset_dialog = self.base().get_node_as::<ConfirmationDialog>("../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/ResetConfirmationDialog");
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::Idle));
                reset_dialog.popup_centered();

                self.pending_terrain_id = Some(id);
            } else {
                match id {
                    0 => {
                        let mut config_dialog = self.base().get_node_as::<ConfirmationDialog>(
                    "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog",
                );
                        config_dialog.set_size(Vector2i::new(450, 200));
                        config_dialog.popup_centered();
                    }
                    1 => {
                        let mut file_dialog = self.base().get_node_as::<FileDialog>(
                    "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
                );
                        file_dialog.set_title(&GString::from("Select custom GeoTIFF (.tif)"));
                        file_dialog.set_size(Vector2i::new(650, 400));
                        file_dialog.clear_filters();
                        file_dialog.add_filter(&GString::from(
                            "*.tif, *.tiff ; Professional GeoTIFF / DEM",
                        ));
                        file_dialog.popup_centered();
                    }
                    2 => {
                        let mut file_dialog = self.base().get_node_as::<FileDialog>(
                    "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
                );
                        file_dialog.set_title(&GString::from("Select custom glb-scene"));
                        file_dialog.clear_filters();
                        file_dialog.add_filter(&GString::from("*.glb ; 3D Scene Model"));
                        file_dialog.set_size(Vector2i::new(650, 400));
                        file_dialog.popup_centered();
                    }
                    3 => {
                        let mut file_dialog = self.base().get_node_as::<FileDialog>(
                    "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
                );
                        file_dialog.set_title(&GString::from("Select custom HeightMap Image"));
                        file_dialog.set_size(Vector2i::new(650, 400));
                        file_dialog.clear_filters();
                        file_dialog.add_filter(&GString::from(
                    "*.png, *.jpg, *.jpeg, *.exr, *.tif, *.tiff ; Grayscale Heightmap Images",
                ));
                        file_dialog.popup_centered();
                    }
                    _ => {}
                }
            }
        }
    }

    #[func]
    fn _on_reset_confirmation_dialog_canceled(&mut self) {
        if let Some(mut sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetSimState(SimState::Spectator));
        }
    }

    #[func]
    fn _on_reset_confirmation_dialog_confirmed(&mut self) {
        if let Some(mut sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::ResetSimulation);

            match self.pending_terrain_id {
                Some(0) => {
                    let mut config_dialog = self.base().get_node_as::<ConfirmationDialog>(
                        "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog",
                    );
                    config_dialog.set_size(Vector2i::new(450, 200));
                    config_dialog.popup_centered();
                }
                Some(1) => {
                    let mut file_dialog = self.base().get_node_as::<FileDialog>(
                        "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
                    );
                    file_dialog.set_title(&GString::from("Select custom GeoTIFF (.tif)"));
                    file_dialog.set_size(Vector2i::new(650, 400));
                    file_dialog.clear_filters();
                    file_dialog
                        .add_filter(&GString::from("*.tif, *.tiff ; Professional GeoTIFF / DEM"));
                    file_dialog.popup_centered();
                }
                Some(2) => {
                    let mut file_dialog = self.base().get_node_as::<FileDialog>(
                        "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
                    );
                    file_dialog.set_title(&GString::from("Select custom glb-scene"));
                    file_dialog.clear_filters();
                    file_dialog.add_filter(&GString::from("*.glb ; 3D Scene Model"));
                    file_dialog.set_size(Vector2i::new(650, 400));
                    file_dialog.popup_centered();
                }
                Some(3) => {
                    let mut file_dialog = self.base().get_node_as::<FileDialog>(
                        "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
                    );
                    file_dialog.set_title(&GString::from("Select custom HeightMap Image"));
                    file_dialog.set_size(Vector2i::new(650, 400));
                    file_dialog.clear_filters();
                    file_dialog.add_filter(&GString::from(
                        "*.png, *.jpg, *.jpeg, *.exr, *.tif, *.tiff ; Grayscale Heightmap Images",
                    ));
                    file_dialog.popup_centered();
                }
                _ => {}
            }

            self.pending_terrain_id = None;
        }
    }

    #[func]
    fn _on_topo_dialog_confirmed(&mut self) {
        // Find text line node for API Key
        let api_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog/GridContainer/ApiKeyInput",
        );
        // Find text line node for Latitude
        let lat_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog/GridContainer/LatInput",
        );
        // Find text line node for Longitude
        let lon_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog/GridContainer/LonInput",
        );
        // Find text line node for Width X
        let sizex_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog/GridContainer/SizeXInput",
        );
        // Find text line node for Depth Y
        let sizey_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/OpenTopoDialog/GridContainer/SizeYInput",
        );

        self.api_key = api_box.get_text().to_string();
        self.center_latitude = lat_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(49.7847);
        self.center_longitude = lon_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(9.8742);
        self.area_size_x = sizex_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(200.0);
        self.area_size_y = sizey_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(200.0);

        self.execute_opentopo_download();
    }

    #[func]
    fn _on_terrain_file_dialog_file_selected(&mut self, system_path: GString) {
        let path = system_path.to_string();
        let path_lower = system_path.to_lower();

        let mut file_dialog = self.base().get_node_as::<FileDialog>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TerrainFileDialog",
        );
        let mode = file_dialog.get_title().to_string();

        if mode == "Select custom HeightMap Image" {
            file_dialog.set_title(&GString::from(""));

            let mut image_dialog = self.base().get_node_as::<ConfirmationDialog>(
                "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/ImageHeightmapDialog",
            );
            self.temp_file_path = path;
            image_dialog.set_title(&GString::from("Configure Height Limits"));
            image_dialog.set_size(Vector2i::new(450, 200));
            image_dialog.popup_centered();
            return;
        }

        if path_lower.ends_with(".glb") {
            if let Some(cashed_path) = self.cache_external_file(&path) {
                self.laod_glb_mesh(&cashed_path);
            }
        } else if path_lower.ends_with(".tif") || path_lower.ends_with(".tiff") {
            let path_obj = std::path::Path::new(&path);
            if let Some(filename_str) = path_obj.file_name().and_then(|f| f.to_str()) {
                // Try parsing openTopo cache filename schema using Regex
                // Expected Format: "terrain_{latitude}_{longitude}_{size_x}x{size_y}m.tif" or
                // ".tiff"
                //
                // Pattern syntax:
                // ^                - start of the String
                // $                - very end of the string
                // terrain          - literal match
                // ()               - marks capture groups
                //
                // Character classes
                //  .               - any single char
                //  \d              - any digit
                //  \w              - any alphanumeric char
                //  \s              - any whitespace char
                //  [a-zA-Z]        - custom set
                //
                // Quantifiers
                // *                - 0 or more times
                // +                - at least once
                // ?                - 0 or 1 -> optional
                // {n}              - exactly n times
                // {n,m}            - between n and m times
                let topo_cache_apttern =
                    Regex::new(r"^terrain_(-?\d+\.\d+)_(-?\d+\.\d+)_(\d+)x(\d+)m\.tiff?$").unwrap();

                if let Some(captures) = topo_cache_apttern.captures(filename_str) {
                    if let (Ok(lat), Ok(lon), Ok(size_x), Ok(size_y)) = (
                        captures[1].parse::<f64>(),
                        captures[2].parse::<f64>(),
                        captures[3].parse::<f64>(),
                        captures[4].parse::<f64>(),
                    ) {
                        // 4. Die Parameter im Struct updaten, damit die Simulation mit den korrekten Werten läuft
                        self.center_latitude = lat;
                        self.center_longitude = lon;
                        self.area_size_x = size_x;
                        self.area_size_y = size_y;

                        godot_print!(
                            "[Terrain] Parameters loaded: Lat: {}, Lon: {}, Size: {}x{}m",
                            lat,
                            lon,
                            size_x,
                            size_y
                        );
                    }
                    if let Some(cached_path) = self.cache_external_file(&path) {
                        self.laod_tiff_heightmap(&cached_path);
                    }
                } else {
                    if let Some(cached_path) = self.cache_external_file(&path) {
                        self.temp_file_path = cached_path;
                        let mut config_dialog = self.base().get_node_as::<ConfirmationDialog>(
                            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TiffDialog",
                        );
                        config_dialog.set_size(Vector2i::new(450, 200));
                        config_dialog.popup_centered();
                    }
                }
            } else {
                godot_error!("[Terrain] Format Exception")
            }
        } else {
            godot_error!("[Terrain] Format Exception");
        }
    }

    #[func]
    fn _on_tiff_dialog_confirmed(&mut self) {
        let system_path = self.temp_file_path.clone();
        if system_path.is_empty() {
            godot_error!("[Terrain] No file path catched");
            return;
        }

        let size_x_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TiffDialog/GridContainer/SizeXInput",
        );
        let size_y_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/TiffDialog/GridContainer/SizeYInput",
        );

        self.area_size_x = size_x_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(200.0);
        self.area_size_y = size_y_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(200.0);

        if let Some(cached_path) = self.cache_external_file(&system_path) {
            self.laod_tiff_heightmap(&cached_path);
        }
        self.temp_file_path.clear();
    }

    #[func]
    fn _on_heightmap_dialog_confirmed(&mut self) {
        let system_path = self.temp_file_path.clone();
        if system_path.is_empty() {
            godot_error!("[Terrain] No file path catched");
            return;
        }

        let min_h_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/ImageHeightmapDialog/GridContainer/MinHeightInput",
        );
        let max_h_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/ImageHeightmapDialog/GridContainer/MaxHeightInput",
        );
        let size_x_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/ImageHeightmapDialog/GridContainer/SizeXInput",
        );
        let size_y_box = self.base().get_node_as::<LineEdit>(
            "../CanvasLayer/TopMenuBar/LayoutBox/MenuBar/TerrainMenu/ImageHeightmapDialog/GridContainer/SizeYInput",
        );

        let min_height = min_h_box
            .get_text()
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);
        let max_height = max_h_box
            .get_text()
            .to_string()
            .parse::<f32>()
            .unwrap_or(50.0);

        self.area_size_x = size_x_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(200.0);
        self.area_size_y = size_y_box
            .get_text()
            .to_string()
            .parse::<f64>()
            .unwrap_or(200.0);

        if let Some(cached_path) = self.cache_external_file(&system_path) {
            self.load_generic_image_heightmap(&cached_path, min_height, max_height);
        }
        self.temp_file_path.clear();
    }

    fn load_generic_image_heightmap(&mut self, system_path: &str, min_h: f32, max_h: f32) {
        let mut image = Image::new_gd();

        if image.load(&GString::from(system_path)) != godot::global::Error::OK {
            godot_error!(
                "[Terrain] Unable to load image file from path: {}",
                system_path
            );
            return;
        }

        let w = image.get_width() as u32;
        let h = image.get_height() as u32;
        let mut elevation_data = Vec::with_capacity((w * h) as usize);

        for y in 0..h {
            for x in 0..w {
                let color = image.get_pixel(x as i32, y as i32);
                let brightness = color.luminance();
                let final_height = min_h + (brightness as f32) * (max_h - min_h);
                elevation_data.push(final_height);
            }
        }

        self.terrain_data.terrain_path = system_path.to_string();
        self.terrain_data.max_height = Some(max_h);
        self.terrain_data.min_height = Some(min_h);
        self.terrain_data.terrain_type = TerrainType::ImageHeightmap;

        self.update_terrain_geometry(w, h, elevation_data);
    }
}
