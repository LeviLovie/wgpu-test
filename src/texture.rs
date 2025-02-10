use anyhow::*;
use image::GenericImageView;

pub struct Texture {
    #[allow(unused)]
    pub texture: egui_wgpu::wgpu::Texture,
    pub view: egui_wgpu::wgpu::TextureView,
    pub sampler: egui_wgpu::wgpu::Sampler,
}

impl Texture {
    pub fn from_bytes(
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = egui_wgpu::wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&egui_wgpu::wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: egui_wgpu::wgpu::TextureDimension::D2,
            format: egui_wgpu::wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: egui_wgpu::wgpu::TextureUsages::TEXTURE_BINDING
                | egui_wgpu::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            egui_wgpu::wgpu::ImageCopyTexture {
                aspect: egui_wgpu::wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: egui_wgpu::wgpu::Origin3d::ZERO,
            },
            &rgba,
            egui_wgpu::wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&egui_wgpu::wgpu::SamplerDescriptor {
            address_mode_u: egui_wgpu::wgpu::AddressMode::ClampToEdge,
            address_mode_v: egui_wgpu::wgpu::AddressMode::ClampToEdge,
            address_mode_w: egui_wgpu::wgpu::AddressMode::ClampToEdge,
            mag_filter: egui_wgpu::wgpu::FilterMode::Linear,
            min_filter: egui_wgpu::wgpu::FilterMode::Nearest,
            mipmap_filter: egui_wgpu::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }

    pub const DEPTH_FORMAT: egui_wgpu::wgpu::TextureFormat =
        egui_wgpu::wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(
        device: &egui_wgpu::wgpu::Device,
        config: &egui_wgpu::wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let size = egui_wgpu::wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = egui_wgpu::wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: egui_wgpu::wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: egui_wgpu::wgpu::TextureUsages::RENDER_ATTACHMENT
                | egui_wgpu::wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&egui_wgpu::wgpu::SamplerDescriptor {
            address_mode_u: egui_wgpu::wgpu::AddressMode::ClampToEdge,
            address_mode_v: egui_wgpu::wgpu::AddressMode::ClampToEdge,
            address_mode_w: egui_wgpu::wgpu::AddressMode::ClampToEdge,
            mag_filter: egui_wgpu::wgpu::FilterMode::Linear,
            min_filter: egui_wgpu::wgpu::FilterMode::Linear,
            mipmap_filter: egui_wgpu::wgpu::FilterMode::Nearest,
            compare: Some(egui_wgpu::wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }
}
