use godot::{prelude::*, signal};

#[derive(GodotConvert, Var, Export, Copy, Clone, PartialEq, Eq, Default, Debug)]
#[godot(via = GString)]
pub enum SimState {
    #[default]
    Idle,
    Spectator,
    GarageEditor,
    NodeEditor,
}

impl SimState {
    pub fn to_string(&self) -> String {
        let text = match self {
            SimState::Idle => "Idle",
            SimState::Spectator => "Spectator",
            SimState::NodeEditor => "Editor",
            SimState::GarageEditor => "Editor",
        };
        String::from(text)
    }

    pub fn get_color(&self) -> Color {
        match self {
            SimState::Idle => Color::from_rgb(0.7, 0.7, 0.7),
            SimState::Spectator => Color::from_rgb(0.2, 0.8, 0.2),
            SimState::NodeEditor => Color::from_rgb(0.9, 0.5, 0.1),
            SimState::GarageEditor => Color::from_rgb(0.9, 0.5, 0.1),
        }
    }
}

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct SignalBus {
    base: Base<Node>,
}

#[godot_api]
impl SignalBus {
    #[signal]
    pub fn cam_limit_changed(max_x: f32, max_y: f32, min_x: f32, min_y: f32);
    #[signal]
    pub fn sim_state_changed(new_state: SimState);
    #[signal]
    pub fn project_item_state_changed(id: i32, disabled: bool);
    #[signal]
    pub fn sim_config_loaded(sensors: Dictionary<GString, Vector3>, garage: Variant);
    #[signal]
    pub fn on_garage_ex_changed(exists: bool);
    #[signal]
    pub fn new_object_spawned(sensor: bool, name: String, id: u32);
}
