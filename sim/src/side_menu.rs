use std::collections::HashMap;

use godot::{
    classes::{
        AnimationPlayer, Button, Control, IControl, LineEdit, PanelContainer, ResourceLoader,
        VBoxContainer,
    },
    prelude::*,
};

use crate::{
    list_item::ListItem,
    signal_bus::{SignalBus, SimState},
    sim_core::{SimCore, SimEvent},
};

#[derive(GodotClass)]
#[class(base=Control)]
pub struct SideMenu {
    sim_core: Option<Gd<SimCore>>,
    sim_state: SimState,
    base: Base<Control>,
    is_expanded: bool,
    item_scene: Option<Gd<PackedScene>>,
}

#[godot_api]
impl IControl for SideMenu {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            sim_core: None,
            is_expanded: false,
            sim_state: SimState::Idle,
            item_scene: None,
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
                .sim_state_changed()
                .connect_other(&self.to_gd(), Self::on_sim_state_changed);

            signal_bus_node
                .signals()
                .on_garage_ex_changed()
                .connect_other(&self.to_gd(), Self::on_garage_ex_changed);

            signal_bus_node
                .signals()
                .new_object_spawned()
                .connect_other(&self.to_gd(), Self::on_new_object_spawned);
        }

        if let Some(sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            self.sim_core = Some(sim_core)
        }

        if let Some(scene) =
            ResourceLoader::singleton().load("res://assets/scenes/editor_list_item.tscn")
        {
            self.item_scene = Some(scene.cast::<PackedScene>());
        } else {
            godot_error!("[SideMenu] Couldnt load item scene");
        }
    }
}

#[godot_api]
impl SideMenu {
    #[func]
    fn _on_toggle_button_pressed(&mut self) {
        let mut animation_player = self
            .base()
            .get_node_as::<AnimationPlayer>("AnimationPlayer");
        if self.is_expanded {
            animation_player
                .play_backwards_ex()
                .name("toggle_menu")
                .done();

            self.is_expanded = false;
        } else {
            animation_player.play_ex().name("toggle_menu").done();
            self.is_expanded = true;
        }
    }

    #[func]
    fn _on_add_garage_pressed(&mut self) {
        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetSimState(SimState::GarageEditor));
        }
    }

    #[func]
    fn _on_add_uwb_pressed(&mut self) {
        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetSimState(SimState::NodeEditor));
        }
    }

    fn on_new_object_spawned(&mut self, is_sensor: bool, name: String, id: u32) {
        let Some(ref packed_scene) = self.item_scene else {
            godot_error!("[SideMenu] Couldnt find ListItem scene");
            return;
        };

        let container_path = if is_sensor {
            "HBox/MenuPanel/VBox/UWBScroll/UWBList"
        } else {
            "HBox/MenuPanel/VBox/GarageList"
        };

        let mut list_container = self.base().get_node_as::<VBoxContainer>(container_path);

        let Some(instance) = packed_scene.instantiate() else {
            godot_error!("[SideMenu] Couldnt instantiate list item");
            return;
        };

        let mut item = instance.cast::<ListItem>();
        item.set_visible(true);
        item.bind_mut().object_id = id;
        item.get_node_as::<LineEdit>("HBox/PanelContainer/NameEdit")
            .set_text(&GString::from(&name));

        list_container.add_child(&item);
        godot_print!(
            "[SideMenu] List item added for ID {} ({})",
            id,
            container_path
        );
    }

    fn on_garage_ex_changed(&mut self, exists: bool) {
        self.base()
            .get_node_as::<Button>("HBox/MenuPanel/VBox/TitleBarGarage/AddGarage")
            .set_disabled(exists);
    }

    fn on_sim_state_changed(&mut self, new_state: SimState) {
        self.sim_state = new_state;

        godot_print!("[SideMenu] New State: {:?}", new_state);

        let mut animation_player = self
            .base()
            .get_node_as::<AnimationPlayer>("AnimationPlayer");

        if self.base().is_visible() {
            match new_state {
                SimState::Spectator => {
                    animation_player.play_ex().name("to_spectator").done();
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/NodeTab")
                        .set_position(Vector2::new(0.0, 35.0));
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/GarageTab")
                        .set_position(Vector2::new(0.0, 35.0));
                }
                SimState::NodeEditor => {
                    animation_player.play_ex().name("to_node").done();
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/SpectatorTab")
                        .set_position(Vector2::new(0.0, 35.0));
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/GarageTab")
                        .set_position(Vector2::new(0.0, 35.0));
                }
                SimState::GarageEditor => {
                    animation_player.play_ex().name("to_garage").done();
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/NodeTab")
                        .set_position(Vector2::new(0.0, 35.0));
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/SpectatorTab")
                        .set_position(Vector2::new(0.0, 35.0));
                }
                SimState::Idle => {
                    self.base_mut().set_visible(false);
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/NodeTab")
                        .set_position(Vector2::new(0.0, 35.0));
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/GarageTab")
                        .set_position(Vector2::new(0.0, 35.0));
                    self.base()
                        .get_node_as::<PanelContainer>("HBox/MenuPanel/VBox/TitleBar/SpectatorTab")
                        .set_position(Vector2::new(0.0, 0.0));
                }

                _ => {}
            }
        }
    }
}
