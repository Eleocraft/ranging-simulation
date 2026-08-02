use godot::classes::{
    Camera3D, CharacterBody3D, ICharacterBody3D, Input, InputEvent, InputEventMouseButton,
    InputEventMouseMotion, input::MouseMode,
};
use godot::classes::{PhysicsRayQueryParameters3D, PhysicsServer3D, ResourceLoader};
use godot::global::MouseButton;
use godot::prelude::*;

use crate::signal_bus::{SignalBus, SimState};
use crate::sim_core::{SimCore, SimEvent};
use crate::sim_types::SimObjectData;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SpawnType {
    Sensor,
    Garage,
}

#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct PhysicalFlyCam {
    #[export]
    movement_speed: f32,
    #[export]
    mouse_sens: f32,

    blueprint_sensor: Option<Gd<PackedScene>>,
    blueprint_garage: Option<Gd<PackedScene>>,
    preview_instance: Option<Gd<Node3D>>,
    yaw: f32,
    pitch: f32,

    next_id: u32,

    sim_state: SimState,

    garage_placed: bool,

    limit_x_max: f32,
    limit_z_max: f32,
    limit_x_min: f32,
    limit_z_min: f32,

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl PhysicalFlyCam {
    fn on_cam_limit_changed(&mut self, max_x: f32, max_y: f32, min_x: f32, min_y: f32) {
        self.limit_x_max = max_x;
        self.limit_z_max = max_y;
        self.limit_x_min = min_x;
        self.limit_z_min = min_y;
        godot_print!(
            "[CAM] limitations set to: X: {} -> {}, Y: {} -> {}",
            min_x,
            max_x,
            min_y,
            max_y
        );
    }

    fn on_sim_state_changed(&mut self, new_state: SimState) {
        self.sim_state = new_state;
        if let Some(mut preview) = self.preview_instance.take() {
            preview.queue_free();
        }
    }

    fn on_sim_config_loaded(&mut self, sensors: Dictionary<GString, Vector3>, garage_var: Variant) {
        for (name, point) in sensors.iter_shared() {
            let name: String = name.to_string();
            if !self.spawn_object_at(point, SpawnType::Sensor, name.clone()) {
                godot_error!("[CAM] Error whily spawning sensor '{}' at {}", name, point);
            }
        }

        if let Ok(garage_dic) = garage_var.try_to::<Dictionary<GString, Variant>>() {
            if let (Some(name_var), Some(pos_var)) =
                (garage_dic.get("name"), garage_dic.get("position"))
            {
                let name: String = name_var.try_to().unwrap_or("Gerage".to_string());
                let pos: Vector3 = pos_var.try_to().unwrap_or(Vector3::ZERO);

                if !self.spawn_object_at(pos, SpawnType::Garage, name.clone()) {
                    godot_error!("[CAM] Error whily spawning gerage '{}' at {}", name, &pos);
                }
            }
        }
    }

    fn on_garage_ex_changed(&mut self, exists: bool) {
        godot_print!("[CAM] garage existance change to {}", &exists);
        self.garage_placed = exists;
    }

    fn raycast_from_cam_center(&mut self) -> Option<Vector3> {
        // Get camera3d child node
        let camera = self.base().get_node_as::<Camera3D>("Camera3D");

        // viewport resolution to get cetner coordinates of the screen
        let viewport_size = camera.get_viewport().unwrap().get_visible_rect().size;
        let screen_center = viewport_size / 2.0;

        let ray_origin = camera.project_ray_origin(screen_center);

        // Projection to 3D-direction
        // 1) Converts screen pixels into Normalized Device Coordinates (NDC) in [-1, 1]
        // 2) Scales according to the field of view: f = tan(FOV/2)
        // 3) Compensates screen disportion using aspect ratio A = W/H
        //
        // v_lokal = [(NDC_x * f * A), (NDC_y * f), -1]
        //
        // 4) Transforms the local vector using the cameras rotation matrix
        // 5) Normakizes final vector
        let ray_direction = camera.project_ray_normal(screen_center);

        // Maximum distance the ray is travelliung through the sceen
        let max_distance = 400.0;

        // Global 3D endpoint using linear equation
        let ray_end = ray_origin + (ray_direction * max_distance);

        // Fetch the World3D ressource (global 3D context of the scene containing physics space,
        // environmnt, navigation map, etc)
        let world_3d = camera.get_world_3d()?;

        // Access the global PhysicsServer3D and request the direct state for worlds physics space
        // get_space() returns the RID (handle) of the physics space (= physics simulation instance)
        let mut space_state =
            PhysicsServer3D::singleton().space_get_direct_state(world_3d.get_space())?;

        // Origin and endpoint are packed into raycast query object
        let query = PhysicsRayQueryParameters3D::create(ray_origin, ray_end)?;

        // Cast the ray and perform rapid ray-triangle interseciton tests agains the terrains
        // CollisionShape
        let result = space_state.intersect_ray(&query);

        if !result.is_empty() {
            result.get("position").map(|v| v.to::<Vector3>())
        } else {
            return None;
        }
    }

    fn spawn_object_at(&mut self, point: Vector3, spawn_type: SpawnType, name: String) -> bool {
        if spawn_type == SpawnType::Garage && self.garage_placed {
            return false;
        }
        let blueprint_scene = match spawn_type {
            SpawnType::Sensor => &self.blueprint_sensor,
            SpawnType::Garage => &self.blueprint_garage,
        };

        let Some(scene) = blueprint_scene else {
            return false;
        };

        let Some(instance) = scene.instantiate() else {
            return false;
        };

        let mut sensor_node = instance.cast::<Node3D>();
        sensor_node.set_visible(true);

        if let Some(mut current_scene) = self.base().get_tree().get_current_scene() {
            current_scene.add_child(&sensor_node);
            sensor_node.set_global_position(point);
        }

        let mut sim_core = self.base().get_node_as::<SimCore>("/root/GlobalSimCore");

        match spawn_type {
            SpawnType::Sensor => {
                let id = self.next_id;
                self.next_id += 1;
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::AddSensor(SimObjectData::new(
                        id,
                        name,
                        point,
                        sensor_node,
                    )));
            }
            SpawnType::Garage => {
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::AddGarage(SimObjectData::new(
                        0,
                        name,
                        point,
                        sensor_node,
                    )));
            }
        }

        true
    }
}

#[godot_api]
impl ICharacterBody3D for PhysicalFlyCam {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            movement_speed: 15.0,
            mouse_sens: 0.15,
            yaw: 0.0,
            pitch: 0.0,
            sim_state: SimState::Idle,
            limit_x_max: 100.0,
            limit_z_max: 100.0,
            limit_x_min: 0.0,
            limit_z_min: 0.0,
            next_id: 10,
            base,
            blueprint_sensor: None,
            blueprint_garage: None,
            preview_instance: None,
            garage_placed: false,
        }
    }

    fn ready(&mut self) {
        // Connect to global SignalBus
        if let Some(signal_bus_node) = self
            .base()
            .try_get_node_as::<SignalBus>("/root/GlobalSignalBus")
        {
            signal_bus_node
                .signals()
                .cam_limit_changed()
                .connect_other(&self.to_gd(), Self::on_cam_limit_changed);

            signal_bus_node
                .signals()
                .sim_state_changed()
                .connect_other(&self.to_gd(), Self::on_sim_state_changed);

            signal_bus_node
                .signals()
                .sim_config_loaded()
                .connect_other(&self.to_gd(), Self::on_sim_config_loaded);

            signal_bus_node
                .signals()
                .on_garage_ex_changed()
                .connect_other(&self.to_gd(), Self::on_garage_ex_changed);
        }

        // load sensor blueprint
        if let Some(scene) =
            ResourceLoader::singleton().load("res://assets/scenes/sensor_blueprint.tscn")
        {
            self.blueprint_sensor = Some(scene.cast::<PackedScene>());
        } else {
            godot_error!("[CAM] Couldnt load sensor blueprint scene");
        }

        // load garage blueprint
        if let Some(scene) =
            ResourceLoader::singleton().load("res://assets/scenes/garage_blueprint.tscn")
        {
            self.blueprint_garage = Some(scene.cast::<PackedScene>());
        } else {
            godot_error!("[CAM] Couldnt load garage blueprint scene")
        }
    }

    // Processes input events that havnt been consumed by UI
    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        // if sim state is idle return
        if self.sim_state == SimState::Idle {
            return;
        }

        let mut input = Input::singleton();

        if input.is_action_just_pressed("ui_cancel") {
            input.set_mouse_mode(MouseMode::VISIBLE);
            return;
        }

        if let Ok(mouse_button) = event.clone().try_cast::<InputEventMouseButton>() {
            if mouse_button.is_pressed() && mouse_button.get_button_index() == MouseButton::LEFT {
                if input.get_mouse_mode() == MouseMode::CAPTURED {
                    if self.sim_state == SimState::NodeEditor {
                        let looking_at = self.raycast_from_cam_center();
                        if let Some(point) = looking_at {
                            if !self.spawn_object_at(
                                point,
                                SpawnType::Sensor,
                                format!("Sensor_{}", self.next_id),
                            ) {
                                godot_error!("[CAM] Couldnt spawn sensor");
                            }
                        }
                    } else if self.sim_state == SimState::GarageEditor {
                        let looking_at = self.raycast_from_cam_center();
                        if let Some(point) = looking_at {
                            if !self.spawn_object_at(
                                point,
                                SpawnType::Garage,
                                String::from("Garage"),
                            ) {
                                godot_error!("[CAM] Couldnt spawn garage");
                            }
                        }
                    }
                } else {
                    input.set_mouse_mode(MouseMode::CAPTURED);
                }
            }

            if input.get_mouse_mode() == MouseMode::CAPTURED {
                let button_index = mouse_button.get_button_index();

                if button_index == MouseButton::WHEEL_UP {
                    self.movement_speed += 2.0;
                    self.movement_speed = self.movement_speed.clamp(5.0, 100.0);
                } else if button_index == MouseButton::WHEEL_DOWN {
                    self.movement_speed -= 2.0;
                    self.movement_speed = self.movement_speed.clamp(5.0, 100.0);
                }
            }
        }

        if input.get_mouse_mode() != MouseMode::CAPTURED {
            return;
        }

        // Check if the input event is a mmouse movement_speed
        if let Ok(mouse_motion) = event.try_cast::<InputEventMouseMotion>() {
            let relative = mouse_motion.get_relative();

            // Accumulate movement multiplied by sens
            self.yaw -= relative.x * self.mouse_sens;
            self.pitch -= relative.y * self.mouse_sens;

            self.pitch = self.pitch.clamp(-89.0, 89.0);

            // Apply pitch to physical body
            let yaw_rad = self.yaw.to_radians();
            self.base_mut()
                .set_rotation(Vector3::new(0.0, yaw_rad, 0.0));

            // Apply vertical rotation only to the camera child
            if let Some(mut camera) = self.base().try_get_node_as::<Camera3D>("Camera3D") {
                camera.set_rotation(Vector3::new(self.pitch.to_radians(), 0.0, 0.0));
            }
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        let input = Input::singleton();

        // Only allow movement if mouse is in captured MouseMode
        if input.get_mouse_mode() != MouseMode::CAPTURED {
            // Stop movement if mouse is free
            self.base_mut().set_velocity(Vector3::ZERO);
            self.base_mut().move_and_slide();
            return;
        }

        let mut input_dir = Vector3::ZERO;

        let transform = self.base().get_global_transform();
        let forward = -transform.basis.col_c();
        let right = transform.basis.col_a();
        let up = Vector3::UP;

        // Poll kexboard input and accumulate vctors
        if input.is_action_pressed("forward") {
            input_dir += forward;
        }
        if input.is_action_pressed("backward") {
            input_dir -= forward;
        }
        if input.is_action_pressed("right") {
            input_dir += right;
        }
        if input.is_action_pressed("left") {
            input_dir -= right;
        }
        if input.is_action_pressed("up") {
            input_dir += up;
        }
        if input.is_action_pressed("down") {
            input_dir -= up;
        }

        // Normalize:
        if input_dir != Vector3::ZERO {
            input_dir = input_dir.normalized();
        }

        let target_velocity = input_dir * self.movement_speed;
        self.base_mut().set_velocity(target_velocity);
        self.base_mut().move_and_slide();

        // Check limitations
        let mut current_pos = self.base().get_global_position();
        current_pos.x = current_pos
            .x
            .clamp(self.limit_x_min + 0.5, self.limit_x_max - 0.5);
        current_pos.z = current_pos
            .z
            .clamp(self.limit_z_min + 0.5, self.limit_z_max - 0.5);

        self.base_mut().set_global_position(current_pos);

        // Check if mode: node-editor is active for raycasting
        if self.sim_state == SimState::NodeEditor {
            if self.preview_instance.is_none() {
                if let Some(ref scene) = self.blueprint_sensor {
                    if let Some(instance) = scene.instantiate() {
                        let mut preview = instance.cast::<Node3D>();
                        preview.set_visible(true);

                        if let Some(mut current_scene) = self.base().get_tree().get_current_scene()
                        {
                            current_scene.add_child(&preview);
                        }
                        self.preview_instance = Some(preview);
                    }
                }
            }

            let looking_at: Option<Vector3> = self.raycast_from_cam_center();
            if let Some(position) = looking_at {
                if let Some(ref mut preview) = self.preview_instance {
                    preview.set_global_position(position);
                }
            }
        } else if self.sim_state == SimState::GarageEditor {
            if self.preview_instance.is_none() {
                if let Some(ref scene) = self.blueprint_garage {
                    if let Some(instance) = scene.instantiate() {
                        let mut preview = instance.cast::<Node3D>();
                        preview.set_visible(true);

                        if let Some(mut current_scene) = self.base().get_tree().get_current_scene()
                        {
                            current_scene.add_child(&preview);
                        }
                        self.preview_instance = Some(preview);
                    }
                }
            }

            let looking_at: Option<Vector3> = self.raycast_from_cam_center();
            if let Some(position) = looking_at {
                if let Some(ref mut preview) = self.preview_instance {
                    preview.set_global_position(position);
                }
            }
        } else {
            if let Some(mut preview) = self.preview_instance.take() {
                preview.queue_free();
            }
        }
    }
}
