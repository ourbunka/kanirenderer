#![allow(warnings)] 

use std::{clone, default, env, fs::{read, File}, io::BufWriter, ops::Deref, sync::{mpsc::{self, Receiver, Sender, SyncSender}, Arc, Mutex}, thread, vec};
mod texture;
mod model;
mod resources;
mod camera;
mod light;
mod deferredRenderPipeline;
mod frametime;

use bytemuck::{cast_slice, Contiguous};
use frametime::FrameTimeGraphRaw;
use image::{buffer, ImageBuffer, Rgba};
use instant::now;
use light::{init_new_directional_lights_Uniform, init_new_point_lights_buffer, DirectionalLight, DirectionalLightUniformData, PointLightData};
use model::{update_instance_position_rotation, DrawModel, Instance, Model, Vertex};
use cgmath::{num_traits::ToPrimitive, perspective, prelude::*, vec4, Vector3};
use pollster::block_on;
use rand::Rng;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use texture::Texture;
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout, Buffer, BufferAddress, BufferSize, BufferUsages, DepthBiasState, Extent3d, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout, Origin3d, PipelineLayout, RenderPipeline, Sampler, ShaderModule, TextureView};
use winit::{
    dpi::PhysicalSize, event::*, event_loop::{ControlFlow, EventLoop}, platform::windows::{WindowBuilderExtWindows, WindowExtWindows}, window::{self, Fullscreen, WindowBuilder}
};
use winit::window::Window;
use wgpu::TextureFormat;
use crate::resources::*;


#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DebugVert {
    position: [f32;2],
}

use winit::platform::windows::IconExtWindows;
use winit::window::Icon;

fn load_icon(window: &Window){
    let icon_name = "icon.png";
    let img_rgba = image::open(icon_name).unwrap().to_rgba8().into_vec();
    let icon = Icon::from_rgba(img_rgba, 256,256).unwrap();
    window.set_window_icon(Some(icon.clone()));
    window.set_taskbar_icon(Some(icon));
}

fn get_icon() -> Icon{
    let icon_name = "icon.png";
    let img_rgba = image::open(icon_name).unwrap().to_rgba8().into_vec();
    let icon = Icon::from_rgba(img_rgba, 256,256).unwrap();
    icon
}


enum RenderOutputMode {
    Unlit,
    Lit,
    LitWithShadow,
    Wireframe,
    DebugLitWithShadow,
}

enum DebugTexture {
    DepthTexture,
    ShadowTexture,
}

enum WindowMode{
    Fullscreen,
    Windowed,
}

struct State {
    free_cam : bool,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    window_mode: WindowMode,
    render_pipeline: wgpu::RenderPipeline,
    wireframe_pipeline: wgpu::RenderPipeline,
    camera: camera::Camera,
    projection: camera::Projection,
    camera_controller: camera::CameraController,
    camera_uniform: camera::CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // instances: Vec<Instance>,
    // instance_buffer: wgpu::Buffer,
    depth_texture: wgpu::Texture,
    depth_view: TextureView,
    depth_sampler: wgpu::Sampler,
    models : Vec<model::Model>,
    light_uniform: light::LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    point_light: Vec<light::Light>,
    point_light_buffer: wgpu::Buffer,
    directional_light: light::DirectionalLight,
    directional_light_uniform_data: light::DirectionalLightUniformData,
    directional_light_uniform: wgpu::Buffer,
    shadow_texture_size: u32,
    shadow_texture: wgpu::Texture,
    shadow_texture_view: TextureView,
    shadow_sampler: Sampler,
    shadow_shader: ShaderModule,
    shadow_bind_group: BindGroup,
    shadow_pass_light_bind_group: BindGroup,
    shadow_pipeline: RenderPipeline,
    unlit_render_pipeline: RenderPipeline,
    lit_render_pipeline: RenderPipeline,
    debug_pass_pipeline: RenderPipeline,
    debug_pass_bind_group: BindGroup,
    debug_depth_bind_group_layout: BindGroupLayout,
    debug_vertex_buffer: wgpu::Buffer,
    debug_index_buffer: wgpu::Buffer,
    debug_indices: Vec<i32>,
    debug_vert: Vec<[f32;2]>,
    movable_light: light::Light,
    movable_light_controller: light::MovableLightController,
    mouse_pressed: bool,
    left_mouse_pressed: bool,
    mouse_x: u32,
    mouse_y: u32,
    render_output_mode: RenderOutputMode,
    debug_mode_texture: DebugTexture,
    frame_time_graph: frametime::FrameTimeGraph,
    frametime_vertex_buffer: wgpu::Buffer,
    frame_time_render_pipeline: RenderPipeline,
}

fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                // blend: Some(wgpu::BlendState {
                //     alpha: wgpu::BlendComponent::REPLACE,
                //     color: wgpu::BlendComponent::REPLACE,
                // }),
                blend:  Some(wgpu::BlendState{
                    color: {
                        wgpu::BlendComponent{
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            operation: wgpu::BlendOperation::Add,
                        }
                    },
                    alpha:  {
                        wgpu::BlendComponent{
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            operation: wgpu::BlendOperation::Add,
                        }
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}

fn create_wireframe_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Wireframe Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        operation: wgpu::BlendOperation::Add,
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    },
                    alpha: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Line,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}
fn optional_features() -> wgpu::Features {
        let mut f = wgpu::Features::POLYGON_MODE_LINE;
        //f.insert(wgpu::Features::VERTEX_WRITABLE_STORAGE);
        //wgpu::Features::VERTEX_WRITABLE_STORAGE
        f
    }

impl State {
    async fn new(window: Window, file_path: String, file_type:String, use_hdr: bool, window_mode: WindowMode) -> Self {
        let free_cam = true;
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter= instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: optional_features(),
                limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
            },
            None,
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        
        let mut surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| *f == TextureFormat::Rgba8UnormSrgb)
            .unwrap_or(surface_caps.formats[0]);
        if use_hdr{
            surface_format = TextureFormat::Rgba16Float;
        } else {
            surface_format = TextureFormat::Rgba8UnormSrgb;
        }
        println!("{:?}", surface_format);
        
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,//surface_caps.present_modes[2],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![surface_format],
        };
        surface.configure(&device, &config);

        let texture_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_binding_group_layout"),
            });

        let camera = camera::Camera::new((0.0, 5.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));

        let projection = camera::Projection::new(config.width, config.height, cgmath::Deg(45.0), 0.1, 10000.0);
        
        let camera_controller = camera::CameraController::new(300.0, 0.4);

        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);

        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });


        // let smp_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { 
        //     label: None, 
        //     entries: &[
        //         wgpu::nomd
        //     ],
        // });
        let m_pos = [0.0,100.0,0.0];
        let m_color = [20.0,20.0,20.0];
        let m_range = 256.0;

        let light_uniform = light::LightUniform {
            position: m_pos,
            _padding: 0,
            color: m_color,
            range: m_range,
        };

        let movable_light = light::Light::new(m_pos, cgmath::Deg(-90.0),m_color, m_range);

        let movable_light_controller = light::MovableLightController::new(300.0, 1.0, m_range, m_color.into());

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let mut point_light: Vec<light::Light> = Vec::new();
        let mut point_light_data: Vec<light::PointLightData> = Vec::new();

        //for spawning multiple point light
        let light_num = 1;
        for i in 0..light_num {
            if i == 0 {
                let mut new_light = light::Light::new([99999.0,999999.0,99999.0], cgmath::Deg(-90.0), [0.0, 0.0, 0.0], 0.0);
                point_light.push(new_light);
                let new_light_data = point_light[i].generate_point_light_data();
                point_light_data.push(new_light_data);
            } else {
                let color_r = 0.1;
                let mut rng = rand::rng();
                let random_pos = [
                    rng.random_range(-1000.0..1000.0).to_f32().unwrap(), 
                    rng.random_range(10.0..15.0).to_f32().unwrap(), 
                    rng.random_range(-1000.0..1000.0).to_f32().unwrap()
                    ];
                let mut new_light = light::Light::new(random_pos, cgmath::Deg(-90.0), [10.0, 0.0, 0.0], 256.0);
                
                point_light.push(new_light);
                let new_light_data = point_light[i].generate_point_light_data();
                point_light_data.push(new_light_data);
            }
            
        }
        if light_num >= 50 {
            for i in 0..light_num {
            let color_r = 0.1;
            let mut rng = rand::rng();
            let random_pos = [
                rng.random_range(-1000.0..1000.0).to_f32().unwrap(), 
                rng.random_range(10.0..15.0).to_f32().unwrap(), 
                rng.random_range(-1000.0..1000.0).to_f32().unwrap()
                ];
            let mut new_light = light::Light::new(random_pos, cgmath::Deg(-90.0), [0.0, 10.0, 0.0], 256.0);
            
            point_light.push(new_light);
            let new_light_data = point_light[i+50].generate_point_light_data();
            point_light_data.push(new_light_data);
            }
            for i in 0..light_num {
            let color_r = 0.1;
            let mut rng = rand::rng();
            let random_pos = [
                rng.random_range(-1000.0..1000.0).to_f32().unwrap(), 
                rng.random_range(10.0..15.0).to_f32().unwrap(), 
                rng.random_range(-1000.0..1000.0).to_f32().unwrap()
                ];
            let mut new_light = light::Light::new(random_pos, cgmath::Deg(-90.0), [0.0, 0.0, 10.0], 256.0);
            
            point_light.push(new_light);
            let new_light_data = point_light[i+100].generate_point_light_data();
            point_light_data.push(new_light_data);
            }
        }
        
        
        let point_light_buffer = init_new_point_lights_buffer(point_light_data, &device);

        let directional_light = light::DirectionalLight::new([0.0, -0.9902682, -0.1391731], [1.0,1.0,1.0]);

        
        let directional_light_uniform = directional_light.generate_directional_light_data();
        
        let directional_light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Directional Light Buffer"),
            contents: bytemuck::cast_slice(&[directional_light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: (true) },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
            ],
            label: None,
        });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            },
                wgpu::BindGroupEntry{
                binding:1,
                resource: point_light_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry{
                binding:2,
                resource: directional_light_buffer.as_entire_binding(),
            }
            ],
            label: None,
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor{
            label: Some("Scene Depth Texture"),
            size: wgpu::Extent3d{
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC ,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor{
            label: Some("depth view"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            format: Some(wgpu::TextureFormat::Depth32Float),
            ..Default::default()
        });

        let depth_sampler =  device.create_sampler(&wgpu::SamplerDescriptor{
            label: Some("Depth Sampler"),
            compare: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let debug_vert = vec![
            [-1.0 as f32, -1.0], // Bottom-left
            [1.0, -1.0],  // Bottom-right
            [1.0, 1.0],   // Top-right
            [-1.0, 1.0],  // Top-left
        ];

        let debug_indices = vec![0, 1, 2, 0, 2, 3]; //two triangles

        let debug_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:Some("Debug Quad Vert Buffer"),
            contents: bytemuck::cast_slice(&debug_vert),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let debug_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Debug Indices Buffer"),
            contents: bytemuck::cast_slice(&debug_indices),
            usage: wgpu::BufferUsages::INDEX,
        });


        let debug_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DebugVert>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        };

        let debug_depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor{
            label: Some("Debug Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("debug_depth.wgsl").into()),
        });

        let debug_depth_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            label: Some("Debug Depth Bind Group Layout"),
            entries: &[
                //depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { 
                        sample_type: wgpu::TextureSampleType::Depth, 
                        view_dimension: wgpu::TextureViewDimension::D2, 
                        multisampled: false, 
                    },
                    count: None,
                },
                //debug depth sampler
                wgpu::BindGroupLayoutEntry{
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let debug_pass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor{
            label:Some("Debug Pass Bind Group"),
            layout: &debug_depth_bind_group_layout,
            entries: &[
                //depth texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth_view)
                },
                //depth samplers
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&depth_sampler),
                }
            ]
        });

        let debug_pass_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label: Some("Debug Depth Pass Pipeline Layout"),
            bind_group_layouts:&[
                &debug_depth_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let debug_pass_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor { 
            label: Some("Debug Pass Render Pipeline"), 
            layout: Some(&debug_pass_pipeline_layout), 
            vertex: wgpu::VertexState{
                module: &debug_depth_shader,
                entry_point: "vs_main",
                buffers: &[debug_vertex_buffer_layout],
            }, 
            fragment: Some(wgpu::FragmentState {
                module: &debug_depth_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState{
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }), 
            primitive: wgpu::PrimitiveState{
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            }, 
            depth_stencil: None, 
            multisample: wgpu::MultisampleState::default(),             
            multiview: None,
        });

        //to do
        let useDeferredRenderer = false;

        if useDeferredRenderer{
            todo!();
        } else {
            
        }
        
        let shadow_texture_size = 2048;
        let shadow_texture_size_extent3d = wgpu::Extent3d{
            width: shadow_texture_size,
            height: shadow_texture_size,
            depth_or_array_layers:1,
        };
        let shadow_texture =  device.create_texture(
            &wgpu::TextureDescriptor{
                label: Some("shadow map texture"),
                size: shadow_texture_size_extent3d,
                mip_level_count:1,
                sample_count:1,
                dimension:wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }
        );
        let shadow_texture_view = shadow_texture.create_view(
            &wgpu::TextureViewDescriptor::default()
        );

        //pcf filtering
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor{
            label: Some("Shadow Sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor{
            label:Some("Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()),
        });

        let shadow_pass_light_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX ,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: (true) },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX ,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
            ],
            label: None,
        });

        let shadow_pass_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &shadow_pass_light_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            },
                wgpu::BindGroupEntry{
                binding:1,
                resource: point_light_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry{
                binding:2,
                resource: directional_light_buffer.as_entire_binding(),
            }
            ],
            label: None,
        });

        let shadow_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            label: Some("shadow bind group layout"),
            entries: &[
                //shadow map
                wgpu::BindGroupLayoutEntry{
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { 
                        sample_type: wgpu::TextureSampleType::Depth, 
                        view_dimension: wgpu::TextureViewDimension::D2, 
                        multisampled: false,
                    },
                    count:None,
                },
                //shadow sampler
                wgpu::BindGroupLayoutEntry{
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count:None,
                },
            ],
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor{
            label: Some("Shadow Bind Goup"),
            layout: &shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry{
                    binding:0,
                    resource: wgpu::BindingResource::TextureView(&shadow_texture_view),
                },
                wgpu::BindGroupEntry{
                    binding:1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        let shadow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[
                &shadow_pass_light_bind_group_layout,
                &camera_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label: Some("Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState{
                module: &shadow_shader,
                entry_point: "vs_main",
                buffers: &[model::ModelVertex::desc(), 
                model::InstanceRaw::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shadow_shader,
                entry_point: "fs_main",
                targets: &[], // No color targets
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState{
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState{
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let unlit_render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Unlit Render Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let unlit_shader = include_str!("unlit_shader.wgsl").into();

        let unlit_render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Unlit Shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(unlit_shader)),
            };

            create_render_pipeline(&device, 
                &unlit_render_pipeline_layout, 
                config.format, 
                Some(wgpu::TextureFormat::Depth32Float), 
                &[model::ModelVertex::desc(), 
                model::InstanceRaw::desc()], 
                shader)
        };

        let lit_shader = include_str!("lit_shader.wgsl").into();

        let lit_render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lit Without Shadow Map Render Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &light_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let lit_render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Lit Without Shadow Map Shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(lit_shader)),
            };

            create_render_pipeline(&device, 
                &lit_render_pipeline_layout, 
                config.format, 
                Some(wgpu::TextureFormat::Depth32Float), 
                &[model::ModelVertex::desc(), 
                model::InstanceRaw::desc()], 
                shader)
        };

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lit With Shadow Map Render Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &light_bind_group_layout,
                &shadow_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let mut shaders: &str;
        match config.format{
            TextureFormat::Rgba16Float => {shaders = include_str!("shader_hdr.wgsl").into()}
            TextureFormat::Rgba8UnormSrgb => {shaders = include_str!("shader.wgsl").into()}
            _ => {shaders = include_str!("shader.wgsl").into()}
        }
        let render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shaders)),
            };

            create_render_pipeline(&device, 
                &render_pipeline_layout, 
                config.format, 
                Some(wgpu::TextureFormat::Depth32Float), 
                &[model::ModelVertex::desc(), 
                model::InstanceRaw::desc()], 
                shader)
        };

        let wireframe_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor{
                label: Some("Wireframe Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader_wireframe.wgsl").into()),
            };
            create_wireframe_pipeline(&device, 
                &render_pipeline_layout, 
                config.format, 
                Some(wgpu::TextureFormat::Depth32Float), 
                &[model::ModelVertex::desc(), 
                model::InstanceRaw::desc()], 
                shader)
        };

        use std::time::{Duration,Instant};
        let start_loading_time = Instant::now();
        let obj_model = match file_type.clone().as_str() {
            "opengl" => resources::load_model(&file_path, file_type.clone(), &device, &queue, &texture_bind_group_layout,1, cgmath::Vector3 { x: 0.0, y: 0.0, z: 0.0 }).await.unwrap(),
            "default" => resources::load_model(&file_path, file_type.clone(), &device, &queue, &texture_bind_group_layout,1,cgmath::Vector3 { x: 0.0, y: 0.0, z: 0.0 }).await.unwrap(),
            _ => panic!("no file type given"),
        };
        let loading_duration = start_loading_time.elapsed();
        println!("total loading time {:?}" , loading_duration);
        let mut movable_model_counts = 0;

        let mut models = Vec::new();
        models.push(obj_model);
        //movable_model_counts +=1;
        let mut rng = rand::rng();
        let instances_num = 1000;
        for i in 1..=0 {
        let test_mesh = resources::load_model("default_cube.obj",
                "opengl".to_string(), 
                &device, &queue, &texture_bind_group_layout,
                instances_num,cgmath::Vector3 { x: (rng.random_range(-1500.0..1500.0) as f32),
                y: (rng.random_range(30.0..100.0)  as f32), z: (rng.random_range(-1500.0..1500.0)  as f32) }).await.unwrap();

        models.push(test_mesh); 
        println!("pushed : {i}");
        movable_model_counts +=1;
        } 
        println!("total movable model/object : {:?}",movable_model_counts*instances_num);
        
        let mut render_output_mode = RenderOutputMode::LitWithShadow;
        let mut left_mouse_pressed = false;
        let debug_mode_texture = DebugTexture::DepthTexture;

        let max_points = std::mem::size_of::<FrameTimeGraphRaw>() as u64;
        let frametime_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            size: max_points,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST ,
            label: Some("Frame Graph Buffer"),
            mapped_at_creation: false,
        });
        let mut frame_time_graph = frametime::FrameTimeGraph::new();

        let frame_time_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { 
            label: Some("Frame Time Shader") , 
            source: wgpu::ShaderSource::Wgsl(include_str!("frametime.wgsl").into()), 
        });

        let frame_time_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as u64, // 8 bytes per vertex
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2, // Each vertex is a vec2<f32>
                    offset: 0,
                    shader_location: 0,
                },
            ],
        };

        let frame_time_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label: Some("Frame Graph Overlay Pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &frame_time_shader,
                    entry_point: "vs_main",
                    buffers: &[frame_time_vertex_buffer_layout],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &frame_time_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineStrip,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            multiview: None,
        });

        Self {
            free_cam,
            window,
            window_mode,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            wireframe_pipeline,
            camera,
            projection,
            camera_controller,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            depth_view,
            depth_sampler,
            models,
            point_light,
            point_light_buffer,
            directional_light,
            directional_light_uniform_data: directional_light_uniform,
            directional_light_uniform: directional_light_buffer,
            light_uniform,
            light_buffer,
            light_bind_group,
            shadow_texture_size,
            shadow_texture,
            shadow_texture_view,
            shadow_sampler,
            shadow_shader,
            shadow_bind_group,
            shadow_pass_light_bind_group,
            shadow_pipeline,
            unlit_render_pipeline,
            lit_render_pipeline,
            debug_pass_pipeline,
            debug_pass_bind_group,
            debug_depth_bind_group_layout,
            debug_vertex_buffer,
            debug_index_buffer,
            debug_indices,
            debug_vert,
            mouse_pressed: false,
            mouse_x: 0,
            mouse_y: 0,
            left_mouse_pressed,
            //single point light to be removed after implementing movable light controller with ability to control each light in Vec<Light>
            movable_light,
            movable_light_controller,
            render_output_mode,
            debug_mode_texture,
            frame_time_graph,
            frametime_vertex_buffer,
            frame_time_render_pipeline
        }
    }

    
    

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.projection.resize(new_size.width, new_size.height);
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Scene Depth Texture"),
                size: wgpu::Extent3d {
                    width: self.size.width, 
                    height: self.size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.depth_view = self.depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.debug_pass_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor{
                label:Some("Debug Pass Bind Group"),
                layout: &self.debug_depth_bind_group_layout,
                entries: &[
                    //depth texture
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.depth_view)
                    },
                    //depth samplers
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.depth_sampler),
                    }
                ]
            });
        }
    }

    fn input(&mut self, event: &WindowEvent,) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    virtual_keycode: Some(key),
                    state,
                    ..
                },
                ..
            } => {  
                    self.movable_light_controller.process_keyboard(*key, *state);
                    self.camera_controller.process_keyboard(*key, *state);
                    match *key {
                        VirtualKeyCode::Tab if *state == ElementState::Released => {
                                match self.render_output_mode {
                                    RenderOutputMode::Unlit => {self.render_output_mode = RenderOutputMode::Lit; true}
                                    RenderOutputMode::Lit => {self.render_output_mode = RenderOutputMode::LitWithShadow; true}
                                    RenderOutputMode::LitWithShadow => {self.render_output_mode = RenderOutputMode::Wireframe; true}
                                    RenderOutputMode::Wireframe => {self.render_output_mode = RenderOutputMode::DebugLitWithShadow; true}
                                    RenderOutputMode::DebugLitWithShadow => {self.render_output_mode = RenderOutputMode::Unlit; true}
                                } 
                            }

                        VirtualKeyCode::F11 if *state == ElementState::Released => {
                                println!("updating window mode");
                                match self.window_mode {
                                    WindowMode::Fullscreen => {
                                        self.window.set_fullscreen(None);
                                        self.window_mode = WindowMode::Windowed;
                                        self.window.set_cursor_grab(window::CursorGrabMode::Confined);
                                        true
                                    }
                                    WindowMode::Windowed => {
                                        self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                                        self.window_mode = WindowMode::Fullscreen;
                                        self.window.set_cursor_grab(window::CursorGrabMode::Locked);
                                        true
                                    }
                                }
                            }
                        VirtualKeyCode::F1 if *state == ElementState::Released => {
                            match self.config.present_mode {
                                wgpu::PresentMode::AutoVsync => {
                                    self.config.present_mode = wgpu::PresentMode::AutoNoVsync; 
                                    self.surface.configure(&self.device, &self.config);
                                    true
                                },
                                wgpu::PresentMode::AutoNoVsync => {
                                    self.config.present_mode = wgpu::PresentMode::Fifo;
                                    self.surface.configure(&self.device, &self.config);
                                    true
                                },
                                wgpu::PresentMode::Fifo => {
                                    self.config.present_mode = wgpu::PresentMode::Immediate;
                                    self.surface.configure(&self.device, &self.config);
                                    true
                                },
                                wgpu::PresentMode::FifoRelaxed => {
                                    self.config.present_mode = wgpu::PresentMode::Immediate;
                                    self.surface.configure(&self.device, &self.config);
                                    true},
                                wgpu::PresentMode::Immediate => {
                                    self.config.present_mode = wgpu::PresentMode::AutoVsync;
                                    self.surface.configure(&self.device, &self.config);
                                    true
                                },
                                wgpu::PresentMode::Mailbox => {
                                    self.config.present_mode = wgpu::PresentMode::AutoVsync;
                                    self.surface.configure(&self.device, &self.config);
                                    true
                                },
                            }
                        }
                        
                        VirtualKeyCode::Key1 if *state == ElementState::Released => {
                                match self.debug_mode_texture {
                                    DebugTexture::DepthTexture => {
                                            self.debug_mode_texture = DebugTexture::ShadowTexture;

                                            self.debug_pass_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor{
                                                label:Some("Debug Pass Bind Group"),
                                                layout: &self.debug_depth_bind_group_layout,
                                                entries: &[
                                                    //depth texture
                                                    wgpu::BindGroupEntry {
                                                        binding: 0,
                                                        resource: wgpu::BindingResource::TextureView(&self.shadow_texture_view)
                                                    },
                                                    //depth samplers
                                                    wgpu::BindGroupEntry {
                                                        binding: 1,
                                                        resource: wgpu::BindingResource::Sampler(&self.depth_sampler),
                                                    }
                                                ]
                                            });
                                            true
                                        },

                                    DebugTexture::ShadowTexture => {
                                            self.debug_mode_texture = DebugTexture::DepthTexture;
                                            self.debug_pass_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor{
                                                label:Some("Debug Pass Bind Group"),
                                                layout: &self.debug_depth_bind_group_layout,
                                                entries: &[
                                                    //depth texture
                                                    wgpu::BindGroupEntry {
                                                        binding: 0,
                                                        resource: wgpu::BindingResource::TextureView(&self.depth_view)
                                                    },
                                                    //depth samplers
                                                    wgpu::BindGroupEntry {
                                                        binding: 1,
                                                        resource: wgpu::BindingResource::Sampler(&self.depth_sampler),
                                                    }
                                                ]
                                            });
                                            true
                                        },
                                }
                            }
                        
                        VirtualKeyCode::Key2 if *state == ElementState::Pressed && self.directional_light.distance > -3000.0 => {
                            self.directional_light.distance -= 10.0;
                            self.directional_light.shadow_scene_size = self.directional_light.distance.abs() * 1.5;
                            println!("{:?}, {:?}",self.directional_light.distance, self.directional_light.shadow_scene_size);
                            true
                        }
                        VirtualKeyCode::Key3 if *state == ElementState::Pressed && self.directional_light.distance < -100.0 => {
                            self.directional_light.distance += 10.0;
                            self.directional_light.shadow_scene_size = self.directional_light.distance.abs() * 1.5;
                            println!("{:?}, {:?}",self.directional_light.distance, self.directional_light.shadow_scene_size);
                            true
                        }
                        VirtualKeyCode::R if *state == ElementState::Pressed => {
                            self.directional_light.rotate_light(4.0, 0.0, 0.0);
                            println!("rotating directional light.x, {:?}", self.directional_light.light_direction);
                            true
                        }
                        VirtualKeyCode::T if *state == ElementState::Pressed => {
                            self.directional_light.rotate_light(0.0, 4.0, 0.0);
                            println!("rotation directional light.y, {:?}", self.directional_light.light_direction);
                            true
                        }
                        VirtualKeyCode::Y if *state == ElementState::Pressed => {
                            self.directional_light.rotate_light(0.0, 0.0, 4.0);
                            println!("rotation directional light.z, {:?}", self.directional_light.light_direction);
                            true
                        }

                        _ => {false}
                    }
                    
                }
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput { 
                button: MouseButton::Right, state, .. } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            WindowEvent::MouseInput { 
                button: MouseButton::Left, state, .. } => {
                if(*state == ElementState::Pressed){
                self.left_mouse_pressed = true;
                }
                true
            }
            _ => false,
        }
    }


    fn update(&mut self, dt: instant::Duration) {
        self.frame_time_graph.update(dt);
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.movable_light_controller.update_light(&mut self.movable_light, &mut self.light_uniform, dt);
        self.camera_uniform.update_view_proj(&self.camera, &self.projection);
        //self.directional_light.rotate_light((4.0 * dt.as_secs_f32()), (10.0 * dt.as_secs_f32()), 0.0);
        self.directional_light_uniform_data = self.directional_light.generate_directional_light_data();
        
        
        
        use rayon::prelude::*;

        if self.models.len() > 1{
            ///// test moving models, will be ignored when model len() !>1 ///
            println!("chunking model update()");
            let chunk_size_f = self.models.len().to_f32().unwrap()/8.0;
            let chunk_size = chunk_size_f.ceil().to_usize().unwrap();
            let model_chunks = self.models
                .par_chunks(chunk_size)
                .collect::<Vec<_>>();
            let model_chunk_0: &[Model] = model_chunks.get(0).unwrap();
            let model_chunk_1: Option<&[Model]>  = model_chunks.get(1).map(|v| &**v);
            let model_chunk_2: Option<&[Model]>  = model_chunks.get(2).map(|v| &**v);
            let model_chunk_3: Option<&[Model]>  = model_chunks.get(3).map(|v| &**v);
            let model_chunk_4: Option<&[Model]>  = model_chunks.get(4).map(|v| &**v);
            let model_chunk_5: Option<&[Model]>  = model_chunks.get(5).map(|v| &**v);
            let model_chunk_6: Option<&[Model]>  = model_chunks.get(6).map(|v| &**v);
            let model_chunk_7: Option<&[Model]>  = model_chunks.get(7).map(|v| &**v);
            let mut modelpos_chunk_0: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_1: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_2: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_3: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_4: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_5: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_6: Vec<Vector3<f32>> = vec![];
            let mut modelpos_chunk_7: Vec<Vector3<f32>> = vec![];
            println!("chunking position of models");
            if !model_chunk_0.is_empty(){
                
                for model in model_chunk_0{
                    for k in &model.instances{
                        modelpos_chunk_0.push(k.position);
                    }
                
                }
            }
            match model_chunk_1{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_1.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            match model_chunk_2{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_2.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            match model_chunk_3{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_3.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            match model_chunk_4{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_4.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            match model_chunk_5{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_5.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            match model_chunk_6{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_6.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            match model_chunk_7{
                Some(models) => {
                    for model in models{
                        for k in &model.instances{
                            modelpos_chunk_7.push(k.position);
                        }
                    }
                }
                None => {} 
            }
            
            println!("generating channels");
            let mut pos: Arc<Mutex<Vec<Vector3<f32>>>>= Arc::new(Mutex::new(vec![]));
            let (tx_0 ,rx_0): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_1 ,rx_1): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_2 ,rx_2): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_3 ,rx_3): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_4 ,rx_4): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_5 ,rx_5): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_6 ,rx_6): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            let (tx_7 ,rx_7): (SyncSender<Vec<Vector3<f32>>>, Receiver<Vec<Vector3<f32>>> ) = mpsc::sync_channel(1);
            println!("spawning t_0");
            let t_0 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_0.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_0[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res =tx_0.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_1");
            let t_1 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_1.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_1[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_1.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_2");
            let t_2 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_2.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_2[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_2.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_3");
            let t_3 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_3.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_3[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_3.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_6");
            let t_4 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_4.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_4[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_4.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_6");
            let t_5 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_5.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_5[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_5.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_6");
            let t_6 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_6.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_6[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_6.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            println!("spawning t_7");
            let t_7 = std::thread::spawn(move || {
                let mut l_pos: Vec<Vector3<f32>> = vec![];
                for i in 0..modelpos_chunk_7.len(){
                    l_pos.push(model::test_move_model_vec3(modelpos_chunk_7[i], dt));
                }
                let mut sent = false;
                while !sent {
                    println!("sending new pos : pos len() = {:?}", l_pos.len());
                    let res = tx_7.send(l_pos.clone());
                    match res {
                        Ok(_) => {sent = true}
                        Err(err) => {println!("{:?}",err); sent = false}
                    }
                }
            });
            
            match &mut pos.lock(){
                Ok(g) => {
                    g.append(&mut rx_0.recv().unwrap());
                    g.append(&mut rx_1.recv().unwrap());
                    g.append(&mut rx_2.recv().unwrap());
                    g.append(&mut rx_3.recv().unwrap());
                    g.append(&mut rx_4.recv().unwrap());
                    g.append(&mut rx_5.recv().unwrap());
                    g.append(&mut rx_6.recv().unwrap());
                    g.append(&mut rx_7.recv().unwrap());
                }
                Err(err) => {println!("{err}")}
            }
            
            // model_chunks.into_par_iter().for_each(|models|{
            //     let mut chunk_pos = (0..models.len())
            //         .into_iter()
            //         .map(|i| model::test_move_model_vec3(models[i].instances[0].position , dt)).collect::<Vec<_>>();
            //     pos.lock().unwrap().append(&mut chunk_pos);
            // });


            // for models in model_chunks{
            //     let mut chunk_pos = (0..models.len())
            //         .into_iter()
            //         .map(|i| model::test_move_model_vec3(models[i].instances[0].position , dt)).collect::<Vec<_>>();
            //     pos.append(&mut chunk_pos);
            // }
            //let newpos: Vec<Vector3<f32>> = (1..self.models.len()).into_par_iter().map(|i| model::test_move_model_vec3(self.models[i].instances[0].position , dt)).collect::<Vec<_>>();
            let mut newpos: Vec<Vector3<f32>> = vec![];
            newpos = pos.as_ref().lock().unwrap().clone();
            println!("self.models.len() : {:?}", self.models.len());
            println!("newpos.len() : {:?}", newpos.len());
            println!("pos.len() : {:?}", pos.lock().unwrap().len());
            // for i in 0..self.models.len(){
            //     self.models[i].instances[0].position = newpos[i]
            // }
            let mut count = 0;
            for i in 0..self.models.len(){
                    for j in 0..self.models[i].instances.len(){
                        self.models[i].instances[j].position = newpos[count];
                        count += 1;
                    }
                }

            for i in 0..self.models.len(){
                if i != 0{
                    let instance_data = self.models[i].instances.iter().map(model::Instance::to_raw).collect::<Vec<_>>();
                    self.queue.write_buffer(&self.models[i].instance_buffer, 0, bytemuck::cast_slice(&instance_data)); 
                }
            
            }
        }

        // Generate vertices
        let frametime_vertices = self.frame_time_graph.get_vertices(self.window.inner_size().width as f32, self.window.inner_size().height as f32);

        // Write to vertex buffer
        
        /////////
        self.queue.write_buffer(&self.frametime_vertex_buffer, 0, bytemuck::cast_slice(&[frametime_vertices]));
        
        self.queue.write_buffer(&self.camera_buffer, 0,bytemuck::cast_slice(&[self.camera_uniform]));
        
        self.queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[self.light_uniform]));
        self.queue.write_buffer(&self.directional_light_uniform, 0, bytemuck::cast_slice(&[self.directional_light_uniform_data]));

    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture.create_view(&wgpu:: TextureViewDescriptor{
                format: Some(self.config.format), 
                ..Default::default()
            }
        );

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        //shadow pass
        match self.render_output_mode{
            RenderOutputMode::LitWithShadow | RenderOutputMode::DebugLitWithShadow => {
                let mut shadow_pass = Arc::new(Mutex::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Pass"),
                    color_attachments: &[], // No color output
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_texture_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0), // Clear to max depth
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                })));
                shadow_pass.lock().unwrap().set_pipeline(&self.shadow_pipeline);
                shadow_pass.lock().unwrap().set_bind_group(0, &self.shadow_pass_light_bind_group, &[]);
                &self.models.par_iter().for_each(|model|{
                    let vb =model.instance_buffer.slice(..);
                    let mut locked_sp = shadow_pass.lock().unwrap();
                    locked_sp.set_vertex_buffer(1, vb );
                    for mesh in &model.meshes{
                        locked_sp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        locked_sp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        locked_sp.set_bind_group(0, &self.shadow_pass_light_bind_group, &[]);
                        locked_sp.set_bind_group(1, &self.camera_bind_group, &[]);
                        locked_sp.draw_indexed(0..mesh.num_elements, 0, (0..model.instances.len() as u32).clone());
                    }
                });
            }
            _ => {}
        }
        

        {
            let mut render_pass = Arc::new(Mutex::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            })));

            use crate::model::DrawModel;
            match self.render_output_mode {
                RenderOutputMode::Unlit => {
                    render_pass.lock().unwrap().set_pipeline(&self.unlit_render_pipeline);
                    &self.models.par_iter().for_each(|model|{
                        let vb =model.instance_buffer.slice(..);
                        let mut locked_rp = render_pass.lock().unwrap();
                        locked_rp.set_vertex_buffer(1, vb );

                        for mesh in &model.meshes{
                            locked_rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                            locked_rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            if !&model.materials.is_empty() {
                                let material = &model.materials[mesh.material];
                                locked_rp.set_bind_group(0, &material.bind_group, &[]);
                            }
                            locked_rp.set_bind_group(1, &self.camera_bind_group, &[]);
                            locked_rp.draw_indexed(0..mesh.num_elements, 0, (0..model.instances.len() as u32).clone());
                        }
                    });
                }
                RenderOutputMode::Lit => {
                    render_pass.lock().unwrap().set_pipeline(&self.lit_render_pipeline);
                    &self.models.par_iter().for_each(|model|{
                        let vb =model.instance_buffer.slice(..);
                        let mut locked_rp = render_pass.lock().unwrap();
                        locked_rp.set_vertex_buffer(1, vb );

                        for mesh in &model.meshes{
                            locked_rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                            locked_rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            if !&model.materials.is_empty() {
                                let material = &model.materials[mesh.material];
                                locked_rp.set_bind_group(0, &material.bind_group, &[]);
                            }
                            locked_rp.set_bind_group(1, &self.camera_bind_group, &[]);
                            locked_rp.set_bind_group(2, &self.light_bind_group, &[]);
                            locked_rp.draw_indexed(0..mesh.num_elements, 0, (0..model.instances.len() as u32).clone());
                        }
                    });
                }
                RenderOutputMode::LitWithShadow => {
                    //println!("rendering Colored");
                    render_pass.lock().unwrap().set_pipeline(&self.render_pipeline);
                    &self.models.par_iter().for_each(|model|{
                        let vb =model.instance_buffer.slice(..);
                        let mut locked_rp = render_pass.lock().unwrap();
                        locked_rp.set_vertex_buffer(1, vb );

                        locked_rp.draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group, &self.shadow_bind_group);
                    });
                    // for model in &self.models {
                    //     render_pass.lock().unwrap().set_vertex_buffer(1, model.instance_buffer.slice(..) );
                    //     render_pass.lock().unwrap().draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group)
                    // }
                }
                RenderOutputMode::Wireframe => {
                    //println!("rendering Wireframe");
                    render_pass.lock().unwrap().set_pipeline(&self.wireframe_pipeline);
                    &self.models.par_iter().for_each(|model|{
                        let vb =model.instance_buffer.slice(..);
                        let mut locked_rp = render_pass.lock().unwrap();
                        locked_rp.set_vertex_buffer(1, vb );
                        locked_rp.draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group, &self.shadow_bind_group);
                    });
                    // for model in &self.models{
                    //     render_pass.lock().unwrap().set_vertex_buffer(1, model.instance_buffer.slice(..) );

                    //     render_pass.lock().unwrap().draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group);
                    // }
                }
                RenderOutputMode::DebugLitWithShadow => {
                    render_pass.lock().unwrap().set_pipeline(&self.render_pipeline);
                    &self.models.par_iter().for_each(|model|{
                        let vb =model.instance_buffer.slice(..);
                        let mut locked_rp = render_pass.lock().unwrap();
                        locked_rp.set_vertex_buffer(1, vb );

                        locked_rp.draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group, &self.shadow_bind_group);
                    });
                }
            }
        }

        //debug pass
        match self.render_output_mode {
            RenderOutputMode::DebugLitWithShadow => {
                let mut debug_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
                    label: Some("Debug Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops:wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                    //..Default::default()
                });

                debug_pass.set_pipeline(&self.debug_pass_pipeline);
                debug_pass.set_bind_group(0, &self.debug_pass_bind_group, &[]);
                debug_pass.set_vertex_buffer(0, self.debug_vertex_buffer.slice(..));
                debug_pass.set_index_buffer(self.debug_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                debug_pass.draw_indexed(0..self.debug_indices.len() as u32, 0, 0..1);
            }
            _ => {
                //skip debug pass
                }
        }

        //frame time graph pass
        match self.render_output_mode {
            RenderOutputMode::DebugLitWithShadow => {
                let mut frame_time_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
                    label: Some("Frame Time Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });

                frame_time_pass.set_pipeline(&self.frame_time_render_pipeline);
                frame_time_pass.set_vertex_buffer(0, self.frametime_vertex_buffer.slice(..));
                frame_time_pass.draw(0..256 as u32, 0..1 as u32);
            }

            _ => {}
        }
        
        self.queue.submit(std::iter::once(encoder.finish()));

        
        //experiment with picking value from texture
        //todo color id pass
        self.device.poll(wgpu::Maintain::Wait);
         
        if self.left_mouse_pressed {
            self.left_mouse_pressed = false;
            use wgpu::BufferDescriptor;
            let start = instant::Instant::now();

            let bytes_per_row = self.depth_texture.width() * 4;
            let aligned_bytes_per_row = align_up(bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            let temp_buffer_size = (aligned_bytes_per_row as u64 * self.depth_texture.height() as u64) as BufferAddress;
            let temp_buffer = Arc::new(Mutex::new(self.device.create_buffer(&BufferDescriptor { 
                label: Some("Temp Picking Buffer"), 
                size: temp_buffer_size, 
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                mapped_at_creation: false, 
            }))); 
            let temp_write_buffer = self.device.create_buffer(&BufferDescriptor { 
                label: Some("Temp Write Picking Buffer"), 
                size: temp_buffer_size, 
                usage: BufferUsages::COPY_SRC | BufferUsages::MAP_WRITE, 
                mapped_at_creation: false, 
            }); 
            use wgpu::CommandEncoderDescriptor;
            let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor { 
                label: Some("texture_copy_encoder") 
            });

            

            encoder.copy_texture_to_buffer(
                ImageCopyTexture{
                    texture: &self.depth_texture,
                    mip_level: 0,
                    origin: Origin3d {  //coordinate of the pixel to read
                        x: 0,
                        y: 0,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                }, 
                ImageCopyBuffer{
                    buffer: &temp_buffer.lock().unwrap(),
                    layout: ImageDataLayout{
                        offset: 0,
                        bytes_per_row: Some(aligned_bytes_per_row), //4bytes per pixel
                        rows_per_image: Some(self.depth_texture.height()),    // 1 row for each individual pixel
                    },
                }, 
                Extent3d {
                    width: self.depth_texture.width(), //read 1 pixel
                    height: self.depth_texture.height(),
                    depth_or_array_layers: 1,
                });

                self.queue.submit(std::iter::once(encoder.finish()));
                self.device.poll(wgpu::Maintain::Wait);
                let texture_width = self.depth_texture.width().clone();
                let mut done = Arc::new(Mutex::new(false));
                let done2 = Arc::clone(&done);
                let temp_buffer2 = Arc::clone(&temp_buffer);
                let (x, y) = (self.mouse_x, self.mouse_y);
                let buffer_f = temp_buffer.lock().unwrap().slice(..).map_async(wgpu::MapMode::Read,  move |result| {
                    match result {
                       Ok(()) => {
                            let mut g = done2.lock().unwrap();
                            *g = true;
                            let buf = temp_buffer2.lock().unwrap();
                            let data = buf.slice(..).get_mapped_range();
                            
                            //read Float32Depth texture pixel color/depth value at mouse coord
                            let pixels: &[f32] = bytemuck::cast_slice(&data);
                            let pixel_index = ((x + y * texture_width)*4) as usize;
                            let depth_value: f32 = pixels[pixel_index];

                            ////
                            //read rgba32float texture pixel color at mouse coord
                            // Calculate the index
                            //let index = ((x * texture_width + x) * 4) as usize;

                            // Get the RGBA color
                            // let r = pixels[index];
                            // let g = pixels[index + 1];
                            // let b = pixels[index + 2];
                            // let a = pixels[index + 3];
                            // let rgba_color = vec4(r, g, b, a);
                            // then find model in self.models with matching color id
                            /// 
                            let near = 0.1;
                            let far = 10000.0;

                            let linear_depth = near * far / (far - depth_value * ( far - near ));
                            let normalized_depth = linear_depth / far;

                            drop(data);
                            println!("Clicked Pixel Color {:?}",normalized_depth);
                            buf.unmap();
                        }
                        Err(e) => {
                            println!("Buffer Mapping flailed: {:?}", e);
                        }
                    }
                }); 
                self.device.poll(wgpu::Maintain::Wait);
                while !*done.lock().unwrap(){
                    //print!("waiting for gpu callback")
                }
                if *done.lock().unwrap() {
                    {
                        println!("done");
                    }
                    
                    let duration = instant::Instant::now() - start;
                    println!("Reading Selection took : {:?} sec", duration.as_secs_f32());
                }
                let texture_width = self.depth_texture.width().clone();

                let duration = instant::Instant::now() - start;
                println!("Reading Selection took : {:?} sec", duration.as_secs_f32());
        };
        

        

        output.present();
        Ok(())
    }

}

fn align_up(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) / alignment * alignment
}

pub async fn run(file_path: String, file_type:String, fullscreen_mode: String, use_hdr: bool) {
    let event_loop = EventLoop::new();
    let window_size: PhysicalSize<u32> = PhysicalSize { width: 1440, height: 1080 };
    //let icon = get_icon();
    let window = WindowBuilder::new()
        .with_inner_size(window_size)
        //.with_taskbar_icon(Some(icon.clone()))
        //.with_window_icon(Some(icon))
        .build(&event_loop)
        .unwrap();
    let mut window_mode = WindowMode::Windowed;
    match fullscreen_mode.as_str() {
        "fullscreen" => {   
                            window_mode = WindowMode::Fullscreen;
                            println!("fullscreen mode");
                            let mut monitor = event_loop
                                .available_monitors()
                                .next()
                                .expect("no monitor found!");
                            println!("Monitor: {:?}", monitor.name());

                            let mut mode = monitor.video_modes().nth(0).unwrap(); //next().expect("no mode found");
                            let fullscreen = Some(Fullscreen::Borderless(None)); //Some(Fullscreen::Exclusive(mode.clone()));
                            window.set_fullscreen(fullscreen);
                            window.set_cursor_grab(window::CursorGrabMode::Locked);

        },
        "windowed" => println!("windowed mode"),
        _ => println!("windowed mode"),
    };

    

    
    let mut state = State::new(window, file_path, file_type, use_hdr, window_mode).await;
    let mut last_render_time = instant::Instant::now();

    event_loop.run(move | event, _, control_flow | {*control_flow = ControlFlow::Poll; match event {
        Event::DeviceEvent { event: DeviceEvent::MouseMotion{delta,}, .. } => {
            match state.left_mouse_pressed{
                true => {state.mouse_x = delta.0 as u32; state.mouse_y= delta.1 as u32;}
                false => {}
            }
           if state.mouse_pressed {
            state.camera_controller.process_mouse(delta.0, delta.1)
            } else {
                state.camera_controller.process_mouse(delta.0, delta.1)
            } 
        }
        
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() && !state.input(event,) => {
                match event {
                    #[cfg(not(target_arch="wasm32"))]
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }

                    WindowEvent::DroppedFile(file_path) => {
                        println!("File Dropped, File Path: {:?}", file_path)
                    }

                    WindowEvent::HoveredFile(file_path) => {
                        println!("Hovered File, File Path: {:?}", file_path)
                    }
                    
                    WindowEvent::HoveredFileCancelled => {
                        println!("Hovered File Cancelled")
                    }
        
                    _ => {}
                }
            }
         

        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
            let now = instant::Instant::now();
            let dt = last_render_time.elapsed();
            //let fps = 1000000/dt.as_micros();
            //println!("fps : {fps}");
            last_render_time = now;
            state.update(dt);
            match state.render() {
                Ok(_) => {}

                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),

                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,

                Err(e) => eprintln!("{:?}", e),
            }
        }

        Event::MainEventsCleared => {
            state.window().request_redraw();
        }

        _ => {}
    }}
    );
}


use std::ffi::*;

#[no_mangle]
pub unsafe  extern "C" fn run_kanirenderer(file_path_cstring: *const c_char, file_type_c: *const c_char, fs_mode_c: *const c_char, hdr_c: *const c_char ){
    let file_path_cstr = CStr::from_ptr(file_path_cstring).to_str().expect("no path provided");
    let file_path: String =  file_path_cstr.into();
    if file_path.is_empty(){
        panic!("no file path provided")
    }
    let ft_cstr = CStr::from_ptr(file_type_c).to_str().unwrap_or("default");
    let file_type:String = ft_cstr.into(); //.unwrap_or("default").try_into().unwrap_or("default".to_string());
    let fs_cstr = CStr::from_ptr(fs_mode_c).to_str().unwrap_or("fullscreen");
    let fullscreen_mode:String = fs_cstr.into(); //unwrap_or("fullscreen").try_into().unwrap_or("fullscreen".to_string());
    let hdr_cstr = CStr::from_ptr(hdr_c).to_str().unwrap_or("false");
    let mut use_hdr: bool = false;
    match hdr_cstr {
        "true" => {use_hdr = true}
        "false" => {use_hdr = false}
        _ => {}
    }
    pollster::block_on(run(file_path, file_type, fullscreen_mode, use_hdr));
}