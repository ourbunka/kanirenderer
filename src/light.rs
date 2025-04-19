use cgmath::*;
use cgmath::num_traits::ToPrimitive;
use wgpu::{util::{self, DeviceExt}, Buffer};
use winit::event::*;
use winit::dpi::PhysicalPosition;
use instant::Duration;
use std::{f32::consts::FRAC_PI_2, future::IntoFuture};

use crate::{load_model, model};


#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);


const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

//#[derive(Debug)]
pub struct Light {
    pub position: Point3<f32>,
    yaw: Rad<f32>,
    pub range: f32,
    pub color: [f32;3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _padding: u32,
    pub color: [f32; 3],
    pub range: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightData {
    pub position: [f32; 3],
    pub _padding: u32,
    pub color: [f32; 3],
    pub range: f32,
}

pub struct DirectionalLight {
    pub color: [f32;3],
    pub _padding: f32,
    pub light_direction: [f32;3],
    pub _padding2: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DirectionalLightUniformData {
    pub color: [f32;3],
    pub _padding: f32,
    pub light_direction: [f32;3],
    pub _padding2: f32,
}

impl DirectionalLight {
    pub fn new(direction: [f32;3],color: [f32;3]) -> Self {
        Self {
            color: color,
            _padding: 1.0,
            light_direction: direction,
            _padding2:1.0,
        }
    }

    pub fn generate_directional_light_data(&self) -> DirectionalLightUniformData {
        let direction = self.light_direction;
        let color = self.color;
        DirectionalLightUniformData {
            color: color,
            _padding: 1.0,
            light_direction: direction,
            _padding2: 1.0,
        }
    }
}

pub fn init_new_directional_lights_Uniform(directional_light_uniform : DirectionalLightUniformData, device: &wgpu::Device, ) -> wgpu::Buffer {
    let new_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
        label: Some("Directional Lights Uniform"),
        contents: bytemuck::cast_slice(&[directional_light_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }    
    );
    new_buffer
}


impl Light {
    pub fn new<V: Into<Point3<f32>>, Y: Into<Rad<f32>>,>(position: V, yaw: Y,color: [f32;3], range : f32) -> Self {
        
        Self {
            position: position.into(),
            yaw: yaw.into(),
            range: range,
            color: color,
        }
    }

    pub fn generate_point_light_data(&self) -> PointLightData {
        let position = Vector3 { x: self.position.x, y: self.position.y, z: self.position.z};
        //let color = [1.0 as f32,1.0,1.0];
        let color = self.color;
        let range = self.range;
        PointLightData {
            position: position.into(),
            _padding: 0,
            color: color,
            range: range
        }
    }
}

pub fn init_new_point_lights_buffer(point_light_data : Vec<PointLightData>, device: &wgpu::Device, ) -> Buffer {
    let new_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
        label: Some("Point Lights Buffer"),
        contents: bytemuck::cast_slice(&point_light_data),
        usage: wgpu::BufferUsages::STORAGE,
        }    
    );
    new_buffer
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
    range: f32,
    light_color: Vector3<f32>
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
            range: 1.0,
            light_color: Vector3::new(1.0, 1.0, 1.0)
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
            VirtualKeyCode::Equals => {
                if state == ElementState::Pressed  && self.range > 0.1 {
                    println!("Increasing Point Light Ranges");
                    self.range = self.range - 0.2;
                    //println!("{:?}", self.range);
                }
                true
            }
            VirtualKeyCode::Minus => {
                if state == ElementState::Pressed && self.range < 500.0 {
                    println!("Decreasing Point Light Ranges");
                    self.range = self.range + 0.2;
                    //println!("{:?}", self.range);
                }
                true
            }
            VirtualKeyCode::LBracket =>{
                if state == ElementState::Pressed && self.light_color.x >0.1 {
                    self.light_color -= [0.1,0.1,0.1].into();
                }
                true
            }
            VirtualKeyCode::RBracket => {
                if state == ElementState::Pressed && self.light_color.x <10.0 {
                    self.light_color += [0.1,0.1,0.1].into();
                }
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
        light.range = self.range.into();
        light_uniform.range = self.range.into();
        light_uniform.color = self.light_color.into();
    }
}