use cgmath::*;
use winit::event::*;
use winit::dpi::PhysicalPosition;
use instant::Duration;
use std::f32::consts::FRAC_PI_2;


#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);


const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

#[derive(Debug)]
pub struct Light {
    pub position: Point3<f32>,
    yaw: Rad<f32>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _padding: u32,
    pub color: [f32; 3],
    pub _padding2: u32,
}

impl Light {
    pub fn new<V: Into<Point3<f32>>, Y: Into<Rad<f32>>,>(position: V, yaw: Y,) -> Self {
        Self {
            position: position.into(),
            yaw: yaw.into(),
        }
    }
}

pub struct MovableLightController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    speed: f32,
    sensitivity: f32,
}

impl MovableLightController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed { 1.0 } else { 0.0 };

        match key {
            VirtualKeyCode::I | VirtualKeyCode::Up => {
                self.amount_forward = amount;
                true
            }
            VirtualKeyCode::K | VirtualKeyCode::Down => {
                self.amount_backward = amount;
                true
            }
            VirtualKeyCode::J | VirtualKeyCode::Left => {
                self.amount_left = amount;
                true
            }
            VirtualKeyCode::L | VirtualKeyCode::Right => {
                self.amount_right = amount;
                true
            }
            VirtualKeyCode::U => {
                self.amount_up = amount;
                true
            }
            VirtualKeyCode::O => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn update_light ( &mut self, light: &mut Light, light_uniform: &mut LightUniform, dt: Duration) {
        let dt = dt.as_secs_f32();

        let (yaw_sin, yaw_cos) = light.yaw.0.sin_cos();
        let forward = Vector3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right = Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();
        light.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        light.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        // let (pitch_sin, pitch_cos) = light.pitch.0.sin_cos();
        // light.position += scrollward * self.scroll * self.speed * self.sensitivity * dt;


        light.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        light_uniform.position = light.position.into();
    }
}