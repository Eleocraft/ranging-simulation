use std::usize;

use godot::{
    classes::{Button, HBoxContainer, HSlider, IHBoxContainer, IHSlider, Label},
    prelude::*,
};

use crate::signal_bus::{SignalBus, SimState};
use crate::sim_core::{SimCore, SimEvent};
use crate::sim_logic::{PlaybackState, SimCom};

#[derive(GodotClass)]
#[class(base=HBoxContainer)]
pub struct SimPanel {
    base: Base<HBoxContainer>,
    sim_core: Option<Gd<SimCore>>,
    sim_state: SimState,
    allowed_values: [f64; 9],
    speed: f64,
    playback_state: PlaybackState,
    frame_counter: u8,
}

#[godot_api]
impl IHBoxContainer for SimPanel {
    fn init(base: Base<HBoxContainer>) -> Self {
        Self {
            base,
            sim_core: None,
            sim_state: SimState::Idle,
            allowed_values: [
                50.0, 100.0, 250.0, 500.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0,
            ],
            speed: 50.0,
            playback_state: PlaybackState::Stopped,
            frame_counter: 0,
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
        }

        // get sim core
        if let Some(sim_core) = self
            .base()
            .try_get_node_as::<SimCore>("/root/GlobalSimCore")
        {
            self.sim_core = Some(sim_core)
        }

        // Get slider and configure
        let mut slider = self.base_mut().get_node_as::<HSlider>("SimSpeed/Slider");
        slider.set_min(0.0);
        slider.set_max((self.allowed_values.len() - 1) as f64);
        slider.set_step(1.0);
    }
}

#[godot_api]
impl SimPanel {
    #[func]
    fn _on_slider_value_changed(&mut self, index: f64) {
        let speed = self.allowed_values[(index as usize)];
        self.speed = speed;
        let speed_string = format!("{} μs/Frame", speed);

        let mut label = self.base().get_node_as::<Label>("SimSpeed/Label");
        label.set_text(&GString::from(&speed_string));
    }

    #[func]
    fn _on_slider_drag_ended(&mut self, _index: f64) {
        if let Some(mut sim_core) = self.sim_core.clone() {
            godot_print!("[SimPanel] Slider drag ended");
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetSimSpeed(self.speed as u32));
        }
    }

    #[func]
    fn _on_play_pressed(&mut self) {
        let mut play_button = self.base().get_node_as::<Button>("SimSpeed/Play");
        let mut stop_button = self.base().get_node_as::<Button>("SimSpeed/Stop");

        if self.playback_state == PlaybackState::Stopped {
            play_button.set_text(&GString::from("‖"));
            stop_button.set_disabled(false);
            self.playback_state = PlaybackState::Playing;
        } else if self.playback_state == PlaybackState::Paused {
            play_button.set_text(&GString::from("‖"));
            stop_button.set_disabled(false);
            self.playback_state = PlaybackState::Playing;
        } else if self.playback_state == PlaybackState::Playing {
            play_button.set_text(&GString::from("▶"));
            stop_button.set_disabled(false);
            self.playback_state = PlaybackState::Paused;
        }

        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetPlaybackState(self.playback_state));
        }
    }

    #[func]
    fn _on_stop_pressed(&mut self) {
        let mut play_button = self.base().get_node_as::<Button>("SimSpeed/Play");
        let mut stop_button = self.base().get_node_as::<Button>("SimSpeed/Stop");

        play_button.set_text(&GString::from("▶"));
        stop_button.set_disabled(true);
        self.playback_state = PlaybackState::Stopped;

        if let Some(mut sim_core) = self.sim_core.clone() {
            sim_core
                .bind_mut()
                .push_sim_event(SimEvent::SetPlaybackState(PlaybackState::Stopped));
        }
    }

    fn on_sim_state_changed(&mut self, new_state: SimState) {
        self.sim_state = new_state;
        if new_state == SimState::Simulation {
            self.base_mut().set_visible(true);
        } else {
            self.base_mut().set_visible(false);
        }
    }
}
