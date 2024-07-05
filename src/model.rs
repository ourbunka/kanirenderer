use std::{ops::Range, usize};

use cgmath::{num_traits::ToPrimitive, perspective, prelude::*, Point3, Vector3};
use instant::Duration;
use crate::texture::Texture;

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }

}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
}

impl Model {
    pub fn test_move_model(& mut self , i :usize) {
        if i % 2 == 0{
            self.instances[0].position.x += 1.0;  
        } else {
            self.instances[0].position.x -= 1.0;  
        }
        if self.instances[0].position.x < -100.0 {
            self.instances[0].position.x = 100.0
        }
        if self.instances[0].position.x > 100.0 {
            self.instances[0].position.x = -100.0
        }
        
        //let instance_data = self.instances.iter().map(Instance::to_raw).collect::<Vec<_>>();

    }
}

pub fn test_move_model(posx: f32 , i :usize, dt: Duration) ->f32 {
    let mut newpos = posx;
    if i % 2 == 0{
        newpos += 1.0;  
    } else {
        newpos -= 1.0;  
    }
    if posx < -100.0 {
        newpos = 100.0
    }
    if posx > 100.0 {
        newpos = -100.0
    }
    newpos
    //let instance_data = self.instances.iter().map(Instance::to_raw).collect::<Vec<_>>();

}


pub struct Material {
    pub name: String,
    pub diffuse_texture: Texture,
    pub normal_texture: Texture,
    pub bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn new( device: &wgpu::Device, 
        name: &str, 
        diffuse_texture: Texture, 
        normal_texture: Texture,
        layout: &wgpu::BindGroupLayout) -> Self {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                    },
                ],
                label: Some(name),
            });

            Self {
                name: String::from(name),
                diffuse_texture,
                normal_texture,
                bind_group,
            }
        }
}

pub struct Mesh  {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

pub trait  DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh, 
        material: &'a Material, 
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(&mut self, mesh: &'b Mesh, material: &'b Material, camera_bind_group: &'b wgpu::BindGroup, light_bind_group: &'b wgpu::BindGroup) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_mesh_instanced(
            &mut self,
            mesh: &'b Mesh,
            material: &'b Material,
            instances: Range<u32>,
            camera_bind_group: &'b wgpu::BindGroup,
            light_bind_group: &'b wgpu::BindGroup,
        ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_model(
            &mut self,
            model: &'b Model,
            camera_bind_group: &'b wgpu::BindGroup,
            light_bind_group: &'b wgpu::BindGroup,
        ) {
        self.draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_model_instanced(
            &mut self,
            model: &'b Model,
            instances: Range<u32>,
            camera_bind_group: &'b wgpu::BindGroup,
            light_bind_group: &'b wgpu::BindGroup,
        ) {
        for mesh in &model.meshes {
            if !&model.materials.is_empty() {
                let material = &model.materials[mesh.material];
                self.draw_mesh_instanced(mesh, material, instances.clone(), camera_bind_group, light_bind_group);
            }
            
        }
    }
}


pub struct Instance {
    pub position: cgmath::Vector3<f32>,
    pub rotation: cgmath::Quaternion<f32>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[allow(dead_code)]
pub struct InstanceRaw {
    pub model: [[f32;4]; 4],
    pub normal: [[f32;3]; 3],
    pub _padding: u32,
}

pub fn update_instance(num_instances:i32, current_position: Vector3<f32>, ) -> Vec<Instance>{
    const SPACE_BETWEEN: f32 = 3.0;
    let NUM_INSTANCES_PER_ROW = num_instances;
    let instance = NUM_INSTANCES_PER_ROW*NUM_INSTANCES_PER_ROW;
    //let spawn_position = current_position;
    use rayon::prelude::*;
    let instances = (0..NUM_INSTANCES_PER_ROW)
            .into_par_iter()
            .flat_map_iter(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = (SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0))+current_position.x;
                    let z = (SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0));
                    let y = (SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0))+(instance.to_f32().unwrap()*2.0 as f32);

                    let mut position = cgmath::Vector3 { x, y, z };
                    if instance == 0 {
                        position = current_position
                    }
                    let rotation = if position.is_zero() {
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.0),
                        )
                    }else if instance == 1 {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
                    };
                    
                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();
    instances
}



impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        let model = cgmath::Matrix4::from_translation(self.position) * cgmath::Matrix4::from(self.rotation);
        InstanceRaw { 
            model: model.into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
            _padding: 0,
        }
    }
    
}


impl Vertex for InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We don't have to do this in code though.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // NEW!
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

