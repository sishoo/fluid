pub struct Vec2 {
    x: f32,
    y: f32
}
pub type Vertex2 = Vec2;

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: x,
            y: x
        }
    }
}