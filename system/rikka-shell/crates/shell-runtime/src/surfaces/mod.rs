mod descs;
mod exit_animation;
mod fading;
mod positioner_state;
mod sizing;
mod taking;
mod unmap;

use std::sync::{Arc, MutexGuard};

pub(crate) use descs::{Descs, SurfaceId};
pub(crate) use positioner_state::{PositionerState, Unmapping};
use rikka_blur::Glass;
use rikka_present::Presenter;
use rikka_renderer::Renderer;
use rikka_seat::Seats;
use rikka_types::Size;
use rikka_ui::{DragEnd, Event, Handler, RuntimeMessage, Ui, clear_drag_payload};
use rikka_ui_host::{Embedder, FrameOutcome, Hosted};
use rikka_wayland::Host;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_wayland_client::wp_alpha_modifier_v1::WpAlphaModifierV1;
use rikka_wayland_client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use rikka_wayland_client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use rikka_wayland_client::{ObjectId, Proxy, Route};
use rikka_window_view::WindowViews;
pub(crate) use taking::Area;
use unmap::UnmapCallbacks;
pub(crate) use unmap::UnmapSync;

use super::convert;
use super::layer::LayerSurface;
use super::locking::Locker;
use super::opacity::{AlphaMultiplier, Driving};
use super::role::Role;
use super::screens::Screens;
use super::surface_state::{SurfaceEffects, SurfaceState};
use crate::error::Result;
use shell_kit::Outputs;
use shell_kit::Placement;
use shell_kit::PointerMode;
use shell_kit::SharedTheme;
use shell_kit::hosting::ErasedSurface;
use shell_kit::hosting::Measured;
use shell_kit::hosting::{OutputTarget, SurfaceSettings};
use shell_kit::{Sealed, SessionLock};

const ROOM_TO_MEASURE: Size = Size::new(720.0, 720.0);

type Hosting = Hosted<SurfaceState, Presenter>;

type Hosts = Embedder<ObjectId, SurfaceState, Presenter>;

pub(super) struct Surfaces {
    name: String,
    shared_theme: SharedTheme,
    descs: Descs,
    outputs: Outputs,
    host: Host,
    route: Route,
    compositor: WlCompositor,
    layer_shell: ZwlrLayerShellV1,
    glass: Option<Glass>,
    alpha_modifier: Option<WpAlphaModifierV1>,
    window_views: Option<WindowViews>,
    embedder: Hosts,
    locker: Locker,
    unmap_callbacks: UnmapCallbacks,
}

pub(crate) struct Wayland {
    pub(crate) host: Host,
    pub(super) route: Route,
    pub(crate) compositor: WlCompositor,
    pub(super) layer_shell: ZwlrLayerShellV1,
    pub(crate) glass: Option<Glass>,
    pub(crate) alpha_modifier: Option<WpAlphaModifierV1>,
    pub(super) window_views: Option<WindowViews>,
    pub(super) locker: Locker,
}

impl Surfaces {
    pub(crate) fn new(
        name: String,
        shared_theme: SharedTheme,
        descs: Vec<ErasedSurface>,
        outputs: Outputs,
        wayland: Wayland,
        embedder: Hosts,
    ) -> Surfaces {
        let Wayland {
            host,
            route,
            compositor,
            layer_shell,
            glass,
            alpha_modifier,
            window_views,
            locker,
        } = wayland;
        Surfaces {
            name,
            shared_theme,
            descs: Descs::new(descs),
            outputs,
            host,
            route,
            compositor,
            layer_shell,
            glass,
            alpha_modifier,
            window_views,
            embedder,
            locker,
            unmap_callbacks: UnmapCallbacks::default(),
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.embedder.shutdown();
    }

    pub(super) fn is_done(&self) -> bool {
        self.embedder.is_empty() && self.descs.is_empty()
    }

    pub(super) fn taking_input(&mut self, id: &ObjectId) -> Option<&mut Hosting> {
        let hosted = self.embedder.get_mut(id)?;
        (!self.descs[hosted.state.desc].is_leaving()).then_some(hosted)
    }

    pub(super) fn hosted(&self, id: &ObjectId) -> Option<&Hosting> {
        self.embedder.get(id)
    }

    pub(super) fn hosted_mut(&mut self, id: &ObjectId) -> Option<&mut Hosting> {
        self.embedder.get_mut(id)
    }

    pub(crate) fn deliver(&mut self, id: &ObjectId, inbound: RuntimeMessage) {
        self.embedder.deliver(id, inbound);
    }

    pub(super) fn reconcile(&mut self, screens: &Screens) {
        let outputs = screens.outputs();
        let live: Vec<ObjectId> = outputs.iter().map(Proxy::object_id).collect();
        let moved = self.outputs.set(
            outputs
                .iter()
                .filter_map(|output| screens.placement(output))
                .collect(),
        );

        let orphaned: Vec<ObjectId> = self
            .embedder
            .iter()
            .filter(|(_, s)| s.state.output.as_ref().is_some_and(|id| !live.contains(id)))
            .map(|(id, _)| *id)
            .collect();
        for id in orphaned {
            self.despawn(&id);
        }

        for desc in self.descs.ids() {
            if !self.descs[desc].is_up() {
                if !self.release_lock(desc) {
                    continue;
                }
                self.despawn_all(desc);
                continue;
            }
            if !self.may_lock(desc) {
                continue;
            }
            match self.descs[desc].settings.output.clone() {
                target @ (OutputTarget::Auto
                | OutputTarget::Aimed(_)
                | OutputTarget::Destined(_)) => {
                    if !self.spawned_anywhere(desc) {
                        let output = target.aim().and_then(|name| screens.named(&name));
                        self.spawn(desc, output, screens);
                    }
                }
                OutputTarget::All => {
                    let missing: Vec<WlOutput> = outputs
                        .iter()
                        .filter(|output| !self.spawned_on(desc, &output.object_id()))
                        .cloned()
                        .collect();
                    for output in missing {
                        self.spawn(desc, Some(&output), screens);
                    }
                }
            }
        }
        if moved {
            self.notify_moved(screens);
        }
    }

    pub(super) fn despawn(&mut self, id: &ObjectId) {
        self.embedder.remove(id);
    }

    fn despawn_all(&mut self, desc: SurfaceId) {
        let theirs: Vec<ObjectId> = self
            .embedder
            .iter()
            .filter(|(_, hosted)| hosted.state.desc == desc)
            .map(|(id, _)| *id)
            .collect();
        for id in theirs {
            self.despawn(&id);
        }
    }

    fn spawned_on(&self, desc: SurfaceId, output: &ObjectId) -> bool {
        self.copies(desc)
            .any(|hosted| hosted.state.output.as_ref() == Some(output))
    }

    fn spawned_anywhere(&self, desc: SurfaceId) -> bool {
        self.copies(desc).next().is_some()
    }

    fn copies(&self, desc: SurfaceId) -> impl Iterator<Item = &Hosting> {
        self.embedder
            .iter()
            .map(|(_, hosted)| hosted)
            .filter(move |hosted| hosted.state.desc == desc)
    }

    fn copies_mut(&mut self, desc: SurfaceId) -> impl Iterator<Item = &mut Hosting> {
        self.embedder
            .values_mut()
            .filter(move |hosted| hosted.state.desc == desc)
    }

    fn spawn(&mut self, desc: SurfaceId, output: Option<&WlOutput>, screens: &Screens) {
        let Some(role) = self.put_up(desc, output) else {
            return;
        };
        let wl_surface = role.surface().clone();
        let theme = self.shared_theme.theme();
        let blurs = self.descs[desc].settings.blurs(&theme);
        let place = output
            .and_then(|output| screens.placement(output))
            .unwrap_or_default();
        let room = measuring_room(&place);
        let output_name = place.name.clone();
        let mut ui = (self.descs[desc].source)(theme, place);

        let settings = &self.descs[desc].settings;
        ui.set_background(Some(settings.clear_color()));
        ui.set_measured(settings.fits);
        if self.descs[desc].exit().is_some() {
            ui.arrive();
        }
        let fit = settings
            .fits
            .then(|| Measured::new(settings.extent.requested()));

        let hosted = self.embedder.insert(
            wl_surface.object_id(),
            ui,
            SurfaceState::new(
                desc,
                output.map(Proxy::object_id),
                role,
                fit,
                room,
                SurfaceEffects {
                    glass: self.glass.as_ref(),
                    blurs,
                    window_views: self.window_views.as_ref(),
                },
            ),
        );
        if self.descs[desc].settings.positioner.is_some() {
            hosted.state.positioner_state = Some(PositionerState::Hidden);
            hosted.drawable(false);
        }
        let settings = &self.descs[desc].settings;
        if let (Some(manager), Some(opacity)) = (&self.alpha_modifier, &settings.opacity) {
            hosted.state.alpha = Some(AlphaMultiplier::new(
                manager,
                &wl_surface,
                opacity,
                output_name.clone(),
            ));
        }
        hosted.state.driving = settings
            .drives
            .clone()
            .map(|opacity| Driving::new(opacity, output_name));
        hosted.seals_with(Arc::new(|state: &mut SurfaceState| {
            if let Some(embedded_views) = &mut state.embedded_views {
                embedded_views.commit();
            }
            if let Some(driving) = &mut state.driving {
                driving.seal();
            }
        }));
        hosted.presents_with(Arc::new(move |state: &mut SurfaceState| {
            state.glazing.apply();
            if let Some(embedded_views) = &mut state.embedded_views {
                embedded_views.publish();
            }
            if let Some(driving) = &mut state.driving {
                driving.show();
            }
            wl_surface.frame().set_key(wl_surface.object_id());
        }));
    }

    fn put_up(&mut self, desc: SurfaceId, output: Option<&WlOutput>) -> Option<Role> {
        let settings = &self.descs[desc].settings;
        if settings.lock.is_some() {
            let surface = self.compositor.create_surface();
            return self.locker.guard(surface, output?).map(Role::Lock);
        }
        let namespace = settings.namespace(&self.name);
        let layer = LayerSurface::new(
            &self.compositor,
            &self.layer_shell,
            output,
            convert::layer(settings.layer),
            namespace,
        );
        if settings.positioner.is_none() {
            apply_settings(layer.role(), settings);
            layer.commit();
        }
        Some(Role::Layer(layer))
    }

    fn may_lock(&mut self, desc: SurfaceId) -> bool {
        let Some(lock) = self.descs[desc].settings.lock.clone() else {
            return true;
        };
        if !self.locker.can_lock() {
            tracing::warn!("compositor lacks ext-session-lock, cannot lock the screen");
            lock.unlock();
            return false;
        }
        if lock.sealed() == Sealed::Open {
            lock.settle(Sealed::Locking);
        }
        true
    }

    fn release_lock(&mut self, desc: SurfaceId) -> bool {
        let Some(lock) = self.descs[desc].settings.lock.clone() else {
            return true;
        };
        self.locker.release();
        if !self.locker.is_free() {
            return false;
        }
        lock.settle(Sealed::Open);
        true
    }

    pub(super) fn handle_locked(&mut self) {
        let locked = self.locker.handle_locked();
        for lock in self.locks() {
            lock.settle(match locked {
                true => Sealed::Locked,
                false => Sealed::Open,
            });
        }
    }

    pub(super) fn handle_lock_finished(&mut self) {
        self.locker.handle_finished();
        for lock in self.locks() {
            if lock.is_wanted() {
                tracing::warn!("compositor refused or ended the session lock");
            }
            lock.unlock();
            lock.settle(Sealed::Open);
        }
    }

    fn locks(&self) -> Vec<SessionLock> {
        self.descs
            .iter()
            .filter_map(|(_, surface)| surface.settings.lock.clone())
            .collect()
    }

    pub(super) fn follow_skin(&mut self) {
        if !self.shared_theme.take_moved() {
            return;
        }
        let theme = self.shared_theme.theme();
        let glass = self.glass.as_ref();
        for hosted in self.embedder.values_mut() {
            let settings = &self.descs[hosted.state.desc].settings;
            hosted.state.set_blur(glass, settings.blurs(&theme));
            hosted.ui.set_theme(theme.clone());
        }
    }

    pub(crate) fn frame(&mut self) -> FrameOutcome {
        let (descs, compositor) = (&self.descs, &self.compositor);
        self.embedder.frame(|drawing| {
            fit_to_content(drawing.state, drawing.ui);
            drawing
                .state
                .glazing
                .set_regions(drawing.ui.glass(), drawing.ui.opaque());
            if let Some(driving) = &mut drawing.state.driving {
                driving.draw();
            }
            if let Some(embedded_views) = &mut drawing.state.embedded_views {
                embedded_views.set_windows(drawing.ui.windows());
            }
            if descs[drawing.state.desc].pointer() != PointerMode::Panes {
                return;
            }
            let panes = taking::panes(drawing.ui.glass());
            if panes == drawing.state.panes {
                return;
            }
            let taken = taking::Taking::Within(panes.clone());
            drawing.state.panes = panes;
            taking::take(compositor, drawing.state.role.surface(), &taken);
        })
    }

    pub(crate) fn service(&mut self, seats: &mut Seats) {
        for hosted in self.embedder.values_mut() {
            for request in hosted.ui.take_window_requests() {
                tracing::debug!("layer surface ignores window request {request:?}");
            }
            if hosted.ui.take_drag_start().is_some() {
                hosted.ui.handle_event(Event::DragEnded(DragEnd::Aborted));
                clear_drag_payload();
            }
            let scale = u32::try_from(hosted.state.scale).unwrap_or(1);
            seats.show_cursor(hosted.state.role.surface(), hosted.ui.cursor(), scale);
        }
    }

    fn notify_moved(&mut self, screens: &Screens) {
        let handlers: Vec<(SurfaceId, Handler<(Placement,)>)> = self
            .descs
            .iter()
            .filter_map(|(desc, surface)| Some((desc, surface.moved.clone()?)))
            .collect();
        for (desc, handler) in handlers {
            for hosted in self.copies_mut(desc) {
                let Some(placement) = hosted
                    .state
                    .output
                    .as_ref()
                    .and_then(|output| screens.placement_of(output))
                else {
                    continue;
                };
                hosted
                    .ui
                    .start(handler.clone().map(move |()| (placement.clone(),)));
            }
        }
    }

    fn run_on(&mut self, desc: SurfaceId, handler: &Handler) {
        for hosted in self.copies_mut(desc) {
            hosted.ui.start(handler.clone());
        }
    }

    fn renderer(&mut self) -> Result<MutexGuard<'_, Renderer>> {
        let name = self.name.clone();
        self.embedder.renderer_or_try_init(|| {
            let builder = rikka_gpu::Gpu::builder()
                .app_name(name)
                .video_decode()
                .compute_support();
            Ok(Renderer::new(&builder.build()?)?)
        })
    }
}

impl std::fmt::Debug for Surfaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Surfaces")
            .field("name", &self.name)
            .field("descs", &self.descs)
            .finish_non_exhaustive()
    }
}

fn fit_to_content(state: &SurfaceState, ui: &Ui) {
    let Some(fit) = &state.fit else {
        return;
    };
    let size = ui.content_size();
    if size.width < 1.0 || size.height < 1.0 {
        return;
    }
    fit.set(size);
}

fn apply_settings(role: &ZwlrLayerSurfaceV1, settings: &SurfaceSettings) {
    let size = settings.extent.requested();
    role.set_anchor(convert::anchor(settings.extent.anchored(settings.anchor)));
    role.set_size(
        size.width.round().max(0.0) as u32,
        size.height.round().max(0.0) as u32,
    );
    role.set_margin(
        settings.margin.top().round() as i32,
        settings.margin.right().round() as i32,
        settings.margin.bottom().round() as i32,
        settings.margin.left().round() as i32,
    );
    role.set_exclusive_zone(convert::exclusive_zone(settings.exclusive));
    role.set_keyboard_interactivity(convert::keyboard_mode(settings.keyboard));
}

#[must_use]
fn measuring_room(place: &Placement) -> Size {
    match place.size.width >= 1.0 && place.size.height >= 1.0 {
        true => place.size,
        false => ROOM_TO_MEASURE,
    }
}
