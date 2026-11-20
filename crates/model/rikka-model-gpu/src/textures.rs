use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_gpu::{Pixels, TextureFormat};
use rikka_image::Image as Raster;
use rikka_model_core::{ImageId, Model};
use rikka_renderer::{Renderer, TextureId as GpuTexture};
use rustc_hash::FxHashMap;

use crate::options::UploadOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Encoding {
    Srgb,
    Linear,
}

impl Encoding {
    fn format(self) -> TextureFormat {
        match self {
            Self::Srgb => TextureFormat::Rgba8Srgb,
            Self::Linear => TextureFormat::Rgba8Premultiplied,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Picture {
    Plain {
        image: ImageId,
        encoding: Encoding,
    },
    Channel {
        image: ImageId,
        channel: usize,
    },
    OcclusionRoughnessMetallic {
        occlusion: Option<ImageId>,
        metallic_roughness: Option<ImageId>,
    },
}

impl Picture {
    pub(crate) fn surface(
        occlusion: Option<ImageId>,
        metallic_roughness: Option<ImageId>,
    ) -> Option<Self> {
        if occlusion.is_none() || occlusion == metallic_roughness {
            return Some(Self::Plain {
                image: metallic_roughness.or(occlusion)?,
                encoding: Encoding::Linear,
            });
        }
        Some(Self::OcclusionRoughnessMetallic {
            occlusion,
            metallic_roughness,
        })
    }

    fn images(self) -> [Option<ImageId>; 2] {
        match self {
            Self::Plain { image, .. } | Self::Channel { image, .. } => [Some(image), None],
            Self::OcclusionRoughnessMetallic {
                occlusion,
                metallic_roughness,
            } => [occlusion, metallic_roughness],
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Pictures {
    pixels: Vec<(Picture, Pixels)>,
}

impl Pictures {
    pub(crate) fn prepare(model: &Model, wanted: &[Picture], options: &UploadOptions) -> Self {
        let mut uses: FxHashMap<ImageId, usize> = FxHashMap::default();
        for image in wanted.iter().flat_map(|picture| picture.images()).flatten() {
            *uses.entry(image).or_default() += 1;
        }
        let mut images: Vec<ImageId> = uses.keys().copied().collect();
        images.sort_unstable();
        let mut decoded = decode_all(model, &images, options);

        let mut pixels = Vec::with_capacity(wanted.len());
        for &picture in wanted {
            let made = match picture {
                Picture::Plain { image, encoding } => decoded
                    .get(&image)
                    .map(|raster| pixels_of(raster, encoding)),
                Picture::Channel { image, channel } => decoded
                    .get(&image)
                    .map(|raster| channel_of(raster, channel)),
                Picture::OcclusionRoughnessMetallic {
                    occlusion,
                    metallic_roughness,
                } => pack(
                    occlusion.and_then(|image| decoded.get(&image)),
                    metallic_roughness.and_then(|image| decoded.get(&image)),
                ),
            };
            pixels.extend(made.map(|made| (picture, made)));
            for image in picture.images().into_iter().flatten() {
                let left = uses.entry(image).or_default();
                *left = left.saturating_sub(1);
                if *left == 0 {
                    decoded.remove(&image);
                }
            }
        }
        Self { pixels }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Textures {
    uploaded: Vec<GpuTexture>,
    bound: FxHashMap<Picture, GpuTexture>,
}

impl Textures {
    pub(crate) fn upload(renderer: &mut Renderer, pictures: Pictures) -> Self {
        let mut textures = Self::default();
        for (picture, pixels) in pictures.pixels {
            match renderer.textures().create(pixels) {
                Ok(texture) => {
                    textures.uploaded.push(texture);
                    textures.bound.insert(picture, texture);
                }
                Err(error) => tracing::warn!(%error, "failed to upload model texture"),
            }
        }
        textures
    }

    pub(crate) fn get(&self, picture: Picture) -> Option<GpuTexture> {
        self.bound.get(&picture).copied()
    }

    pub(crate) fn take_uploaded(&mut self) -> Vec<GpuTexture> {
        self.bound.clear();
        std::mem::take(&mut self.uploaded)
    }
}

fn decode_all(
    model: &Model,
    images: &[ImageId],
    options: &UploadOptions,
) -> FxHashMap<ImageId, Raster> {
    let cores = std::thread::available_parallelism().map_or(1, |cores| cores.get());
    let workers = cores.min(images.len());
    if workers <= 1 {
        return images
            .iter()
            .filter_map(|&image| Some((image, decode(model, image, options)?)))
            .collect();
    }

    let next = AtomicUsize::new(0);
    let work = || {
        let mut decoded = Vec::new();
        while let Some(&image) = images.get(next.fetch_add(1, Ordering::Relaxed)) {
            decoded.extend(decode(model, image, options).map(|raster| (image, raster)));
        }
        decoded
    };
    std::thread::scope(|scope| {
        let decoders: Vec<_> = (0..workers)
            .map(|worker| {
                std::thread::Builder::new()
                    .name(format!("rikka-model-decode-{worker}"))
                    .spawn_scoped(scope, work)
                    .expect("failed to spawn model texture decoder")
            })
            .collect();
        decoders
            .into_iter()
            .flat_map(|decoder| {
                decoder
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

fn decode(model: &Model, image: ImageId, options: &UploadOptions) -> Option<Raster> {
    let model_image = model.images.get(image.index())?;
    let decoded = match Raster::from_bytes(&model_image.bytes) {
        Ok(decoded) => decoded,
        Err(error) => {
            tracing::warn!(image = model_image.name, %error, "failed to decode model image");
            return None;
        }
    };
    let Some(limit) = options.max_texture_size else {
        return Some(decoded);
    };
    if decoded.width() <= limit && decoded.height() <= limit {
        return Some(decoded);
    }
    match decoded.resize_to_fit(limit, limit, rikka_image::Filter::Lanczos3) {
        Ok(resized) => Some(resized),
        Err(error) => {
            tracing::warn!(image = model_image.name, %error, "failed to resize model image");
            Some(decoded)
        }
    }
}

fn pixels_of(raster: &Raster, encoding: Encoding) -> Pixels {
    let (width, height) = raster.dimensions();
    Pixels::new(
        width,
        height,
        encoding.format(),
        raster.rgba8().into_owned(),
    )
}

fn channel_of(raster: &Raster, channel: usize) -> Pixels {
    let (width, height) = raster.dimensions();
    let mut pixels = raster.rgba8().into_owned();
    for texel in pixels.as_chunks_mut::<4>().0 {
        *texel = [texel[channel]; 4];
    }
    Pixels::new(width, height, Encoding::Linear.format(), pixels)
}

fn pack(occlusion: Option<&Raster>, metallic_roughness: Option<&Raster>) -> Option<Pixels> {
    let shape = metallic_roughness.or(occlusion)?;
    let (width, height) = shape.dimensions();
    let mut pixels = match metallic_roughness {
        Some(raster) => raster.rgba8().into_owned(),
        None => Raster::filled(
            width,
            height,
            rikka_image::ColorType::Rgba8,
            rikka_types::Color::WHITE,
        )
        .ok()?
        .into_rgba8(),
    };

    if let Some(occlusion) = occlusion {
        let scaled = match occlusion.dimensions() == (width, height) {
            true => None,
            false => match occlusion.resize_exact(width, height, rikka_image::Filter::Bilinear) {
                Ok(resized) => Some(resized),
                Err(error) => {
                    tracing::warn!(%error, "failed to fit occlusion map to texture size");
                    None
                }
            },
        };
        let fitted = scaled
            .as_ref()
            .or(Some(occlusion).filter(|occlusion| occlusion.dimensions() == (width, height)));
        if let Some(fitted) = fitted {
            for (packed, red) in pixels
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(fitted.rgba8().as_chunks::<4>().0)
            {
                packed[0] = red[0];
            }
        }
    }
    Some(Pixels::new(
        width,
        height,
        Encoding::Linear.format(),
        pixels,
    ))
}
