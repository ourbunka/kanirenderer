use wgpu::{*};
use winit::dpi::PhysicalSize;

pub fn new_gbuffer_texture_bind_group(
    device: &wgpu::Device,
    g_buffer_texture_layout: &wgpu::BindGroupLayout,
    size: PhysicalSize<u32>)-> (TextureView, TextureView, TextureView ,BindGroup){

    let gbuffer_texture_2d_float16 = device.create_texture(&wgpu::TextureDescriptor{
        size: Extent3d { width: size.width, height: size.height, ..Default::default()},
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        format: wgpu::TextureFormat::Rgba16Float,
        mip_level_count:1,
        sample_count:1,
        dimension: wgpu::TextureDimension::D2,
        label: None,
        view_formats: &[wgpu::TextureFormat::Rgba16Float]
    });
    let gbuffer_texture_2d_float16_view = gbuffer_texture_2d_float16.create_view(&wgpu::TextureViewDescriptor{
        ..Default::default()
    });
    let gbuffer_texture_albedo = device.create_texture(&wgpu::TextureDescriptor{
        size: Extent3d { width: size.width, height: size.height, ..Default::default()},
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        format: wgpu::TextureFormat::Bgra8Unorm,
        mip_level_count:1,
        sample_count:1,
        dimension: wgpu::TextureDimension::D2,
        label: None,
        view_formats: &[wgpu::TextureFormat::Bgra8Unorm]
    });
    let gbuffer_texture_albedo_view = gbuffer_texture_albedo.create_view(&wgpu::TextureViewDescriptor{
        ..Default::default()
    });
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor{
        size: Extent3d { width: size.width, height: size.height, ..Default::default()},
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        format: wgpu::TextureFormat::Depth24Plus,
        mip_level_count:1,
        sample_count:1,
        dimension: wgpu::TextureDimension::D2,
        label: None,
        view_formats: &[wgpu::TextureFormat::Depth24Plus]
    });
    let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor{
        ..Default::default()
    });
    
    
    let gbuffer_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor{
        layout: &g_buffer_texture_layout,
        entries: &[
            wgpu::BindGroupEntry{
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&gbuffer_texture_2d_float16_view),
            },
            wgpu::BindGroupEntry{
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gbuffer_texture_albedo_view),
            },
            wgpu::BindGroupEntry{
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&depth_texture_view),
            }
        ],
        label: None,
    });
    (gbuffer_texture_2d_float16_view, gbuffer_texture_albedo_view, depth_texture_view, gbuffer_texture_bind_group)
}

pub fn create_gbuffer_texture_bind_group_layout(
    device: &wgpu::Device,
    bind_group: &BindGroup){
    let gbuffer_texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
        entries: &[
            wgpu::BindGroupLayoutEntry{
                binding:0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { 
                    sample_type: wgpu::TextureSampleType::Float { filterable: false }, 
                    view_dimension: wgpu::TextureViewDimension::D2, 
                    multisampled: false 
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry{
                binding:1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { 
                    sample_type: wgpu::TextureSampleType::Float { filterable: false }, 
                    view_dimension: wgpu::TextureViewDimension::D2, 
                    multisampled: false 
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry{
                binding:2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { 
                    sample_type: wgpu::TextureSampleType::Depth, 
                    view_dimension: wgpu::TextureViewDimension::D2, 
                    multisampled: false 
                },
                count: None,
            }
        ],
        label: Some("G Buffer Bind Group Layout"),
    });

}

pub fn create_write_gbuffer_pipeline(
    device: &wgpu::Device,
    g_buffer_texture_layout: &wgpu::BindGroupLayout,
    vertex_layouts: &[wgpu::VertexBufferLayout],) {
    
    let vertex_write_gbuffer_shader_module_desc = wgpu::ShaderModuleDescriptor {
        label: Some("Vertex Write GBuffer Sahder"),
        source: wgpu::ShaderSource::Wgsl(include_str!("vertexWriteGBuffers.wgsl").into()),
    };

    let shader_vertex_write_gbuffer = device.create_shader_module(vertex_write_gbuffer_shader_module_desc);
    
    let write_gbuffer_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
        label: Some("write G buffer Pipeline Layout"),
        bind_group_layouts: &[g_buffer_texture_layout],
        push_constant_ranges: &[],
    });

    let fragment_write_gbuffer_shader_desc = wgpu::ShaderModuleDescriptor {
        label: Some("Fragment Write G Buffer Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("fragmentWriteGBuffers.wgsl").into()),
    };

    let shader_fragment_write_gbuffer = device.create_shader_module(fragment_write_gbuffer_shader_desc);

    let write_gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
        label: Some("write G buffer Pipeline"),
        layout: Some(&write_gbuffer_pipeline_layout),
        vertex:wgpu::VertexState{
            module: &shader_vertex_write_gbuffer,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_fragment_write_gbuffer,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::all(),
            }),
            Some(wgpu::ColorTargetState{
                format: wgpu::TextureFormat::Bgra8Unorm,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::all(),
            })],
        }),
        depth_stencil: Some(
            wgpu::DepthStencilState{
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
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
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,

    });
}


pub fn create_deferred_render_pipeline(
    device: &wgpu::Device,
    g_buffer_texture_layout: &wgpu::BindGroupLayout,
    light_buffer_layout: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
) -> (wgpu::ShaderModule, wgpu::ShaderModule, wgpu::PipelineLayout,wgpu::RenderPipeline) {
    let deferred_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
        label:Some("Deferred Render Pipeline Layout"),
        bind_group_layouts: & [
            g_buffer_texture_layout,
            light_buffer_layout
        ],
        push_constant_ranges: &[],
    });

    let shader_vertex_texture_quad = wgpu::ShaderModuleDescriptor{
        label:Some("Vertex Texture Quad Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("vertexTextureQuad.wgsl").into()),
    };

    let shader_mod_vertex_texture_quad = device.create_shader_module(shader_vertex_texture_quad);

    let shader_fragment_deferred_rendering = wgpu::ShaderModuleDescriptor{
        label:Some("Fragment Deferred Rendering Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("fragmentDeferredRendering.wgsl").into()),
    };

    let shader_mod_fragment_deferred_rendering = device.create_shader_module(shader_fragment_deferred_rendering);


    let deferred_render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Deferred Render Pipeline"),
        layout: Some(&deferred_pipeline_layout),
        //vertextquad.wgsl
        vertex: wgpu::VertexState {
            module: &shader_mod_vertex_texture_quad,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        //fragmentdeferredrendering.wgsl
        fragment: Some(wgpu::FragmentState {
            module: &shader_mod_fragment_deferred_rendering,
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
    });
    (shader_mod_vertex_texture_quad, shader_mod_fragment_deferred_rendering, deferred_pipeline_layout, deferred_render_pipeline)
}