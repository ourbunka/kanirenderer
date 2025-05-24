use image::{DynamicImage, GenericImageView};
use anyhow::*;
use std::result::Result::Ok;
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

fn invert_green_channel(image: &mut DynamicImage) {
    match image {
        DynamicImage::ImageRgb8(img) => {
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let mut new_pixel = *pixel;
                new_pixel[1] = 255 - pixel[1];
                *pixel = new_pixel;
            }
        },
        DynamicImage::ImageRgba8(img) => {
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let mut new_pixel = *pixel;
                new_pixel[1] = 255 - pixel[1];
                *pixel = new_pixel;
            }
        },
        DynamicImage::ImageRgb16(img) => {
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let mut new_pixel = *pixel;
                new_pixel[1] = 65535 - pixel[1]; // 16-bit inversion
                *pixel = new_pixel;
            }
        },
        DynamicImage::ImageRgba16(img) => {
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let mut new_pixel = *pixel;
                new_pixel[1] = 65535 - pixel[1]; // 16-bit inversion
                *pixel = new_pixel;
            }
        },
        DynamicImage::ImageRgb32F(img) => {
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let mut new_pixel = *pixel;
                new_pixel[1] = 1.0 - pixel[1]; // 32-bit float inversion
                *pixel = new_pixel;
            }
        },
        DynamicImage::ImageRgba32F(img) => {
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let mut new_pixel = *pixel;
                new_pixel[1] = 1.0 - pixel[1]; // 32-bit float inversion
                *pixel = new_pixel;
            }
        },
        _ => {
            println!("Unsupported image format for green channel inversion.");
        }
    }
}

impl Texture {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label:&str,
        is_normal_map: bool,
    ) -> Result<Self> {
        let img_res = image::load_from_memory(bytes);
        let mut img;
        match img_res {
            Ok(i) => {img = i;}
            Err(err) => {println!("{:?}",err); return Err(err.into());}
        }
        Self::from_image(device, queue, &img, Some(label), is_normal_map)
    }

    pub fn from_opengl_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label:&str,
        is_normal_map: bool,
    ) -> Result<Self> {
        let img_res = image::load_from_memory(bytes);
        let mut img;
        match img_res {
            Ok(i) => {img = i;}
            Err(err) => {println!("{:?}",err); return Err(err.into());}
        }
        let mut flippedimg = img.flipv();
        if is_normal_map{
            invert_green_channel(&mut flippedimg);
        }
        Self::from_image(device, queue, &flippedimg, Some(label), is_normal_map)
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
        is_normal_map: bool,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let format = if is_normal_map{
            match img.color(){
                image::ColorType::L8 => {wgpu::TextureFormat::Rgba8Unorm},
                image::ColorType::La8 => {wgpu::TextureFormat::Rgba8Unorm},
                image::ColorType::Rgb8=> {wgpu::TextureFormat::Rgba8Unorm},
                image::ColorType::Rgba8=> {wgpu::TextureFormat::Rgba8Unorm},
                image::ColorType::L16=> {wgpu::TextureFormat::Rgba16Unorm},
                image::ColorType::La16=> {wgpu::TextureFormat::Rgba16Unorm},
                image::ColorType::Rgb16=> {wgpu::TextureFormat::Rgba16Unorm},
                image::ColorType::Rgba16=> {wgpu::TextureFormat::Rgba16Unorm},
                image::ColorType::Rgb32F=> {wgpu::TextureFormat::Rgba32Float},
                image::ColorType::Rgba32F=> {wgpu::TextureFormat::Rgba32Float},
                _ => {wgpu::TextureFormat::Rgba8Unorm},
                            }
        } else {
            wgpu::TextureFormat::Rgba8UnormSrgb
        };

        let texture = device.create_texture(
            &wgpu::TextureDescriptor {
                label,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            }
        );

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                border_color: None,
                ..Default::default()
            }
        );

        Ok(Self { texture, view, sampler })
    }

    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, label: &str) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT ,
            view_formats: &[],
        };

        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                ..Default::default()
            }
        );

        Self { texture, view, sampler }
    }
}