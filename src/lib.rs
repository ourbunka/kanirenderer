#![allow(warnings)] 

use std::{clone, vec};
mod texture;
mod model;
mod resources;
mod camera;
mod light;
mod deferredRenderPipeline;

use bytemuck::Contiguous;
use light::{init_new_directional_lights_Uniform, init_new_point_lights_buffer, DirectionalLight, DirectionalLightUniformData, PointLightData};
use model::{update_instance, Model, Vertex};
use cgmath::{num_traits::ToPrimitive, perspective, prelude::*};
use rand::Rng;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use texture::Texture;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Fullscreen, self}, dpi::PhysicalSize,
};
use winit::window::Window;
use wgpu::TextureFormat;
use crate::resources::*;




#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_position: [f32;4],
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        //use cgmath::SquareMatrix;
        Self {
            view_position: [0.0; 4],
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &camera::Camera, projection: &camera::Projection) {
        //using vec4 because of uniforms 16byte requirement
        self.view_position = camera.position.to_homogeneous().into();
        self.view_proj = (projection.calc_matrix() * camera.calc_matrix()).into();
    }

    
}


#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    position: [f32; 3],
    _padding: u32,
    color: [f32; 3],
    _padding2: u32,
}

enum RenderOutputMode {
    Colored,
    Wireframe,
}

struct State {
    free_cam : bool,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    render_pipeline: wgpu::RenderPipeline,
    wireframe_pipeline: wgpu::RenderPipeline,
    camera: camera::Camera,
    projection: camera::Projection,
    camera_controller: camera::CameraController,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // instances: Vec<Instance>,
    // instance_buffer: wgpu::Buffer,
    depth_texture: Texture,
    models : Vec<model::Model>,
    light_uniform: light::LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    point_light: Vec<light::Light>,
    point_light_buffer: wgpu::Buffer,
    directional_light: light::DirectionalLight,
    directional_light_uniform_data: light::DirectionalLightUniformData,
    directional_light_uniform: wgpu::Buffer,
    movable_light: light::Light,
    movable_light_controller: light::MovableLightController,
    mouse_pressed: bool,
    render_output_mode: RenderOutputMode,
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
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
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
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
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
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
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
        wgpu::Features::POLYGON_MODE_LINE
    }

impl State {
    async fn new(window: Window, file_path: String, file_type:String, use_hdr: bool) -> Self {
        let free_cam = true;
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
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
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        if use_hdr{
            surface_format = TextureFormat::Rgba16Float;
        }
        println!("{:?}", surface_format);
        
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,//surface_caps.present_modes[2],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
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

        let projection = camera::Projection::new(config.width, config.height, cgmath::Deg(45.0), 0.1, 100.0);
        
        let camera_controller = camera::CameraController::new(300.0, 0.4);

        let mut camera_uniform = CameraUniform::new();
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

        let shadow_depth_texture_size = 1024;
        let shadow_depth_texture_size_extent3d = wgpu::Extent3d{
            width: shadow_depth_texture_size,
            height: shadow_depth_texture_size,
            depth_or_array_layers:1,
        };
        let shadow_depth_texture =  device.create_texture(
            &wgpu::TextureDescriptor{
                label: Some("shadow depth texture"),
                size: shadow_depth_texture_size_extent3d,
                mip_level_count:1,
                sample_count:1,
                dimension:wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }
        );
        let shadow_depth_texture_view = shadow_depth_texture.create_view(
            &wgpu::TextureViewDescriptor::default()
        );
        
        //let smp_vertex_buffer = wgpu::VertexBufferLayout

        let shadow_depth_uniform_buffer_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            entries: &[
                wgpu::BindGroupLayoutEntry{
                    binding:0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { 
                        ty: wgpu::BufferBindingType::Uniform, 
                        has_dynamic_offset: false, 
                        min_binding_size: None 
                    },
                    count: None,
                }
            ],
            label: None
        });

        // let smp_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { 
        //     label: None, 
        //     entries: &[
        //         wgpu::nomd
        //     ],
        // });


        let light_uniform = light::LightUniform {
            position: [0.0, 300.0, 0.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            range: 1.0,
        };

        let movable_light = light::Light::new([0.0, 300.0, 0.0], cgmath::Deg(-90.0),[0.01,0.01,0.01], 0.05);

        let movable_light_controller = light::MovableLightController::new(300.0, 1.0);

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
            let color_r = 0.1;
            let mut rng = rand::rng();
            let random_r = rng.random_range(0.0..1.0).to_f32().unwrap();
            let random_g = rng.random_range(0.0..1.0).to_f32().unwrap();
            let random_b = rng.random_range(0.0..1.0).to_f32().unwrap();
            let random_pos = [
                rng.random_range(-1500.0..1500.0).to_f32().unwrap(), 
                rng.random_range(0.0..55.0).to_f32().unwrap(), 
                rng.random_range(-1500.0..1500.0).to_f32().unwrap()
                ];
            let new_light = light::Light::new(random_pos, cgmath::Deg(-90.0), [0.01,0.01,0.01], 0.01);
            point_light.push(new_light);
            let new_light_data = point_light[i].generate_point_light_data();
            point_light_data.push(new_light_data);
        }
        let point_light_buffer = init_new_point_lights_buffer(point_light_data, &device);

        let directional_light = light::DirectionalLight::new([0.2, -1.0, -0.3], [1.0,1.0,1.0]);

        
        let directional_light_uniform = directional_light.generate_directional_light_data();
        
        let directional_light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Directional Light Buffer"),
            contents: bytemuck::cast_slice(&[directional_light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        //let directional_light_uniform = init_new_directional_lights_Uniform(directional_light_uniform_data.clone(), &device);

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

        // const NUM_INSTANCES_PER_ROW: u32 = 1;

        // const SPACE_BETWEEN: f32 = 3.0;

        // let instances = (0..NUM_INSTANCES_PER_ROW)
        //     .flat_map(|z| {
        //         (0..NUM_INSTANCES_PER_ROW).map(move |x| {
        //             let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
        //             let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

        //             let position = cgmath::Vector3 { x, y: 0.0, z };

        //             let rotation = if position.is_zero() {
        //                 cgmath::Quaternion::from_axis_angle(
        //                     cgmath::Vector3::unit_z(),
        //                     cgmath::Deg(0.0),
        //                 )
        //             } else {
        //                 cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
        //             };

        //             Instance { position, rotation }
        //         })
        //     })
        //     .collect::<Vec<_>>();
        // let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();

        // let instance_buffer = device.create_buffer_init(
        //     &wgpu::util::BufferInitDescriptor {
        //         label: Some("Instance Buffer"),
        //         contents: bytemuck::cast_slice(&instance_data),
        //         usage: wgpu::BufferUsages::VERTEX,
        //     }
        // );

        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        //to do
        let useDeferredRenderer = false;

        if useDeferredRenderer{
            todo!();
        } else {
            
        }

        let shadow_depth_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label:None,
            bind_group_layouts: &[
                &shadow_depth_uniform_buffer_bind_group_layout,
                &camera_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // let shadow_depth_render_pipeline = {
        //     let shader_module_desc = wgpu::ShaderModuleDescriptor{
        //         label: None,
        //         source: wgpu::ShaderSource::Wgsl(include_str!("vertexShadow.wgsl").into()),
        //     };
        //     let shader = device.create_shader_module(shader_module_desc);
        //     device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
        //         label: None,
        //         layout: Some(&shadow_depth_pipeline_layout),
        //         vertex: wgpu::VertexState {
        //             module: &shader,
        //             entry_point: "vs_main",
        //             buffers: &[model::ModelVertex::desc(), model::InstanceRaw::desc()],
        //         },
        //         fragment: None,
        //         primitive: wgpu::PrimitiveState {
        //             topology: wgpu::PrimitiveTopology::TriangleList,
        //             strip_index_format: None,
        //             front_face: wgpu::FrontFace::Ccw,
        //             cull_mode: Some(wgpu::Face::Back),
        //             polygon_mode: wgpu::PolygonMode::Fill,
        //             unclipped_depth: false,
        //             conservative: false,
        //         },
        //         depth_stencil: Some(texture::Texture::DEPTH_FORMAT).map(|format| wgpu::DepthStencilState{
        //             format,
        //             depth_write_enabled: true,
        //             depth_compare: wgpu::CompareFunction::Less,
        //             stencil: wgpu::StencilState::default(),
        //             bias: wgpu::DepthBiasState::default(),
        //         }),
        //         multisample: wgpu::MultisampleState {
        //             count: 1,
        //             mask: !0,
        //             alpha_to_coverage_enabled: false,
        //         },
        //         multiview: None,
        //     })
        // };

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &light_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            };

            create_render_pipeline(&device, 
                &render_pipeline_layout, 
                config.format, 
                Some(wgpu::TextureFormat::Depth24PlusStencil8), 
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
                Some(wgpu::TextureFormat::Depth24PlusStencil8), 
                &[model::ModelVertex::desc(), 
                model::InstanceRaw::desc()], 
                shader)
        };

        // let depth_texture_texture = device.create_texture(&wgpu::TextureDescriptor{
        //     label: Some("depth texture"),
        //     size: wgpu::Extent3d { 
        //         width: config.width, 
        //         height: config.height, 
        //         depth_or_array_layers: 1, 
        //     },
        //     mip_level_count:1,
        //     sample_count:1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::Depth24PlusStencil8,
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        //     view_formats: &[],
        // });

        


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
        for i in 1..=0 {
        let test_mesh = resources::load_model("default_cube.obj",
                "opengl".to_string(), 
                &device, &queue, &texture_bind_group_layout,
                5,cgmath::Vector3 { x: (rng.random_range(-1500.0..1500.0) as f32),
                y: (rng.random_range(30.0..100.0)  as f32), z: (rng.random_range(-1500.0..1500.0)  as f32) }).await.unwrap();

        models.push(test_mesh); 
        println!("pushed : {i}");
        movable_model_counts +=1;
        } 
        println!("total movable model/object : {movable_model_counts}");
        
        let mut render_output_mode = RenderOutputMode::Colored;


        Self {
            free_cam,
            window,
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
            models,
            point_light,
            point_light_buffer,
            directional_light,
            directional_light_uniform_data: directional_light_uniform,
            directional_light_uniform: directional_light_buffer,
            light_uniform,
            light_buffer,
            light_bind_group,
            mouse_pressed: false,
            //single point light to be removed after implementing movable light controller with ability to control each light in Vec<Light>
            movable_light,
            movable_light_controller,
            render_output_mode
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
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
            
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

                    //handle render mode switching, to redo
                    if *key == VirtualKeyCode::Tab && *state == ElementState::Released {
                       match self.render_output_mode {
                            RenderOutputMode::Colored => {self.render_output_mode = RenderOutputMode::Wireframe; true}
                            RenderOutputMode::Wireframe => {self.render_output_mode = RenderOutputMode::Colored; true}
                        } 
                    } else {
                        false
                    }
                    
                }
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput { button: MouseButton::Right, state, .. } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            _ => false,
        }
    }


    fn update(&mut self, dt: instant::Duration) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.movable_light_controller.update_light(&mut self.movable_light, &mut self.light_uniform, dt);
        self.camera_uniform.update_view_proj(&self.camera, &self.projection);
        
        
        
        // use rayon::prelude::*;

        // if self.models.len() > 1{
        //     ///// test moving models, will be ignored when model len() !>1 ///
        //     let newpos = (1..self.models.len()).into_par_iter().map(|i| model::test_move_model_vec3(self.models[i].instances[0].position , dt)).collect::<Vec<_>>();
        //     for i in 1..self.models.len(){
        //         self.models[i].instances[0].position = newpos[i-1]
        //     }

        //     for i in 0..self.models.len(){
        //         if i != 0{
        //             self.models[i].instances = update_instance(self.models[i].instance_num, self.models[i].instances[0].position);
        //             let instance_data = self.models[i].instances.par_iter().map(model::Instance::to_raw).collect::<Vec<_>>();
        //             self.queue.write_buffer(&self.models[i].instance_buffer, 0, bytemuck::cast_slice(&instance_data)); 
        //     }
            
        // }
        // }

        
        /////////
        
        
        self.queue.write_buffer(&self.camera_buffer, 0,bytemuck::cast_slice(&[self.camera_uniform]));
        
        self.queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[self.light_uniform]));
        self.queue.write_buffer(&self.directional_light_uniform, 0, bytemuck::cast_slice(&[self.directional_light_uniform_data]));

    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture.create_view(&wgpu:: TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            //render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            use crate::model::DrawModel;
            match self.render_output_mode {
                RenderOutputMode::Colored => {
                    //println!("rendering Colored");
                    render_pass.set_pipeline(&self.render_pipeline);
                    for model in &self.models {
                        render_pass.set_vertex_buffer(1, model.instance_buffer.slice(..) );
                        render_pass.draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group)
                    }
                }
                RenderOutputMode::Wireframe => {
                    render_pass.set_pipeline(&self.wireframe_pipeline);
                    //println!("rendering Wireframe");
                    for model in &self.models{
                        render_pass.set_vertex_buffer(1, model.instance_buffer.slice(..) );

                        render_pass.draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group);
                    }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

}



pub async fn run(file_path: String, file_type:String, fullscreen_mode: String, use_hdr: bool) {
    env_logger::init();
    
    let event_loop = EventLoop::new();
    let window_size: PhysicalSize<u32> = PhysicalSize { width: 1440, height: 1080 };
    let window = WindowBuilder::new().with_inner_size(window_size).build(&event_loop).unwrap();

    match fullscreen_mode.as_str() {
        "fullscreen" => {
                            println!("fullscreen mode");
                            let mut monitor = event_loop
                                .available_monitors()
                                .next()
                                .expect("no monitor found!");
                            println!("Monitor: {:?}", monitor.name());

                            let mut mode = monitor.video_modes().nth(0).unwrap(); //next().expect("no mode found");
                            let fullscreen = Some(Fullscreen::Borderless(None)); //Some(Fullscreen::Exclusive(mode.clone()));
                            window.set_fullscreen(fullscreen);

        },
        "windowed" => println!("windowed mode"),
        _ => println!("windowed mode"),
    };

    

    
    let mut state = State::new(window, file_path, file_type, use_hdr).await;
    let mut last_render_time = instant::Instant::now();

    event_loop.run(move | event, _, control_flow | {*control_flow = ControlFlow::Poll; match event {
        Event::DeviceEvent { event: DeviceEvent::MouseMotion{delta,}, .. } => if state.mouse_pressed {
            state.camera_controller.process_mouse(delta.0, delta.1)
        } else {
            state.camera_controller.process_mouse(delta.0, delta.1)
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
        
                    _ => {}
                }
            }
         

        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
            let now = instant::Instant::now();
            let dt = now - last_render_time;
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