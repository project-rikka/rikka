use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::{Duration, Instant};

use rikka_compositor_desktop::{Drawn, SnapshotId};
use rikka_compositor_geometry::{Logical, Physical, Rect};
use rikka_compositor_output::CaptureSessionId;
use rikka_compositor_planes::Surface as Candidate;
use rikka_compositor_surface::{Dmabuf, Feedback, PlacedSurface, SurfaceId, Surfaces};
use rikka_compositor_types::OutputId;
use rikka_renderer::{Audience, Renderer, Scene};
use rikka_types::{Color, Rect as SceneRect, RoundedRect};

use super::DragIcon;
use super::composition::Composition;
use super::holds::Pin;
use super::pointer::CursorFor;
use super::presenting::Presenting;
use super::staging::Staging;
use super::textures::ClientTextures;
use super::timeline::TimelineWatch;
use super::view::View;
use crate::cursor::Cursor;
use crate::outputs::Output;
use crate::policy::Decor;

const BACKGROUND: Color = Color::rgba(0.06, 0.06, 0.08, 1.0);

const LOCKED_BACKGROUND: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);

const MAX_RECYCLED: usize = 4;

pub(crate) struct Scenes {
    by_output: BTreeMap<OutputId, Composition>,
    by_capture: HashMap<CaptureSessionId, Composition>,
    textures: ClientTextures,
    drawn_at: HashMap<SurfaceId, Instant>,
    recycled: Vec<Vec<Drawn>>,
    watch: TimelineWatch,
    decor: Decor,
}

pub(crate) struct Stage<'a> {
    pub(crate) cursor: &'a mut Cursor,
    pub(crate) surfaces: &'a mut Surfaces,
    pub(crate) drawn: Vec<Drawn>,
    pub(crate) staging: Staging,
    pub(crate) icon: Option<DragIcon>,
    pub(crate) corners: bool,
    pub(crate) locked: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Composed {
    pub(crate) drawn: Vec<Drawn>,
    pub(crate) staging: Staging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Framing {
    Screen { cursor: bool },
    Window { cursor: bool },
    Cursor,
}

impl Scenes {
    pub(crate) fn new(watch: TimelineWatch, decor: Decor) -> Self {
        Self {
            by_output: BTreeMap::new(),
            by_capture: HashMap::new(),
            textures: ClientTextures::default(),
            drawn_at: HashMap::new(),
            recycled: Vec::new(),
            watch,
            decor,
        }
    }

    pub(crate) fn set_decor(&mut self, decor: Decor) -> bool {
        std::mem::replace(&mut self.decor, decor) != decor
    }

    pub(crate) fn compose(
        &mut self,
        output: &Output,
        stage: Stage<'_>,
        renderer: &mut Renderer,
    ) -> &mut Scene {
        self.textures.reclaim(stage.surfaces, renderer);
        let decor = self.decor;
        let view = View::from_output(output);
        let composition = self
            .by_output
            .entry(output.id())
            .or_insert_with(Composition::new);
        composition.scene.set_background(backdrop(stage.locked));
        let composed = Composed {
            drawn: stage.drawn,
            staging: stage.staging,
        };
        composition.draw_trees(
            view,
            decor,
            stage.surfaces,
            &composed,
            &mut self.textures,
            renderer,
        );
        composition.draw_icon(
            view,
            stage.icon,
            stage.surfaces,
            &mut self.textures,
            renderer,
        );
        composition.draw_corners(output, stage.corners);
        composition.draw_cursor(
            view,
            stage.cursor,
            stage.surfaces,
            &mut self.textures,
            renderer,
            CursorFor::Screen {
                plane: output.has_cursor_plane(),
            },
        );
        let replaced = std::mem::replace(&mut composition.composed, composed);
        recycle(&mut self.recycled, replaced.drawn);
        &mut composition.scene
    }

    pub(crate) fn compose_capture(
        &mut self,
        session: CaptureSessionId,
        view: Option<View>,
        framing: Framing,
        stage: Stage<'_>,
        renderer: &mut Renderer,
    ) -> &mut Scene {
        self.textures.reclaim(stage.surfaces, renderer);
        let decor = self.decor;
        let composition = self
            .by_capture
            .entry(session)
            .or_insert_with(Composition::new);
        match framing {
            Framing::Screen { cursor } | Framing::Window { cursor } => {
                let icon = match framing {
                    Framing::Screen { .. } => stage.icon,
                    _ => None,
                };
                composition.scene.set_background(backdrop(stage.locked));
                let composed = Composed {
                    drawn: stage.drawn,
                    staging: stage.staging,
                };
                composition.draw_trees(
                    view,
                    decor,
                    stage.surfaces,
                    &composed,
                    &mut self.textures,
                    renderer,
                );
                composition.draw_icon(view, icon, stage.surfaces, &mut self.textures, renderer);
                match cursor {
                    true => composition.draw_cursor(
                        view,
                        stage.cursor,
                        stage.surfaces,
                        &mut self.textures,
                        renderer,
                        CursorFor::Capture,
                    ),
                    false => composition.hide_cursor(renderer),
                }
                recycle(&mut self.recycled, composed.drawn);
            }
            Framing::Cursor => {
                composition.scene.set_background(Color::TRANSPARENT);
                let image = stage.cursor.image(view.map_or(1.0, |view| view.scale));
                let at = SceneRect::new(0.0, 0.0, image.width as f32, image.height as f32);
                composition.place_pointer(image, at, true, Audience::Everyone, renderer);
            }
        }
        &mut composition.scene
    }

    pub(crate) fn spare_drawn(&mut self) -> Vec<Drawn> {
        self.recycled.pop().unwrap_or_default()
    }

    pub(crate) fn composed(&self, output: OutputId) -> Option<&Composed> {
        self.by_output
            .get(&output)
            .map(|composition| &composition.composed)
    }

    pub(crate) fn portrays_windows(&self, output: OutputId) -> bool {
        self.composed(output).is_some_and(|composed| {
            composed
                .staging
                .portrayed
                .values()
                .any(|portrayed| !portrayed.is_empty())
        })
    }

    pub(crate) fn scene(&mut self, output: OutputId) -> Option<&mut Scene> {
        self.by_output
            .get_mut(&output)
            .map(|composition| &mut composition.scene)
    }

    pub(crate) fn forget_capture(&mut self, session: CaptureSessionId, renderer: &mut Renderer) {
        if let Some(mut composition) = self.by_capture.remove(&session) {
            composition.hide_pointer(renderer);
            self.textures.unkeep(composition.staged.thaw_all());
        }
    }

    pub(crate) fn touch(
        &mut self,
        surface: SurfaceId,
        shown: bool,
        surfaces: &mut Surfaces,
        renderer: &mut Renderer,
    ) {
        match shown {
            true => self.textures.upload_early(surface, surfaces, renderer),
            false => self.textures.touch(surface),
        }
    }

    pub(crate) fn validate(&mut self, dmabuf: &Dmabuf, renderer: &mut Renderer) -> bool {
        self.textures.validate(dmabuf, renderer)
    }

    pub(crate) fn forget_surface(&mut self, surface: SurfaceId) {
        self.textures.forget(surface);
        self.drawn_at.remove(&surface);
        self.watch_kept_textures();
    }

    pub(crate) fn drawn(
        &mut self,
        output: OutputId,
        surfaces: &mut Surfaces,
        time: Duration,
        frame: u64,
    ) -> Vec<SurfaceId> {
        let Some(composition) = self.by_output.get_mut(&output) else {
            return Vec::new();
        };
        composition.landed();
        let now = Instant::now();
        let mut occluded = Vec::new();
        for tree in composition.drawn_trees() {
            if !tree.is_shown() {
                occluded.push(tree.root());
                continue;
            }
            surfaces.frame_done_for(tree.members(), time);
            self.drawn_at.insert(tree.root(), now);
        }
        self.textures.read(composition.surfaces(), frame);
        self.watch_kept_textures();
        occluded
    }

    pub(crate) fn capture_drawn(
        &mut self,
        session: CaptureSessionId,
        frame: u64,
        surfaces: &mut Surfaces,
        time: Duration,
    ) {
        if let Some(composition) = self.by_capture.get_mut(&session) {
            let now = Instant::now();
            for tree in composition.drawn_trees() {
                let root = tree.root();
                let seen = self.by_output.values().any(|shown| {
                    shown
                        .drawn_trees()
                        .any(|tree| tree.root() == root && tree.is_shown())
                });
                if !seen {
                    surfaces.frame_done_for(tree.members(), time);
                    self.drawn_at.insert(root, now);
                }
            }
            composition.landed();
            self.textures.read(composition.surfaces(), frame);
        }
        self.watch_kept_textures();
    }

    pub(crate) fn output_captured(&mut self, output: OutputId, frame: u64) {
        if let Some(composition) = self.by_output.get(&output) {
            self.textures.read(composition.surfaces(), frame);
        }
        self.watch_kept_textures();
    }

    pub(crate) fn watch_frame(&mut self, frame: u64) {
        self.watch.ask(frame);
    }

    pub(crate) fn gpu_reached(&mut self, finished: u64, surfaces: &mut Surfaces) {
        self.textures.release(finished, surfaces);
        self.watch_kept_textures();
    }

    pub(crate) fn pin(&mut self, surface: SurfaceId) -> Option<Pin> {
        self.textures.pin(surface)
    }

    pub(crate) fn unpin(&mut self, pin: Pin, surfaces: &mut Surfaces) {
        self.textures.unpin(pin, surfaces);
        self.watch_kept_textures();
    }

    pub(crate) fn generation(&self, surface: SurfaceId) -> Option<u64> {
        self.textures
            .look(surface)
            .map(|(_, generation)| generation)
    }

    fn watch_kept_textures(&mut self) {
        if let Some(frame) = self.textures.awaited() {
            self.watch.ask(frame);
        }
    }

    pub(crate) fn feedback(
        &self,
        output: OutputId,
        surfaces: &mut Surfaces,
        scanned: Option<SurfaceId>,
    ) -> Presenting {
        let mut composited = Feedback::default();
        let mut zero_copy = Feedback::default();
        if let Some(composition) = self.by_output.get(&output) {
            for tree in composition.drawn_trees() {
                let members = tree.members();
                match scanned
                    .filter(|scanned| members.iter().any(|placed| placed.surface == *scanned))
                {
                    None => composited.absorb(surfaces.take_feedback_for(members)),
                    Some(scanned) => {
                        let (on_plane, rest): (Vec<PlacedSurface>, Vec<PlacedSurface>) = members
                            .iter()
                            .copied()
                            .partition(|placed| placed.surface == scanned);
                        zero_copy.absorb(surfaces.take_feedback_for(&on_plane));
                        composited.absorb(surfaces.take_feedback_for(&rest));
                    }
                }
            }
        }
        Presenting::new(composited, zero_copy)
    }

    pub(crate) fn showing(&self, root: SurfaceId) -> impl Iterator<Item = OutputId> + '_ {
        self.by_output
            .iter()
            .filter(move |(_, composition)| composition.roots().any(|shown| shown == root))
            .map(|(output, _)| *output)
    }

    pub(crate) fn drawn_within(&self, root: SurfaceId, within: Duration) -> bool {
        self.drawn_at
            .get(&root)
            .is_some_and(|drawn| drawn.elapsed() < within)
    }

    pub(crate) fn covering(
        &self,
        output: OutputId,
        whole: Rect<i32, Physical>,
        surfaces: &Surfaces,
    ) -> Option<Candidate<SurfaceId>> {
        self.by_output
            .get(&output)?
            .covering(whole, surfaces, &self.textures)
    }

    pub(crate) fn damage_whole(&mut self, output: OutputId) {
        if let Some(composition) = self.by_output.get_mut(&output) {
            composition.scene.damage_all();
        }
    }

    pub(crate) fn forget(&mut self, output: OutputId, renderer: &mut Renderer) {
        if let Some(mut composition) = self.by_output.remove(&output) {
            composition.hide_pointer(renderer);
            self.textures.unkeep(composition.staged.thaw_all());
        }
    }

    pub(crate) fn freeze(
        &mut self,
        root: SurfaceId,
        snapshot: SnapshotId,
        geometry: Rect<i32, Logical>,
        frosted: &[RoundedRect],
    ) -> bool {
        let mut frozen = false;
        for composition in self
            .by_output
            .values_mut()
            .chain(self.by_capture.values_mut())
        {
            let Some(tree) = composition
                .trees
                .iter()
                .find(|tree| tree.root() == root && tree.view().is_none())
            else {
                continue;
            };
            if let Some(looks) = composition.staged.freeze(tree, snapshot, geometry, frosted) {
                frozen = true;
                self.textures.keep(looks);
            }
        }
        frozen
    }

    pub(crate) fn thaw(&mut self, snapshot: SnapshotId) {
        for composition in self
            .by_output
            .values_mut()
            .chain(self.by_capture.values_mut())
        {
            let looks = composition.staged.thaw(snapshot);
            self.textures.unkeep(looks);
        }
    }
}

fn backdrop(locked: bool) -> Color {
    match locked {
        true => LOCKED_BACKGROUND,
        false => BACKGROUND,
    }
}

fn recycle(recycled: &mut Vec<Vec<Drawn>>, mut drawn: Vec<Drawn>) {
    if recycled.len() < MAX_RECYCLED {
        drawn.clear();
        recycled.push(drawn);
    }
}

impl fmt::Debug for Scenes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Scenes")
            .field("outputs", &self.by_output.keys().collect::<Vec<_>>())
            .field("captures", &self.by_capture.len())
            .field("textures", &self.textures)
            .field("decor", &self.decor)
            .finish()
    }
}
