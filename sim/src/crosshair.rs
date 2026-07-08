use godot::{
    classes::{Control, IControl},
    prelude::*,
};

#[derive(GodotClass)]
#[class(base=Control)]
pub struct Crosshair {
    base: Base<Control>,
}

#[godot_api]
impl IControl for Crosshair {
    fn init(base: Base<Control>) -> Self {
        Self { base }
    }

    fn draw(&mut self) {
        let color = Color::from_rgba(1.0, 0.45, 0.0, 0.6);
        let line_width = 1.5;

        let size = self.base().get_size();
        let c = size / 2.0;

        // left to center
        self.base_mut()
            .draw_line_ex(c + Vector2::new(-15.0, 0.0), c, color)
            .width(line_width)
            .done();

        // right to center
        self.base_mut()
            .draw_line_ex(c, c + Vector2::new(15.0, 0.0), color)
            .width(line_width)
            .done();

        // top to center
        self.base_mut()
            .draw_line_ex(c + Vector2::new(0.0, -15.0), c, color)
            .width(line_width)
            .done();

        // bottom to center
        self.base_mut()
            .draw_line_ex(c, c + Vector2::new(0.0, 15.0), color)
            .width(line_width)
            .done();
    }
}
