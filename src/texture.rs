use crate::renderer::WgpuRenderer;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn from_bytes(renderer: &WgpuRenderer, bytes: &[u8], label: &str) -> anyhow::Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(renderer, &img, Some(label))
    }

    pub fn from_image(
        renderer: &WgpuRenderer,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> anyhow::Result<Self> {
        let rgba = img.to_rgba8();

        use image::GenericImageView;
        let (texture_width, texture_height) = img.dimensions();

        let size = wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        };
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        });

        renderer.queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * texture_width),
                rows_per_image: std::num::NonZeroU32::new(texture_height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }
}
