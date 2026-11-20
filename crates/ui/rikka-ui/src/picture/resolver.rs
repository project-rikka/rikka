use std::sync::Once;

use rikka_picture::Changes;
use rikka_renderer::TextureId;

use crate::picture::ImageSource;

pub trait TextureResolver {
    fn resolve(&mut self, source: &ImageSource) -> Option<TextureId>;

    fn changes(&mut self, source: &ImageSource, since: u64) -> Changes {
        let _ = (source, since);
        Changes::UNKNOWN
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DirectResolver;

impl TextureResolver for DirectResolver {
    fn resolve(&mut self, source: &ImageSource) -> Option<TextureId> {
        match source {
            ImageSource::Texture { id, .. } => Some(*id),
            _ => {
                warn_unresolved();
                None
            }
        }
    }
}

fn warn_unresolved() {
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        tracing::warn!("picture drawn without a host resolver");
    });
}
