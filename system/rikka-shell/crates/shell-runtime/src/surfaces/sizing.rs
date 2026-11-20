use rikka_present::Presenter;
use rikka_renderer::{Error as RendererError, SurfaceConfig};
use rikka_types::Size;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::{ObjectId, Proxy};

use super::{PositionerState, Surfaces, taking};
use crate::error::Result;
use crate::screens::Screens;
use shell_kit::hosting::Measured;

const PREFERS_SCALE: u32 = 6;

impl Surfaces {
    pub(crate) fn configure(
        &mut self,
        id: &ObjectId,
        serial: u32,
        suggested: (u32, u32),
    ) -> Result<()> {
        let Some(hosted) = self.embedder.get_mut(id) else {
            return Ok(());
        };
        let positioner_state = hosted.state.positioner_state;
        if matches!(
            positioner_state,
            Some(PositionerState::Hidden | PositionerState::Unmapping)
        ) {
            return Ok(());
        }
        hosted.state.role.ack_configure(serial);
        let settings = &self.descs[hosted.state.desc].settings;
        let requested = hosted.state.wanted_size(settings.extent.requested());
        let logical = Size::new(
            configured_extent(suggested.0, requested.width),
            configured_extent(suggested.1, requested.height),
        );
        let regrown = hosted.state.fit.is_some() && hosted.state.logical != logical;
        hosted.state.logical = logical;
        hosted.state.configured = true;
        hosted.drawable(positioner_state != Some(PositionerState::Hiding));
        if let Some(PositionerState::Asked(geometry)) = positioner_state {
            hosted.state.positioner_state = Some(PositionerState::Shown(geometry));
            hosted.present_anew();
        } else if regrown {
            hosted.present_anew();
        }
        let pointer = self.descs[hosted.state.desc].pointer();
        let taken = taking::taking(pointer, logical, &hosted.state.panes);
        taking::take(&self.compositor, hosted.state.role.surface(), &taken);
        self.sync_size(id)
    }

    pub(crate) fn entered(
        &mut self,
        surface: &WlSurface,
        output: WlOutput,
        screens: &Screens,
    ) -> Result<()> {
        let id = surface.object_id();
        let Some(hosted) = self.embedder.get_mut(&id) else {
            return Ok(());
        };
        hosted.state.entered.push(output);
        self.rescale_unaware(surface, screens)
    }

    pub(crate) fn left(
        &mut self,
        surface: &WlSurface,
        output: &WlOutput,
        screens: &Screens,
    ) -> Result<()> {
        let id = surface.object_id();
        let Some(hosted) = self.embedder.get_mut(&id) else {
            return Ok(());
        };
        hosted.state.entered.retain(|entered| entered != output);
        self.rescale_unaware(surface, screens)
    }

    fn rescale_unaware(&mut self, surface: &WlSurface, screens: &Screens) -> Result<()> {
        if surface.version() >= PREFERS_SCALE {
            return Ok(());
        }
        self.rescale(&surface.object_id(), screens)
    }

    pub(crate) fn rescale_all(&mut self, screens: &Screens) -> Result<()> {
        let unaware: Vec<ObjectId> = self
            .embedder
            .iter()
            .filter(|(_, hosted)| hosted.state.role.surface().version() < PREFERS_SCALE)
            .map(|(id, _)| *id)
            .collect();
        for id in unaware {
            self.rescale(&id, screens)?;
        }
        Ok(())
    }

    fn rescale(&mut self, id: &ObjectId, screens: &Screens) -> Result<()> {
        let Some(hosted) = self.embedder.get(id) else {
            return Ok(());
        };
        let Some(factor) = hosted
            .state
            .entered
            .iter()
            .map(|output| screens.scale(output))
            .max()
        else {
            return Ok(());
        };
        self.set_scale(id, factor)
    }

    pub(crate) fn set_scale(&mut self, id: &ObjectId, factor: i32) -> Result<()> {
        let Some(hosted) = self.embedder.get_mut(id) else {
            return Ok(());
        };
        if hosted.state.scale == factor {
            return Ok(());
        }
        hosted.state.scale = factor;
        match hosted.state.configured {
            true => self.sync_size(id),
            false => Ok(()),
        }
    }

    fn sync_size(&mut self, id: &ObjectId) -> Result<()> {
        let Some(hosted) = self.embedder.get_mut(id) else {
            return Ok(());
        };
        let settings = &self.descs[hosted.state.desc].settings;
        let config = SurfaceConfig {
            transparent: settings.wants_alpha(),
            buffering: settings.buffering,
        };

        let scale = hosted.state.scale;
        let logical = hosted.state.logical;
        let width = (logical.width * scale as f32).round().max(1.0) as u32;
        let height = (logical.height * scale as f32).round().max(1.0) as u32;
        let viewport = match hosted.state.fit.is_none() {
            true => logical,
            false => hosted.state.room,
        };
        hosted.ui.set_viewport(viewport, scale as f32);

        let scale = f64::from(scale);
        if hosted.has_surface() {
            hosted.adjust(move |presenter| presenter.resize((width, height), scale, false));
            return Ok(());
        }

        let surface = hosted.state.role.surface().clone();
        let host = self.host.clone();
        let mut presenter = {
            let mut renderer = self.renderer()?;
            Presenter::new(&host, &surface, &mut renderer, config).map_err(RendererError::from)?
        };
        presenter.resize((width, height), scale, false);
        if let Some(hosted) = self.embedder.get_mut(id) {
            hosted.attach(presenter);
        }
        Ok(())
    }

    pub(crate) fn follow_geometry(&mut self, screens: &Screens) {
        self.follow_takings();
        self.follow_measurements();
        self.follow_positioners(screens);
    }

    fn follow_takings(&mut self) {
        for desc in self.descs.ids() {
            let pointer = self.descs[desc].pointer();
            let settings = &self.descs[desc].settings;
            let Some(reach) = settings
                .reach
                .as_ref()
                .filter(|reach| reach.take_moved())
                .map(|_| pointer)
            else {
                continue;
            };
            let mut reaching = Vec::new();
            for hosted in self.copies_mut(desc).filter(|hosted| {
                hosted
                    .state
                    .positioner_state
                    .is_none_or(PositionerState::offered)
            }) {
                let role = &hosted.state.role;
                reaching.push((
                    role.surface().clone(),
                    taking::taking(reach, hosted.state.logical, &hosted.state.panes),
                ));
                role.commit();
            }
            for (surface, taken) in reaching {
                taking::take(&self.compositor, &surface, &taken);
                surface.commit();
            }
        }
    }

    fn follow_measurements(&mut self) {
        for hosted in self
            .embedder
            .values_mut()
            .filter(|hosted| hosted.state.positioner_state.is_none())
        {
            let Some(size) = hosted
                .state
                .fit
                .as_ref()
                .filter(|fit| fit.take_moved())
                .map(Measured::get)
            else {
                continue;
            };
            let role = &hosted.state.role;
            let Some(layer) = role.layer() else {
                continue;
            };
            layer.set_size(
                size.width.round().max(0.0) as u32,
                size.height.round().max(0.0) as u32,
            );
            role.commit();
        }
    }
}

fn configured_extent(suggested: u32, requested: f32) -> f32 {
    if suggested > 0 {
        suggested as f32
    } else if requested >= 1.0 {
        requested
    } else {
        1.0
    }
}
