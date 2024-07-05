use std::io::{BufReader, Cursor};
use anyhow::Ok;
use wgpu::util::DeviceExt;
use crate::{model::{self, Instance}, texture};
use cfg_if::cfg_if;
use cgmath::{num_traits::ToPrimitive, perspective, prelude::*, Vector3};

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new("") //std::path::Path::new(env!("OUT_DIR"))
                //.join("res")
                .join(file_name);
            //println!("our dir : {:?}", path);
            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let path = std::path::Path::new("") //std::path::Path::new(env!("OUT_DIR"))
                //.join("res")
                .join(file_name);
            //println!("our dir : {:?}", path);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}


pub async fn load_texture(file_name: &str, is_normal_map: bool, device: &wgpu::Device, queue: &wgpu::Queue,) -> anyhow::Result<texture::Texture> {
    let default_texture: Vec<u8> = include_bytes!("../res/default_normal.png").to_vec();
    let data = load_binary(file_name).await.unwrap_or(default_texture);
    texture::Texture::from_bytes(device, queue, &data, file_name, is_normal_map)
}

pub async fn load_opengl_texture(file_name: &str, is_normal_map: bool, device: &wgpu::Device, queue: &wgpu::Queue,) -> anyhow::Result<texture::Texture> {
    let default_texture: Vec<u8> = include_bytes!("../res/default_normal.png").to_vec();
    let data = load_binary(file_name).await.unwrap_or(default_texture);
    texture::Texture::from_opengl_bytes(device, queue, &data, file_name, is_normal_map)
}

pub async fn load_model(
    mut file_name: &str, 
    file_type: String,
    device: &wgpu::Device, 
    queue: &wgpu::Queue, 
    layout: &wgpu::BindGroupLayout,
    instance: u32,
    spawn_position: Vector3<f32> 
) -> anyhow::Result<model::Model> {
    let obj_text: String;
    if file_name.is_empty(){
        file_name = "default_cube.obj"
    }
    match file_name {
        "default_cube.obj"   => obj_text = load_string(file_name).await.unwrap_or(include_str!("../res/cube.obj").to_string()),
        _                   => obj_text = load_string(file_name).await?,
    }
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);


    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let mat_text: String;
            match file_name {
                "default_cube.obj"   => mat_text = load_string(&p).await.unwrap_or(include_str!("../res/cube.mtl").to_string()),
                _                   => mat_text = load_string(&p).await.unwrap(),
            }
            // = load_string(&p).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();

    for m in obj_materials? {
        let diffuse_texture: texture::Texture;
        let normal_texture: texture::Texture;
        let normal_bytes = include_bytes!("../res/default_normal.png");
        match file_type.as_str() {
            "default" =>    {   if !m.diffuse_texture.is_none() {
                                    diffuse_texture = load_texture(&m.diffuse_texture.unwrap().as_str(), false, device, queue).await?;
                                } else {
                                    println!("no diffuse textures, using fallback texture");
                                    diffuse_texture = texture::Texture::from_bytes(device, queue, normal_bytes, "using a default normal map as fallback diffuse texture", false).unwrap();
                                }
                
                                if !&m.normal_texture.is_none() {
                                    normal_texture = load_texture(&m.normal_texture.unwrap().as_str(), true, device, queue).await?;
                                } else {
                                    println!("no normal textures, using fallback texture");
                                    normal_texture = texture::Texture::from_bytes(device, queue, normal_bytes, "default_normal", true).unwrap();
                                    //load_texture("default_normal.png", device, queue).await?;
                                }
                            },
            "opengl" =>     {   if !m.diffuse_texture.is_none() {
                                    diffuse_texture = load_opengl_texture(&m.diffuse_texture.unwrap().as_str(), false, device, queue, ).await?;
                                } else {
                                    println!("no diffuse textures, using fallback texture");
                                    diffuse_texture = texture::Texture::from_bytes(device, queue, normal_bytes, "using a default normal map as fallback diffuse texture", false).unwrap();
                                }
                                if !&m.normal_texture.is_none() {
                                    normal_texture = load_opengl_texture(&m.normal_texture.unwrap().as_str(), true, device, queue).await?;
                                } else {
                                    println!("no normal textures, using fallback texture");
                                    normal_texture = texture::Texture::from_bytes(device, queue, normal_bytes, "default_normal", true).unwrap();
                                    //load_texture("default_normal.png", device, queue).await?;
                                }
                            },
            _ => panic!("no file type given"),
        }; 
        // let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     layout,
        //     entries: &[
        //         wgpu::BindGroupEntry {
        //             binding: 0,
        //             resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
        //         },
        //         wgpu::BindGroupEntry{
        //             binding: 1,
        //             resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
        //         },
        //     ],
        //     label: None,
        // });
        

        materials.push(model::Material::new(
            device,
            &m.name,
            diffuse_texture,
            normal_texture,
            layout,));
    }

    if materials.len() == 0 {
        println!("{:?} do not have any materials", file_name);
        println!("trying to add a default material");
        let normal_bytes = include_bytes!("../res/default_normal.png");
        let diffuse_texture = texture::Texture::from_bytes(device, queue, normal_bytes, "using a default normal map as fallback diffuse texture", false).unwrap();
        let normal_texture = texture::Texture::from_bytes(device, queue, normal_bytes, "default_normal", true).unwrap();

        materials.push(model::Material::new(
            device,
            "default material",
            diffuse_texture,
            normal_texture,
            layout,));
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let mut vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| model::ModelVertex {
                    position: [
                        m.mesh.positions[i *3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                    tangent: [0.0; 3],
                    bitangent: [0.0; 3],
                })
                .collect::<Vec<_>>();

            let indices = &m.mesh.indices;
            let mut triangles_included = vec![0; vertices.len()];

            for c in indices.chunks(3) {
                let v0 = vertices[c[0] as usize];
                let v1 = vertices[c[1] as usize];
                let v2 = vertices[c[2] as usize];

                let pos0: cgmath::Vector3<_> = v0.position.into();
                let pos1: cgmath::Vector3<_> = v1.position.into();
                let pos2: cgmath::Vector3<_> = v2.position.into();

                let uv0: cgmath::Vector2<_> = v0.tex_coords.into();
                let uv1: cgmath::Vector2<_> = v1.tex_coords.into();
                let uv2: cgmath::Vector2<_> = v2.tex_coords.into();

                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 *  delta_uv2.y - delta_pos2 * delta_uv1.y) * r;

                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

                vertices[c[0] as usize].tangent = (tangent + cgmath::Vector3::from(vertices[c[0] as usize].tangent)).into();
                vertices[c[1] as usize].tangent = (tangent + cgmath::Vector3::from(vertices[c[1] as usize].tangent)).into();
                vertices[c[2] as usize].tangent = (tangent + cgmath::Vector3::from(vertices[c[2] as usize].tangent)).into(); 
                vertices[c[0] as usize].bitangent = (bitangent + cgmath::Vector3::from(vertices[c[0] as usize].bitangent)).into();
                vertices[c[1] as usize].bitangent = (bitangent + cgmath::Vector3::from(vertices[c[1] as usize].bitangent)).into();
                vertices[c[2] as usize].bitangent = (bitangent + cgmath::Vector3::from(vertices[c[2] as usize].bitangent)).into();

                triangles_included[c[0] as usize] += 1;
                triangles_included[c[1] as usize] += 1;
                triangles_included[c[2] as usize] += 1;
            }

            for (i, n) in triangles_included.into_iter().enumerate() {
                let denom = 1.0/ n as f32;
                let mut v = &mut vertices[i];
                v.tangent = (cgmath::Vector3::from(v.tangent) * denom).into();
                v.bitangent = (cgmath::Vector3::from(v.bitangent) * denom).into();
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", file_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", file_name)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            model::Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

        let NUM_INSTANCES_PER_ROW: u32 = instance;

        const SPACE_BETWEEN: f32 = 3.0;

        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let mut position = cgmath::Vector3 { x, y: 0.0, z };
                    if instance == 1 {
                        position = spawn_position
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
                    
                    model::Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();
        let instance_data = instances.iter().map(model::Instance::to_raw).collect::<Vec<_>>();

        let instance_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }
        );

    Ok(model::Model {meshes, materials, instances, instance_buffer})
}

pub async fn load_default_cube(
device: &wgpu::Device, 
queue: &wgpu::Queue, 
layout: &wgpu::BindGroupLayout, 
) -> anyhow::Result<model::Model> {
    let default_cube = load_model("default_cube.obj", "opengl".to_string(), device, queue, layout,1, Vector3 { x: 0.0, y: 0.0, z: 0.0 }).await?;
    Ok(default_cube)
}
