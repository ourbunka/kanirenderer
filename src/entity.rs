pub struct Entity {
    pub model: model::Model,
    pub position: Point3<f32>,
    pub rotation: Point3<f32>,
    pub scale: Point3<f32>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityUniform {
    pub position: [f32; 3],
    pub _padding: u32,
    pub rotation: [f32; 3],
    pub _padding: u32,
    pub scale: [f32; 3],
    pub _padding: u32,
}

impl Entity {
    pub fn new<V: Into<Point3<f32>>, R: Into<Point3<f32>,>>(){

    }
}