use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_surface::{
    BufferChange, Changes, Committed, Dmabuf, SurfaceHost, SurfaceId, Surfaces,
};
use rikka_compositor_types::OutputId;

use crate::context::{capture_ctx, windows_ctx};
use crate::state::Compositor;
use crate::windows::hidden_frame_pace;

impl SurfaceHost for Compositor {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }

    fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        let renderer = self.backend.renderer();
        self.scenes.validate(dmabuf, renderer)
    }

    fn wants_refresh(&mut self, output: OutputId) {
        self.frames.request(output);
    }

    fn output_of(&mut self, output: &WlOutput) -> Option<OutputId> {
        self.outputs.wayland().output_of(output)
    }

    fn committing(&mut self, surface: SurfaceId) {
        if let Some(portals) = self.views.take_staged(surface) {
            self.surfaces.stage::<Compositor, _>(surface, portals);
        }
    }

    fn committed(&mut self, commit: &Committed) {
        rikka_compositor_seat::surface_committed(self, commit);
        let root = self.surfaces.root(commit.surface);
        windows_ctx!(self).root_committed(root);
        capture_ctx!(self).capture_committed(root);
        let cursor = self
            .cursor
            .surface()
            .is_some_and(|(surface, _)| self.surfaces.root(surface) == root);
        if cursor && let Some(output) = self.outputs.containing(self.cursor.position()) {
            self.frames.request(output.id());
        }
        let mut shown = false;
        for output in self.scenes.showing(root) {
            shown = true;
            self.frames.request(output);
        }
        if commit.buffer != BufferChange::Unchanged || commit.changes.contains(Changes::DAMAGE) {
            let renderer = self.backend.renderer();
            self.scenes
                .touch(commit.surface, shown, &mut self.surfaces, renderer);
        }
        if shown {
            return;
        }
        if self.presence.shown(root) || self.windows.drawable(&self.surfaces, root) {
            self.frames.request_all();
        }
        if commit.changes.contains(Changes::FRAME_CALLBACKS)
            && let Some(pace) = hidden_frame_pace(&self.windows, &self.surfaces, root)
        {
            self.throttle.wait(root, pace);
        }
    }

    fn destroyed(&mut self, surface: SurfaceId) {
        self.views.forget(surface);
        rikka_compositor_seat::surface_destroyed(self, surface);
        let shown = self.scenes.showing(surface).next().is_some();
        self.scenes.forget_surface(surface);
        if self.cursor.forget_surface(surface) || shown {
            self.frames.request_all();
        }
    }
}
