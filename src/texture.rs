use anyhow::Result;
use image::GenericImageView;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn from_bytes(device: &wgpu::Device, bytes: &[u8]) -> Result<(Self, wgpu::CommandBuffer)> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, &img)
    }

    pub fn from_image(
        device: &wgpu::Device,
        img: &image::DynamicImage,
    ) -> Result<(Self, wgpu::CommandBuffer)> {
        let rgba = img.as_rgba8().unwrap();
        let dimensions = img.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("happy-tree"),
            // All textures are stored as 3d, we represent our 2d texture
            // by setting depth to 1.
            size: wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // SAMPLED tells wgpu that we want to use this texture in shaders
            // COPY_DST means that we want to copy data to this texture
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let buffer = device.create_buffer_with_data(&rgba, wgpu::BufferUsage::COPY_SRC);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("texture_buffer_copy_encoder"),
        });

        encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &buffer,
                offset: 0,
                bytes_per_row: 4 * dimensions.0,
                rows_per_image: dimensions.1,
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                array_layer: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth: 1,
            },
        );

        let cmd_buffer = encoder.finish();

        let view = texture.create_default_view();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare: wgpu::CompareFunction::Always,
        });

        Ok((
            Self {
                texture,
                view,
                sampler,
            },
            cmd_buffer,
        ))
    }
}
