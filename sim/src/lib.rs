use godot::{classes::ClassDb, prelude::*};

mod cam3d;
mod crosshair;
mod list_item;
mod menu;
mod side_menu;
mod signal_bus;
mod sim_core;
mod simulation;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
