use std::ops::Deref;

use crate::renderer::Renderer;
use rikka_gpu::texture::{Upload, Uploads};

#[derive(Debug)]
#[must_use = "uploads are applied when the handle drops"]
pub struct Textures<'a> {
    renderer: &'a mut Renderer,
}

impl Renderer {
    pub fn textures(&mut self) -> Textures<'_> {
        Textures { renderer: self }
    }

    #[must_use]
    pub fn uploads(&self) -> Uploads {
        self.uploads.clone()
    }

    pub(crate) fn take_uploads(&mut self) {
        let mut taken = std::mem::take(&mut self.taken_uploads);
        self.uploads.take_into(&mut taken);
        for upload in taken.drain(..) {
            self.take_upload(upload);
        }
        self.taken_uploads = taken;
        let retire_at = self.gpu.next_value();
        self.sources
            .textures
            .grow_minified(&mut self.gpu.bind_point(), retire_at);
        self.sources.stream(&self.gpu);
    }

    fn take_upload(&mut self, upload: Upload) {
        let sources = &mut self.sources;
        match upload {
            Upload::Insert { id, slot, created } => {
                sources
                    .textures
                    .insert_created(&mut self.gpu.bind_point(), id, slot, *created);
            }
            Upload::Update {
                id,
                offset,
                extent,
                pixels,
            } => sources.textures.update(id, offset, extent, pixels),
            Upload::Staged { id, rect, span } => sources.textures.update_staged(id, rect, span),
            Upload::Committed(id) => {
                sources.textures.mark_imported_dirty(id);
            }
            Upload::Destroy(id) => {
                sources.destroy_texture(&mut self.gpu.recycler(), id);
            }
            Upload::Video { texture, frame } => match sources.textures.takes_frames(texture) {
                true => sources.video.submit(texture, frame),
                false => drop(frame),
            },
            Upload::Computed { id, offset, copy } => {
                sources.textures.update_computed(id, offset, copy);
            }
        }
    }
}

impl AsRef<Uploads> for Renderer {
    fn as_ref(&self) -> &Uploads {
        &self.uploads
    }
}

impl Deref for Textures<'_> {
    type Target = Uploads;

    fn deref(&self) -> &Uploads {
        &self.renderer.uploads
    }
}

impl Drop for Textures<'_> {
    fn drop(&mut self) {
        self.renderer.take_uploads();
    }
}
