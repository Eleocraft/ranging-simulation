use godot::classes::{
    ConfirmationDialog, Control, FileDialog, IPanelContainer, Input, InputEvent, Label,
    PanelContainer, PopupMenu,
};
use godot::prelude::*;

use crate::side_menu::SideMenu;
use crate::signal_bus::{SignalBus, SimState};
use crate::sim_core::{SimCore, SimEvent};

#[derive(GodotClass)]
#[class(base=PanelContainer)]
pub struct SimMenu {
    sim_core: Option<Gd<SimCore>>,
    sim_state: SimState,
    base: Base<PanelContainer>,
}

#[godot_api]
impl IPanelContainer for SimMenu {
    fn init(base: Base<PanelContainer>) -> Self {
        Self {
            sim_core: None,
            sim_state: SimState::Idle,
            base,
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
                .project_item_state_changed()
                .connect_other(&self.to_gd(), Self::on_project_item_state_changed);
        }

        if let Some(sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            self.sim_core = Some(sim_core)
        }
    }
    // processes input that havent been consumed by UI
    fn unhandled_input(&mut self, _event: Gd<InputEvent>) {
        if self.sim_state == SimState::Idle {
            return;
        }

        let input = Input::singleton();

        if let Some(mut sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            if input.is_action_just_pressed("spectator") && self.sim_state != SimState::Spectator {
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::Spectator));
            } else if input.is_action_just_pressed("node_editor")
                && self.sim_state != SimState::NodeEditor
            {
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::NodeEditor));
            } else if input.is_action_just_pressed("simulation")
                && self.sim_state != SimState::Simulation
            {
                godot_print!("test");
                sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::Simulation));
            }
        }
    }
}

#[godot_api]
impl SimMenu {
    fn on_sim_state_changed(&mut self, new_mode: SimState) {
        self.sim_state = new_mode;
        godot_print!("[MENU] new sim state {:?}", new_mode);

        if let Some(mut label) = self.base().try_get_node_as::<Label>("LayoutBox/Label") {
            let formatted_label_text = format!("-- {} --", self.sim_state.to_string());
            label.set_text(&formatted_label_text);

            let mode_color: Color = self.sim_state.get_color();

            label.add_theme_color_override("font_color", mode_color);
        }

        if let Some(mut crosshair) = self.base().try_get_node_as::<Control>("../Crosshair") {
            let is_editor =
                self.sim_state == SimState::NodeEditor || self.sim_state == SimState::GarageEditor;
            crosshair.set_visible(is_editor);
        }
        if new_mode != SimState::Idle {
            self.base()
                .get_node_as::<SideMenu>("../SideMenu")
                .set_visible(true);
        } else {
            self.base()
                .get_node_as::<SideMenu>("../SideMenu")
                .set_visible(false);
        }
    }

    fn on_project_item_state_changed(&mut self, id: i32, disabled: bool) {
        let mut project_menu = self
            .base()
            .get_node_as::<PopupMenu>("LayoutBox/MenuBar/Project");
        project_menu.set_item_disabled(id, disabled);
    }

    #[func]
    fn _on_close_button_pressed(&mut self) {
        self.base().get_tree().quit();
    }

    #[func]
    fn _on_workspace_id_pressed(&mut self, id: i64) {
        if self.sim_state == SimState::Idle {
            return;
        }

        if let Some(mut sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            match id {
                0 => sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::Spectator)),
                1 => sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::NodeEditor)),
                2 => sim_core
                    .bind_mut()
                    .push_sim_event(SimEvent::SetSimState(SimState::Simulation)),
                _ => {}
            }
        } else {
            godot_error!("No SimCore was found");
        }
    }

    #[func]
    fn _on_project_id_pressed(&mut self, id: i64) {
        match id {
            0 => {
                let mut file_dialog = self
                    .base()
                    .get_node_as::<FileDialog>("LayoutBox/MenuBar/Project/ProjectFileDialog");

                // Set the operation mode to saving single file
                file_dialog.set_file_mode(godot::classes::file_dialog::FileMode::SAVE_FILE);

                // Clear previous filters
                file_dialog.clear_filters();

                file_dialog.set_title(&GString::from("Save Current Project As..."));

                // Add filter for .json
                file_dialog
                    .add_filter(&GString::from("*.json ; Project Simulation Configurations"));
                file_dialog.set_size(Vector2i::new(650, 400));
                file_dialog.popup_centered();
            }
            1 => {
                if let Some(mut sim_core) = self.sim_core.clone() {
                    sim_core
                        .bind_mut()
                        .push_sim_event(SimEvent::SaveConfigFile(String::new()));
                }
            }
            2 => {
                if let Some(mut sim_core) = self
                    .base()
                    .try_get_node_as::<SimCore>("/root/GlobalSimCore")
                {
                    let sensor_count = sim_core.bind().get_sensor_count();
                    if sensor_count != 0 {
                        let mut reset_dialog = self.base().get_node_as::<ConfirmationDialog>(
                            "LayoutBox/MenuBar/Project/ResetConfirmationDialog",
                        );
                        sim_core
                            .bind_mut()
                            .push_sim_event(SimEvent::SetSimState(SimState::Idle));
                        reset_dialog.popup_centered();
                    } else {
                        let mut file_dialog = self.base().get_node_as::<FileDialog>(
                            "LayoutBox/MenuBar/Project/ProjectFileDialog",
                        );

                        // Set the operation mode to saving single file
                        file_dialog.set_file_mode(godot::classes::file_dialog::FileMode::OPEN_FILE);

                        // Clear previous filters
                        file_dialog.clear_filters();

                        file_dialog.set_title(&GString::from("Open Project Configuration..."));

                        // Add filter for .json
                        file_dialog.add_filter(&GString::from(
                            "*.json ; Project Simulation Configurations",
                        ));
                        file_dialog.set_size(Vector2i::new(650, 400));
                        file_dialog.popup_centered();
                    }
                }
            }

            _ => {}
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

            let mut file_dialog = self
                .base()
                .get_node_as::<FileDialog>("LayoutBox/MenuBar/Project/ProjectFileDialog");

            // Set the operation mode to saving single file
            file_dialog.set_file_mode(godot::classes::file_dialog::FileMode::OPEN_FILE);

            // Clear previous filters
            file_dialog.clear_filters();

            file_dialog.set_title(&GString::from("Open Project Configuration..."));

            // Add filter for .json
            file_dialog.add_filter(&GString::from("*.json ; Project Simulation Configurations"));
            file_dialog.set_size(Vector2i::new(650, 400));
            file_dialog.popup_centered();
        }
    }

    #[func]
    fn _on_project_file_dialog_file_selected(&mut self, system_path: GString) {
        let path = system_path.to_string();

        let file_dialog_mode = self
            .base()
            .get_node_as::<FileDialog>("LayoutBox/MenuBar/Project/ProjectFileDialog")
            .get_file_mode();

        match file_dialog_mode {
            godot::classes::file_dialog::FileMode::SAVE_FILE => {
                if let Some(mut sim_core) = self.sim_core.clone() {
                    sim_core
                        .bind_mut()
                        .push_sim_event(SimEvent::SaveConfigFile(path));
                }
            }
            godot::classes::file_dialog::FileMode::OPEN_FILE => {
                if let Some(mut sim_core) = self.sim_core.clone() {
                    sim_core
                        .bind_mut()
                        .push_sim_event(SimEvent::LoadConfigFile(path));
                }
            }

            _ => {}
        }
    }
}
