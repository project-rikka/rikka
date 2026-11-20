use std::path::Path;
use std::sync::Arc;

use animate_look::Asset;
use rikka_color_lookup::ColorLookup;
use rikka_gpu::{Filtering, Pixels, TextureFormat, TextureId};
use rikka_ies::IesProfile;
use rikka_image::Image;
use rikka_renderer::{Renderer, Uploads};
use rikka_renderer_3d::{
    LightProfileId, Panorama, PanoramaSun, TextureBinding, TextureKind, Three,
};
use rikka_sync::Mutex;
use rustc_hash::FxHashMap;

use crate::dressing::wake::Wake;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Wanted {
    Texture { path: Asset, srgb: bool },
    Lookup(Asset),
    Panorama(Asset),
    Sun { path: Asset, threshold: u32 },
    Profile(Asset),
}

impl Wanted {
    fn path(&self) -> &Asset {
        match self {
            Self::Texture { path, .. }
            | Self::Lookup(path)
            | Self::Panorama(path)
            | Self::Sun { path, .. }
            | Self::Profile(path) => path,
        }
    }
}

#[derive(Clone, Debug)]
enum Loaded {
    Texture(TextureId),
    Sun(Option<PanoramaSun>),
    Profile(Arc<IesProfile>),
}

#[derive(Debug)]
enum Loading {
    Pending,
    Ready(Loaded),
    Failed(String),
    Reported,
}

type Shelf = Arc<Mutex<FxHashMap<Wanted, Loading>>>;

#[derive(Debug, Default)]
pub(super) struct Assets {
    shelf: Shelf,
    uploads: Option<Uploads>,
    profiles: FxHashMap<Asset, Option<LightProfileId>>,
}

pub fn is_color_lookup(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("cube") || extension.eq_ignore_ascii_case("3dl")
    })
}

impl Assets {
    pub(super) fn texture(
        &mut self,
        renderer: &Renderer,
        path: &str,
        kind: TextureKind,
        wake: &Wake,
        failures: &mut Vec<String>,
    ) -> Option<TextureBinding> {
        let path = Asset::new(path);
        let wanted = match is_color_lookup(Path::new(path.as_str())) {
            true => Wanted::Lookup(path),
            false => Wanted::Texture {
                path,
                srgb: kind == TextureKind::Colour,
            },
        };
        match self.fetch(renderer, wanted, wake, failures)? {
            Loaded::Texture(id) => Some(TextureBinding::new(id, Filtering::default())),
            Loaded::Sun(_) | Loaded::Profile(_) => None,
        }
    }

    pub(super) fn panorama(
        &mut self,
        renderer: &Renderer,
        path: &str,
        wake: &Wake,
        failures: &mut Vec<String>,
    ) -> Option<TextureId> {
        match self.fetch(renderer, Wanted::Panorama(Asset::new(path)), wake, failures)? {
            Loaded::Texture(id) => Some(id),
            Loaded::Sun(_) | Loaded::Profile(_) => None,
        }
    }

    pub(super) fn panorama_sun(
        &mut self,
        renderer: &Renderer,
        path: &str,
        threshold: f32,
        wake: &Wake,
        failures: &mut Vec<String>,
    ) -> Option<PanoramaSun> {
        let wanted = Wanted::Sun {
            path: Asset::new(path),
            threshold: threshold.to_bits(),
        };
        match self.fetch(renderer, wanted, wake, failures)? {
            Loaded::Sun(sun) => sun,
            Loaded::Texture(_) | Loaded::Profile(_) => None,
        }
    }

    pub(super) fn profile(
        &mut self,
        renderer: &mut Renderer,
        path: &str,
        wake: &Wake,
        failures: &mut Vec<String>,
    ) -> Option<LightProfileId> {
        let path = Asset::new(path);
        if let Some(known) = self.profiles.get(&path) {
            return *known;
        }
        let Loaded::Profile(profile) =
            self.fetch(renderer, Wanted::Profile(path.clone()), wake, failures)?
        else {
            return None;
        };
        let id = match renderer.three().create_light_profile(&profile) {
            Ok(id) => Some(id),
            Err(error) => {
                failures.push(format!("{path}: cannot upload: {error}"));
                None
            }
        };
        self.profiles.insert(path, id);
        id
    }

    fn fetch(
        &mut self,
        renderer: &Renderer,
        wanted: Wanted,
        wake: &Wake,
        failures: &mut Vec<String>,
    ) -> Option<Loaded> {
        let mut shelf = self.shelf.lock();
        match shelf.get_mut(&wanted) {
            Some(Loading::Ready(loaded)) => return Some(loaded.clone()),
            Some(Loading::Pending) => {
                wake.wait();
                return None;
            }
            Some(Loading::Reported) => return None,
            Some(failed @ Loading::Failed(_)) => {
                if let Loading::Failed(reason) = std::mem::replace(failed, Loading::Reported) {
                    failures.push(format!("{}: {reason}", wanted.path()));
                }
                return None;
            }
            None => {}
        }
        shelf.insert(wanted.clone(), Loading::Pending);
        wake.wait();
        drop(shelf);
        let uploads = self
            .uploads
            .get_or_insert_with(|| renderer.uploads())
            .clone();
        let shelf = Arc::clone(&self.shelf);
        let rouse = wake.rouse();
        let key = wanted.clone();
        let spawned = std::thread::Builder::new()
            .name("rikka-animate-asset".into())
            .spawn(move || {
                let loaded = match load(&key, &uploads) {
                    Ok(loaded) => Loading::Ready(loaded),
                    Err(reason) => Loading::Failed(reason),
                };
                shelf.lock().insert(key, loaded);
                rouse();
            });
        if let Err(error) = spawned {
            self.shelf
                .lock()
                .insert(wanted, Loading::Failed(error.to_string()));
        }
        None
    }
}

impl Drop for Assets {
    fn drop(&mut self) {
        let Some(uploads) = &self.uploads else {
            return;
        };
        for loading in self.shelf.lock().values() {
            if let Loading::Ready(Loaded::Texture(id)) = loading {
                uploads.destroy(*id);
            }
        }
    }
}

fn read(path: &Asset) -> Result<Vec<u8>, String> {
    std::fs::read(path.as_str()).map_err(|error| format!("cannot read: {error}"))
}

fn decode(path: &Asset) -> Result<Image, String> {
    Image::from_bytes(&read(path)?).map_err(|error| format!("cannot decode: {error}"))
}

fn uploaded(made: rikka_renderer::Result<TextureId>) -> Result<Loaded, String> {
    made.map(Loaded::Texture)
        .map_err(|error| format!("cannot upload: {error}"))
}

fn load(wanted: &Wanted, uploads: &Uploads) -> Result<Loaded, String> {
    match wanted {
        Wanted::Texture { path, srgb } => {
            let image = decode(path)?;
            let (width, height) = image.dimensions();
            let format = match srgb {
                true => TextureFormat::Rgba8Srgb,
                false => TextureFormat::Rgba8Premultiplied,
            };
            uploaded(uploads.create(Pixels::new(width, height, format, image.into_rgba8())))
        }
        Wanted::Lookup(path) => {
            let text =
                String::from_utf8(read(path)?).map_err(|error| format!("cannot read: {error}"))?;
            let lookup =
                ColorLookup::parse(&text).map_err(|error| format!("cannot parse: {error}"))?;
            uploaded(uploads.create_color_lookup(&lookup))
        }
        Wanted::Panorama(path) => {
            let image = decode(path)?;
            let (width, height) = image.dimensions();
            let pixels = image.into_rgba32f();
            uploaded(uploads.create(Pixels::rgba32f(width, height, &pixels)))
        }
        Wanted::Sun { path, threshold } => {
            let image = decode(path)?;
            let (width, height) = image.dimensions();
            let pixels = image.into_rgba32f();
            let panorama = Panorama::new(width, height, &pixels)
                .map_err(|error| format!("cannot read the panorama: {error}"))?;
            Ok(Loaded::Sun(panorama.sun(f32::from_bits(*threshold))))
        }
        Wanted::Profile(path) => {
            let profile = IesProfile::parse(&read(path)?)
                .map_err(|error| format!("cannot parse: {error}"))?;
            Ok(Loaded::Profile(Arc::new(profile)))
        }
    }
}
