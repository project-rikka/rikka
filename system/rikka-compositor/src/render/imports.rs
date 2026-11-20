use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use rikka_compositor_surface::Dmabuf;
use rikka_compositor_types::Format;
use rikka_gpu::Ycbcr;
use rikka_renderer::{Renderer, TextureId};

use super::specs::with_client_spec;

const LINGER: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DmabufKey {
    device: u64,
    inode: u64,
    planes: [(u32, u32); 4],
    format: Format,
    size: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ImportKey {
    dmabuf: DmabufKey,
    ycbcr: Ycbcr,
}

#[derive(Debug)]
struct Import {
    texture: TextureId,
    idle_since: Option<Instant>,
    orphaned: bool,
}

#[derive(Default)]
pub(crate) struct DmabufImports {
    imported: HashMap<ImportKey, Import>,
}

pub(crate) fn key_of(dmabuf: &Dmabuf) -> Option<DmabufKey> {
    let first = dmabuf.planes().first()?;
    let stat = rustix::fs::fstat(first.fd()).ok()?;
    let mut planes = [(0, 0); 4];
    for (slot, plane) in planes.iter_mut().zip(dmabuf.planes()) {
        *slot = (plane.offset(), plane.stride());
    }
    let size = dmabuf.size();
    Some(DmabufKey {
        device: stat.st_dev,
        inode: stat.st_ino,
        planes,
        format: dmabuf.format(),
        size: (size.w, size.h),
    })
}

impl DmabufImports {
    pub(crate) fn is_empty(&self) -> bool {
        self.imported.is_empty()
    }
    pub(crate) fn texture(
        &mut self,
        dmabuf: &Dmabuf,
        ycbcr: Ycbcr,
        renderer: &mut Renderer,
    ) -> Option<TextureId> {
        let key = ImportKey {
            dmabuf: key_of(dmabuf)?,
            ycbcr,
        };
        if let Some(import) = self.imported.get_mut(&key) {
            import.idle_since = None;
            import.orphaned = false;
            return Some(import.texture);
        }
        match with_client_spec(dmabuf, |spec| {
            renderer.textures().import_ycbcr_dmabuf(spec, ycbcr)
        }) {
            Ok(texture) => {
                self.imported.insert(
                    key,
                    Import {
                        texture,
                        idle_since: None,
                        orphaned: false,
                    },
                );
                Some(texture)
            }
            Err(error) => {
                tracing::debug!("cannot import {:?} dmabuf: {error}", dmabuf.format().fourcc);
                None
            }
        }
    }

    pub(crate) fn orphan(&mut self, gone: impl IntoIterator<Item = Dmabuf>) {
        for dmabuf in gone {
            let Some(key) = key_of(&dmabuf) else {
                continue;
            };
            for (_, import) in self
                .imported
                .iter_mut()
                .filter(|(imported, _)| imported.dmabuf == key)
            {
                import.orphaned = true;
            }
        }
    }

    pub(crate) fn expire(&mut self, in_use: impl Fn(TextureId) -> bool, renderer: &mut Renderer) {
        let now = Instant::now();
        self.imported.retain(|key, import| {
            if in_use(import.texture) {
                import.idle_since = None;
                return true;
            }
            let since = *import.idle_since.get_or_insert(now);
            if !import.orphaned && now.duration_since(since) < LINGER {
                return true;
            }
            tracing::trace!(
                width = key.dmabuf.size.0,
                height = key.dmabuf.size.1,
                orphaned = import.orphaned,
                "dmabuf import released"
            );
            renderer.textures().destroy(import.texture);
            false
        });
    }
}

impl fmt::Debug for DmabufImports {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DmabufImports")
            .field("imported", &self.imported.len())
            .finish()
    }
}
