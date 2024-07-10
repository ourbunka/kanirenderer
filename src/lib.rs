use std::vec;
mod texture;
mod model;
mod resources;
mod camera;
mod light;
mod deferredRenderPipeline;

use bytemuck::Contiguous;
use light::{init_new_point_lights_buffer, PointLightData};
use model::{update_instance, Model, Vertex};
use cgmath::{num_traits::ToPrimitive, perspective, prelude::*};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use texture::Texture;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Fullscreen, self}, dpi::PhysicalSize,
};
use winit::window::Window;

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

struct State {
    free_cam : bool,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    render_pipeline: wgpu::RenderPipeline,
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
    movable_light: light::Light,
    movable_light_controller: light::MovableLightController,
    mouse_pressed: bool,
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

impl State {
    async fn new(window: Window, file_path: String, file_type:String) -> Self {
        let free_cam = true;
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter= instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
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

        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,//surface_caps.present_modes[2],
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
        
        let camera_controller = camera::CameraController::new(200.0, 0.4);

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


        let light_uniform = light::LightUniform {
            position: [0.0, 300.0, 0.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            range: 1.0,
        };

        let movable_light = light::Light::new([0.0, 300.0, 0.0], cgmath::Deg(-90.0),[1.0,1.0,1.0]);

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
            let color_r = i as f32 / 10.0;
            let new_light = light::Light::new([0.0, i.to_f32().unwrap()*100.0, 0.0], cgmath::Deg(-90.0), [0.50,0.50,0.50]);
            point_light.push(new_light);
            let new_light_data = point_light[i].generate_point_light_data();
            point_light_data.push(new_light_data);
        }
        let point_light_buffer = init_new_point_lights_buffer(point_light_data, &device);

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

            create_render_pipeline(&device, &render_pipeline_layout, config.format, Some(texture::Texture::DEPTH_FORMAT), &[model::ModelVertex::desc(), model::InstanceRaw::desc()], shader,)
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
        for i in 1..0 {
        let j = i;
        let test_mesh = resources::load_model("default_cube.obj",
                "opengl".to_string(), 
                &device, &queue, &texture_bind_group_layout,
                j,cgmath::Vector3 { x: (i.to_f32().unwrap()*10.0 as f32),
                y: (i.to_f32().unwrap()*2.0 as f32), z: 0.0 }).await.unwrap();

        models.push(test_mesh);
        movable_model_counts += j*j; 
        println!("pushed : {i}")
        } 
        println!("total movable model/object : {movable_model_counts}");
        
            


        Self {
            free_cam,
            window,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
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
            light_uniform,
            light_buffer,
            light_bind_group,
            mouse_pressed: false,
            //single point light to be removed after implementing movable light controller with ability to control each light in Vec<Light>
            movable_light,
            movable_light_controller,
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
            } => {  self.movable_light_controller.process_keyboard(*key, *state);
                    self.camera_controller.process_keyboard(*key, *state)
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
        
        
        
        use rayon::prelude::*;

        if self.models.len() > 1{
            ///// test moving models, will be ignored when model len() !>1 ///
            let newpos_x_vec = (1..self.models.len()).into_par_iter().map(|i| model::test_move_model(self.models[i].instances[0].position.x ,i, dt)).collect::<Vec<_>>();
            for i in 1..self.models.len(){
                self.models[i].instances[0].position.x = newpos_x_vec[i-1]
            }

            for i in 0..self.models.len(){
                if i != 0{
                    self.models[i].instances = update_instance(self.models[i].instance_num, self.models[i].instances[0].position);
                    let instance_data = self.models[i].instances.par_iter().map(model::Instance::to_raw).collect::<Vec<_>>();
                    //let len = instance_data.len();
                    //println!("model {i} , {len}");
                    self.queue.write_buffer(&self.models[i].instance_buffer, 0, bytemuck::cast_slice(&instance_data)); 
            }
            
        }
        }

        
        /////////
        
        
        self.queue.write_buffer(&self.camera_buffer, 0,bytemuck::cast_slice(&[self.camera_uniform]));
        
        self.queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[self.light_uniform]));

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
            render_pass.set_pipeline(&self.render_pipeline);
            for model in &self.models {
                render_pass.set_vertex_buffer(1, model.instance_buffer.slice(..) );
                render_pass.draw_model_instanced(model, 0..model.instances.len() as u32, &self.camera_bind_group, &self.light_bind_group)
            }
            
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

}

pub async fn run(file_path: String, file_type:String, fullscreen_mode: String) {
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

                            let mut mode = monitor.video_modes().next().expect("no mode found");
                            let fullscreen = Some(Fullscreen::Exclusive(mode.clone()));
                            window.set_fullscreen(fullscreen);

        },
        "windowed" => println!("windowed mode"),
        _ => println!("windowed mode"),
    };

    

    
    let mut state = State::new(window, file_path, file_type).await;
    let mut last_render_time = instant::Instant::now();

    event_loop.run(move | event, _, control_flow | {*control_flow = ControlFlow::Poll; match event {
        Event::DeviceEvent { event: DeviceEvent::MouseMotion{delta,}, .. } => if state.mouse_pressed {
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
pub unsafe  extern "C" fn run_kanirenderer(file_path_cstring: *const c_char, file_type_c: *const c_char, fs_mode_c: *const c_char ){
    let file_path_cstr = CStr::from_ptr(file_path_cstring).to_str().expect("no path provided");
    let file_path: String =  file_path_cstr.into();
    if file_path.is_empty(){
        panic!("no file path provided")
    }
    let ft_cstr = CStr::from_ptr(file_type_c).to_str().unwrap_or("default");
    let file_type:String = ft_cstr.into(); //.unwrap_or("default").try_into().unwrap_or("default".to_string());
    let fs_cstr = CStr::from_ptr(fs_mode_c).to_str().unwrap_or("fullscreen");
    let fullscreen_mode:String = fs_cstr.into(); //unwrap_or("fullscreen").try_into().unwrap_or("fullscreen".to_string());
    pollster::block_on(run(file_path, file_type, fullscreen_mode));
}
