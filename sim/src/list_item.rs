use godot::{
    classes::{IPanelContainer, LineEdit, PanelContainer},
    prelude::*,
};

use crate::sim_core::{SimCore, SimEvent};

#[derive(GodotClass)]
#[class(base=PanelContainer)]
pub struct ListItem {
    pub object_id: u32,

    base: Base<PanelContainer>,
    sim_core: Option<Gd<SimCore>>,
}

#[godot_api]
impl IPanelContainer for ListItem {
    fn init(base: Base<PanelContainer>) -> Self {
        Self {
            base,
            object_id: 0,
            sim_core: None,
        }
    }

    fn ready(&mut self) {
        if let Some(sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            self.sim_core = Some(sim_core)
        }
    }
}

#[godot_api]
impl ListItem {
    #[func]
    fn _on_name_edit_text_submitted(&mut self, new_text: GString) {
        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::ItemNameChanged(
                    self.object_id,
                    new_text.to_string(),
                ));
        }

        let mut line_edit = self
            .base()
            .get_node_as::<LineEdit>("HBox/PanelContainer/NameEdit");
        line_edit.release_focus();
    }

    #[func]
    fn _on_delete_button_pressed(&mut self) {
        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::RemoveListItem(self.object_id));
        }
        self.base_mut().queue_free();
    }
}
