use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct UwbNode {
    base: Base<Node3D>,
    id: u32,
}

#[godot_api]
impl INode3D for UwbNode {
    fn init(base: Base<Node3D>) -> Self {
        Self { base, id: 50000 }
    }
}

#[godot_api]
impl UwbNode {
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }
}
