use std::sync::{Arc, MutexGuard};
use std::time::Instant;

use animate_body::{Body, Faces, MorphWeights};
use animate_cast::{FigureId, PropRole, Resolution, Skeleton, TakePlayback};
use animate_cloth::Brush;
use animate_fluid::EmitterId;
use animate_motion::Take;
use animate_sheet::Lane;
use animate_stage::{Aiming, Lens, Local, Nudge, Pending, SharedStage, Space, Stage, nudged};
use animate_timeline::Timeline;
use animate_transport::Transport;
use rikka_app::prelude::*;
use rikka_physics_tailor::PieceKey;
use rikka_shader_graph::NodeId;
use rikka_types::PixelSize;
use rikka_ui::DRAG_SLOP;
use rikka_viewport::Viewport3d;

use crate::Gesture;
use crate::cloth::{self, ClothOverlay, Stroke};
use crate::easy::{self, CancelOutcome, DragUpdate, PressOutcome, Scene};
use crate::emitter::EmitterGizmo;
use crate::fluid::{self, Gizmo};
use crate::handles::{self, Handle, Layout, Offer, Target};
use crate::overlay::{self, Lettering};
use crate::prop::{self, PropGizmo};
use crate::wind::{self, Grip};

#[derive(Clone, Debug)]
pub struct FigureAim {
    pub id: FigureId,
    pub skeleton: Arc<Skeleton>,
    pub body: Option<Arc<Body>>,
    pub faces: Arc<Faces>,
    pub take: Arc<Take>,
    pub playback: Option<Arc<TakePlayback>>,
    pub paths: bool,
}

impl PartialEq for FigureAim {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.paths == other.paths
            && Arc::ptr_eq(&self.skeleton, &other.skeleton)
            && Arc::ptr_eq(&self.take, &other.take)
            && Arc::ptr_eq(&self.faces, &other.faces)
            && match (&self.playback, &other.playback) {
                (Some(one), Some(two)) => Arc::ptr_eq(one, two),
                (one, two) => one.is_none() && two.is_none(),
            }
            && match (&self.body, &other.body) {
                (Some(one), Some(two)) => Arc::ptr_eq(one, two),
                (one, two) => one.is_none() && two.is_none(),
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClothAim {
    pub id: FigureId,
    pub brush: Brush,
    pub piece: Option<PieceKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marked {
    pub node: NodeId,
    pub movable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Aim {
    Idle,
    Figure(FigureAim),
    Cloth(ClothAim),
    Emitter(EmitterId),
    Camera,
    Wind,
    Prop(PropRole),
}

pub struct Screen {
    viewport: Viewport3d,
    stage: SharedStage,
    transport: Transport,
    timeline: Timeline,
    aim: Aim,
    marked: Option<Marked>,
    resolution: Resolution,
    lettering: Lettering,
    on_gesture: Handler<(Gesture,)>,
}

#[derive(Debug, Default)]
pub struct ScreenState {
    frame: Rect,
    grab: Option<Grab>,
    lit: Option<Handle>,
    space: Space,
    easy: easy::EasyDragState,
    gizmo: Gizmo,
    emitter: EmitterGizmo,
    prop: PropGizmo,
    grip: Grip,
    stroke: Option<Stroke>,
    pointer: Option<Point>,
    clicked: Option<Point>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    View(Point, Hold),
    Handle(Handle, Modifiers),
    Easy,
    Gizmo,
    Emitter,
    Prop,
    Vane,
    Paint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hold {
    Turn,
    Slide,
}

impl Screen {
    pub fn new<Marker>(
        viewport: &Viewport3d,
        stage: &SharedStage,
        transport: &Transport,
        timeline: &Timeline,
        aim: Aim,
        on_gesture: impl IntoHandler<(Gesture,), (), Marker>,
    ) -> Self {
        Self {
            viewport: viewport.clone(),
            stage: Arc::clone(stage),
            transport: transport.clone(),
            timeline: timeline.clone(),
            aim,
            marked: None,
            resolution: Resolution::default(),
            lettering: Lettering {
                global: tr!("animate.handles.global"),
                local: tr!("animate.handles.local"),
            },
            on_gesture: on_gesture.into_handler(),
        }
    }

    pub fn framed(mut self, resolution: Resolution) -> Self {
        self.resolution = resolution;
        self
    }

    pub fn marking(mut self, marked: Option<Marked>) -> Self {
        self.marked = marked;
        self
    }

    fn movable(&self) -> Option<NodeId> {
        self.marked
            .filter(|marked| marked.movable)
            .map(|marked| marked.node)
    }

    fn stage(&self) -> MutexGuard<'_, Stage> {
        self.stage.lock()
    }

    fn is_easy(&self) -> bool {
        matches!(&self.aim, Aim::Figure(figure_aim) if figure_aim.body.is_some())
    }

    pub(super) fn scene(&self, frame: Rect) -> Option<Scene> {
        self.scene_with(frame, false)
    }

    fn scene_with(&self, frame: Rect, feeling: bool) -> Option<Scene> {
        let Aim::Figure(figure_aim) = &self.aim else {
            return None;
        };
        let body = figure_aim.body.clone()?;
        if self.transport.is_playing() {
            return None;
        }
        let at = self.transport.frame();
        let mut stage = self.stage();
        let cloth_snapshot = match feeling {
            true => stage.cloth_snapshot(),
            false => None,
        };
        let graspable = stage.graspable();
        let projection = stage
            .projection()
            .filter(|projection| projection.figure == figure_aim.id)?;
        let human = projection.human.clone()?;
        let view_projection = projection.view_projection;
        let grips = projection.grips.clone();
        let pending = stage.pending_at(figure_aim.id, at);
        let morphs = pending.map_or_else(MorphWeights::default, |pending| pending.morphs.clone());
        let pose_sample = pending.and_then(Pending::posture).map_or_else(
            || figure_aim.take.sample(at as f32, body.rest_pose()),
            |posture| posture.pose_sample.clone(),
        );
        let part = match self.timeline.chosen_lane() {
            Some(Lane::Part(part)) => Some(part),
            _ => None,
        };
        Some(Scene {
            id: figure_aim.id,
            motion_bake: figure_aim
                .playback
                .as_ref()
                .and_then(|playback| playback.motion_bake().cloned()),
            take: Arc::clone(&figure_aim.take),
            lens: Lens::new(view_projection, frame),
            shown: human,
            frame: at,
            pose_sample,
            part,
            paths: figure_aim.paths,
            cloth_snapshot,
            graspable,
            grips,
            body,
            faces: Arc::clone(&figure_aim.faces),
            morphs,
        })
    }

    fn with_aiming<R>(&self, read: impl FnOnce(Option<&Aiming>) -> R) -> R {
        let stage = self.stage();
        let aiming = match self.aim {
            Aim::Emitter(id) => stage.aiming().filter(|aiming| aiming.emitter == id),
            _ => None,
        };
        read(aiming)
    }

    fn offer(&self) -> Offer {
        let still = !self.transport.is_playing();
        match &self.aim {
            Aim::Figure(figure_aim) if still && figure_aim.body.is_none() => {
                let bone = self
                    .timeline
                    .chosen_bone(&figure_aim.skeleton)
                    .and_then(|bone| figure_aim.skeleton.bone(bone));
                Offer {
                    target: bone.map(|_| Target::Bone),
                    turns: bone.is_some_and(|bone| bone.turns),
                    slides: bone.is_some_and(|bone| bone.moves),
                }
            }
            Aim::Camera if still => Offer {
                target: Some(Target::Camera),
                turns: true,
                slides: true,
            },
            Aim::Idle
            | Aim::Figure(_)
            | Aim::Cloth(_)
            | Aim::Emitter(_)
            | Aim::Camera
            | Aim::Wind
            | Aim::Prop(_) => Offer {
                target: None,
                turns: false,
                slides: false,
            },
        }
    }

    fn press(
        &self,
        state: &mut ScreenState,
        frame: Rect,
        position: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> Option<Grab> {
        let viewing = Some(Grab::View(
            position,
            hold(PointerButton::Primary, modifiers),
        ));
        if let Some(Marked {
            node,
            movable: true,
        }) = self.marked
            && !self.transport.is_playing()
            && self
                .stage()
                .emitter_marks()
                .is_some_and(|marks| state.emitter.press(marks, node, frame, position))
        {
            return Some(Grab::Emitter);
        }
        if let Aim::Prop(role) = self.aim
            && !self.transport.is_playing()
            && self
                .stage()
                .prop_frame()
                .filter(|placed| placed.role == role)
                .is_some_and(|placed| state.prop.press(placed, frame, position))
        {
            return Some(Grab::Prop);
        }
        if self.is_easy() {
            let Some(scene) = self.scene(frame) else {
                return viewing;
            };
            let restore = self
                .stage()
                .pending_at(scene.id, scene.frame)
                .and_then(Pending::posture)
                .cloned();
            return match state.easy.press(&scene, position, modifiers, restore) {
                PressOutcome::Grabbed => {
                    ctx.request_focus();
                    Some(Grab::Easy)
                }
                PressOutcome::ReleaseHold(hold) => {
                    ctx.call(&self.on_gesture, (Gesture::ReleaseHold(scene.id, hold),));
                    None
                }
                PressOutcome::Ignored => {
                    state.clicked = Some(position);
                    viewing
                }
            };
        }
        if let Aim::Cloth(clothed) = self.aim
            && !self.transport.is_playing()
            && !modifiers.shift
        {
            let stage = self.stage();
            state.stroke = Stroke::begin(
                &stage,
                clothed.id,
                &clothed.brush,
                frame,
                position,
                modifiers.control,
            );
            return match state.stroke {
                Some(_) => Some(Grab::Paint),
                None => viewing,
            };
        }
        if !self.transport.is_playing()
            && self.with_aiming(|aiming| {
                aiming.is_some_and(|aiming| state.gizmo.press(aiming, frame, position, modifiers))
            })
        {
            return Some(Grab::Gizmo);
        }
        if self.aim == Aim::Wind && !self.transport.is_playing() {
            let pressed = self
                .stage()
                .vane()
                .is_some_and(|vane| state.grip.press(vane, frame, position));
            if pressed {
                return Some(Grab::Vane);
            }
        }
        if let Some(handle) = Layout::new(frame).hit(position, self.offer()) {
            if handle == Handle::Space {
                state.space = match state.space {
                    Space::Global => Space::Local,
                    Space::Local => Space::Global,
                };
                return None;
            }
            return Some(Grab::Handle(handle, modifiers));
        }
        if let Aim::Figure(figure_aim) = &self.aim
            && !self.transport.is_playing()
        {
            let stage = self.stage();
            let picked = stage
                .projection()
                .filter(|projection| projection.figure == figure_aim.id)
                .and_then(|projection| {
                    overlay::bone_at(projection, &figure_aim.skeleton, frame, position)
                });
            drop(stage);
            if let Some(bone) = picked.and_then(|bone| figure_aim.skeleton.bone(bone)) {
                self.timeline.choose_lane(Lane::Bone(bone.name.clone()));
                return None;
            }
        }
        viewing
    }

    fn drag_handle(&self, handle: Handle, travel: Point, modifiers: Modifiers, space: Space) {
        let rise = -travel.y;
        match (handle, &self.aim) {
            (Handle::Turn(axis), Aim::Camera) => {
                let radians = handles::camera_turn(rise, modifiers);
                self.stage().reframe(|shot| shot.turn_about(axis, radians));
            }
            (Handle::Slide(axis), Aim::Camera) => {
                let units = handles::camera_slide(rise, modifiers);
                self.stage().reframe(|shot| shot.slide_along(axis, units));
            }
            (Handle::Turn(axis), Aim::Figure(figure_aim)) => {
                let nudge = Nudge::Turn(axis, handles::bone_turn(rise, modifiers));
                self.nudge(figure_aim, nudge, space);
            }
            (Handle::Slide(axis), Aim::Figure(figure_aim)) => {
                let nudge = Nudge::Slide(axis, handles::bone_slide(axis, rise, modifiers));
                self.nudge(figure_aim, nudge, space);
            }
            (
                Handle::Turn(_) | Handle::Slide(_),
                Aim::Idle | Aim::Cloth(_) | Aim::Emitter(_) | Aim::Wind | Aim::Prop(_),
            )
            | (Handle::Space, _) => {}
        }
    }

    fn nudge(&self, figure_aim: &FigureAim, nudge: Nudge, space: Space) {
        let Some(index) = self.timeline.chosen_bone(&figure_aim.skeleton) else {
            return;
        };
        let Some(bone) = figure_aim.skeleton.bone(index) else {
            return;
        };
        let frame = self.transport.frame();
        let mut stage = self.stage();
        let base = stage
            .pending_at(figure_aim.id, frame)
            .and_then(|pending| pending.bones()?.get(&index).copied())
            .unwrap_or_else(|| Local::playback(&figure_aim.take, &bone.name, frame));
        let parent = stage
            .projection()
            .filter(|projection| projection.figure == figure_aim.id)
            .map(|projection| projection.turn_of(bone.parent))
            .unwrap_or_default();
        let moved = nudged(base, nudge, space, parent, bone);
        stage.hold(figure_aim.id, frame, index, moved);
    }

    fn light(
        &self,
        state: &mut ScreenState,
        frame: Rect,
        position: Option<Point>,
        ctx: &mut EventCtx<'_>,
    ) {
        if state.emitter.hover(
            self.stage().emitter_marks(),
            self.movable(),
            frame,
            position,
        ) {
            ctx.request_paint();
        }
        if let Aim::Prop(role) = self.aim {
            let stage = self.stage();
            let placed = stage.prop_frame().filter(|placed| placed.role == role);
            if state.prop.hover(placed, frame, position) {
                ctx.request_paint();
            }
        }
        if matches!(self.aim, Aim::Cloth(_)) {
            if state.pointer != position {
                state.pointer = position;
                ctx.request_paint();
            }
            return;
        }
        if self.is_easy() {
            if let Some(scene) = self.scene(frame)
                && state.easy.hover(&scene, position)
            {
                ctx.request_paint();
            }
            return;
        }
        if matches!(self.aim, Aim::Emitter(_))
            && self.with_aiming(|aiming| state.gizmo.hover(aiming, frame, position))
        {
            ctx.request_paint();
        }
        if self.aim == Aim::Wind && state.grip.hover(self.stage().vane(), frame, position) {
            ctx.request_paint();
        }
        let highlighted =
            position.and_then(|position| Layout::new(frame).hit(position, self.offer()));
        if state.lit != highlighted {
            state.lit = highlighted;
            ctx.request_paint();
        }
    }

    fn travel(
        &self,
        state: &mut ScreenState,
        frame: Rect,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let Some(scene) = self.scene_with(frame, state.easy.feels()) else {
            return;
        };
        match state.easy.travel(&scene, position) {
            Some(DragUpdate::Pose(posture)) => {
                self.stage().preview(scene.id, scene.frame, *posture);
                ctx.request_paint();
            }
            Some(DragUpdate::Express(expression, weight)) => {
                ctx.call(&self.on_gesture, (Gesture::Express(expression, weight),));
                ctx.request_paint();
            }
            None if state.easy.feels() => ctx.request_paint(),
            None => {}
        }
    }

    fn shift(&self, state: &mut ScreenState, pressed: bool, ctx: &mut EventCtx<'_>) {
        let Some(scene) = self.scene_with(state.frame, state.easy.feels()) else {
            return;
        };
        if let Some(posture) = state.easy.shift(&scene, pressed) {
            self.stage().preview(scene.id, scene.frame, posture);
            ctx.request_paint();
        }
    }

    fn release_easy_drag(&self, state: &mut ScreenState, frame: Rect, ctx: &mut EventCtx<'_>) {
        let Some(scene) = self.scene_with(frame, state.easy.feels()) else {
            state.easy.drop_grab();
            self.stage().release();
            return;
        };
        let released = state.easy.release(&scene);
        if let Some(part) = released.part() {
            self.timeline.choose_lane(Lane::Part(part));
        }
        if let Some(gesture) = released.gesture {
            ctx.call(&self.on_gesture, (gesture,));
        }
        if let Some(marker) = released.marker {
            ctx.call(&self.on_gesture, (Gesture::Focus(Some(marker)),));
        }
    }

    fn place(&self, state: &mut ScreenState, frame: Rect, position: Point, ctx: &mut EventCtx<'_>) {
        if let Some(marks) = self.stage().emitter_marks() {
            state.emitter.travel(marks, frame, position);
        }
        ctx.request_paint();
    }

    fn pour(&self, state: &mut ScreenState, frame: Rect, position: Point, ctx: &mut EventCtx<'_>) {
        let at = self.transport.frame();
        state.gizmo.travel(&mut self.stage(), at, frame, position);
        ctx.request_paint();
    }

    fn paint_cloth(
        &self,
        state: &mut ScreenState,
        frame: Rect,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let (Aim::Cloth(clothed), Some(stroke)) = (&self.aim, state.stroke.as_mut()) else {
            return;
        };
        state.pointer = Some(position);
        stroke.reach(&self.stage(), &clothed.brush, frame, position);
        ctx.request_paint();
    }

    fn end_stroke(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        if let Some((figure, cloth_settings)) = state.stroke.take().and_then(Stroke::finish) {
            ctx.call(&self.on_gesture, (Gesture::Tailor(figure, cloth_settings),));
        }
    }

    fn blow(&self, state: &mut ScreenState, frame: Rect, position: Point, ctx: &mut EventCtx<'_>) {
        let at = self.transport.frame();
        state.grip.travel(&mut self.stage(), at, frame, position);
        ctx.request_paint();
    }

    fn release_gizmo(
        &self,
        state: &mut ScreenState,
        frame: Rect,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let released = self.with_aiming(|aiming| state.gizmo.release(aiming, frame, position));
        if let Some(gesture) = released {
            ctx.call(&self.on_gesture, (gesture,));
        }
    }

    fn cancel(&self, state: &mut ScreenState, frame: Rect, ctx: &mut EventCtx<'_>) {
        state.grab = None;
        ctx.release_pointer();
        ctx.keep_focus();
        ctx.request_paint();
        if state.stroke.take().is_some()
            || state.emitter.cancel()
            || state.prop.cancel(&mut self.stage())
        {
            return;
        }
        if let Aim::Emitter(id) = self.aim
            && state.gizmo.cancel(&mut self.stage(), id)
        {
            return;
        }
        if state.grip.cancel(&mut self.stage()) {
            return;
        }
        let Some(cancelled) = state.easy.cancel() else {
            return;
        };
        let Some(scene) = self.scene(frame) else {
            return;
        };
        match cancelled {
            CancelOutcome::Restore(posture) => {
                self.stage().preview(scene.id, scene.frame, *posture)
            }
            CancelOutcome::Release => self.stage().discard_pending_pose(scene.id, scene.frame),
            CancelOutcome::Express(expression, weight) => {
                ctx.call(&self.on_gesture, (Gesture::Express(expression, weight),));
            }
        }
    }
}

impl Widget for Screen {
    type State = ScreenState;

    fn paint(&self, state: &mut ScreenState, ctx: &mut PaintCtx<'_>) {
        let scale = ctx.scale();
        let frame = self.resolution.frame_in(ctx.bounds(), scale);
        state.frame = frame;
        if frame.is_empty() {
            return;
        }
        self.transport.time();
        let playing = self.transport.is_playing();

        let backdrop = if ctx.theme().is_dark() {
            Color::BLACK
        } else {
            Color::WHITE
        };
        ctx.draw_quad(Quad::new(frame).fill(backdrop));
        let width = (frame.width() * scale).round() as u32;
        let height = (frame.height() * scale).round() as u32;
        let stage = self.stage();
        if let Some(due) = stage.next_draw(Instant::now(), PixelSize::new(width, height)) {
            ctx.wake_at(due);
        }
        stage.watchers().watch(ctx.handle());
        if let Some(texture) = self.viewport.drawn_at(frame, width, height) {
            ctx.draw_image(frame, (texture, Size::new(width as f32, height as f32)));
        }
        if let Some(marked) = self.marked
            && let Some(marks) = stage.emitter_marks()
        {
            state.emitter.paint(ctx, marks, marked, frame);
        }
        if let Aim::Prop(role) = self.aim
            && !playing
            && let Some(placed) = stage.prop_frame().filter(|placed| placed.role == role)
        {
            prop::paint(ctx, placed, frame, &state.prop);
        }
        if let Aim::Figure(figure_aim) = &self.aim
            && figure_aim.body.is_none()
            && !playing
            && let Some(projection) = stage
                .projection()
                .filter(|projection| projection.figure == figure_aim.id)
        {
            let chosen = self.timeline.chosen_bone(&figure_aim.skeleton);
            overlay::paint_bones(ctx, projection, &figure_aim.skeleton, frame, chosen);
        }
        if let Aim::Cloth(clothed) = self.aim
            && let Some(paint_surface) = stage
                .paint_surface()
                .filter(|paint_surface| paint_surface.figure == clothed.id)
            && let Some(figure_cloth) = stage.figure_cloth_for(clothed.id)
        {
            let cloth_settings = match &state.stroke {
                Some(stroke) if stroke.figure() == clothed.id => stroke.working(),
                _ => figure_cloth.cloth_settings.as_ref(),
            };
            let shown = ClothOverlay {
                paint_surface,
                cloth_settings,
                analysis: figure_cloth.analysis.as_deref(),
                piece: clothed.piece,
                tool: clothed.brush.tool,
                brush: state
                    .pointer
                    .filter(|_| !playing)
                    .map(|at| (at, clothed.brush.radius)),
            };
            cloth::paint(ctx, &shown, frame);
            return;
        }
        drop(stage);
        if self.is_easy() {
            if let Some(scene) = self.scene_with(frame, state.easy.feels()) {
                easy::paint(ctx, &scene, &state.easy);
            }
            return;
        }
        self.with_aiming(|aiming| {
            if let Some(aiming) = aiming {
                fluid::paint(ctx, aiming, frame, &state.gizmo);
            }
        });
        if self.aim == Aim::Wind
            && let Some(vane) = self.stage().vane()
        {
            wind::paint(ctx, vane, frame, &state.grip);
        }
        let highlighted = match state.grab {
            Some(Grab::Handle(handle, _)) => Some(handle),
            _ => state.lit,
        };
        overlay::paint_handles(
            ctx,
            &Layout::new(frame),
            self.offer(),
            state.space,
            highlighted,
            &self.lettering,
        );
    }

    fn event(&self, state: &mut ScreenState, event: &Event, ctx: &mut EventCtx<'_>) {
        let frame = state.frame;
        match *event {
            Event::PointerPressed {
                position,
                button,
                modifiers,
            } => {
                if matches!(
                    state.grab,
                    Some(
                        Grab::Easy
                            | Grab::Gizmo
                            | Grab::Emitter
                            | Grab::Prop
                            | Grab::Vane
                            | Grab::Paint
                    )
                ) {
                    self.cancel(state, frame, ctx);
                    return;
                }
                state.clicked = None;
                let grab = match button {
                    PointerButton::Primary => self.press(state, frame, position, modifiers, ctx),
                    PointerButton::Secondary => Some(Grab::View(position, Hold::Turn)),
                    PointerButton::Middle => Some(Grab::View(position, Hold::Slide)),
                    _ => None,
                };
                ctx.request_paint();
                let Some(grab) = grab else {
                    ctx.keep_focus();
                    return;
                };
                match grab {
                    Grab::Handle(..) => ctx.lock_pointer(),
                    Grab::View(..)
                    | Grab::Easy
                    | Grab::Gizmo
                    | Grab::Emitter
                    | Grab::Prop
                    | Grab::Vane
                    | Grab::Paint => {
                        ctx.capture_pointer();
                    }
                }
                state.grab = Some(grab);
            }
            Event::PointerMoved(position) => match state.grab {
                Some(Grab::View(before, hold)) => {
                    if state
                        .clicked
                        .is_some_and(|pressed| pressed.distance(position) >= DRAG_SLOP)
                    {
                        state.clicked = None;
                    }
                    state.grab = Some(Grab::View(position, hold));
                    let (x, y) = (position.x - before.x, position.y - before.y);
                    let mut stage = self.stage();
                    match hold {
                        Hold::Turn => stage.turn(x, y),
                        Hold::Slide => stage.pan(x, y, frame.height()),
                    }
                    drop(stage);
                    ctx.request_paint();
                }
                Some(Grab::Easy) => self.travel(state, frame, position, ctx),
                Some(Grab::Gizmo) => self.pour(state, frame, position, ctx),
                Some(Grab::Emitter) => self.place(state, frame, position, ctx),
                Some(Grab::Prop) => {
                    let at = self.transport.frame();
                    state.prop.travel(&mut self.stage(), at, position);
                    ctx.request_paint();
                }
                Some(Grab::Vane) => self.blow(state, frame, position, ctx),
                Some(Grab::Paint) => self.paint_cloth(state, frame, position, ctx),
                Some(Grab::Handle(..)) => {}
                None => self.light(state, frame, Some(position), ctx),
            },
            Event::PointerReleased { position, .. } => {
                if state.clicked.take().is_some() {
                    ctx.call(&self.on_gesture, (Gesture::Focus(None),));
                }
                match state.grab {
                    Some(Grab::Easy) => self.release_easy_drag(state, frame, ctx),
                    Some(Grab::Gizmo) => self.release_gizmo(state, frame, position, ctx),
                    Some(Grab::Emitter) => {
                        if let Some(gesture) = state.emitter.release() {
                            ctx.call(&self.on_gesture, (gesture,));
                        }
                    }
                    Some(Grab::Prop) => {
                        if let Some(gesture) = state.prop.release() {
                            ctx.call(&self.on_gesture, (gesture,));
                        }
                    }
                    Some(Grab::Vane) => {
                        if let Some(gesture) = state.grip.release() {
                            ctx.call(&self.on_gesture, (gesture,));
                        }
                    }
                    Some(Grab::Paint) => self.end_stroke(state, ctx),
                    _ => {}
                }
                if state.grab.take().is_some() {
                    ctx.release_pointer();
                    ctx.request_paint();
                }
                self.light(state, frame, Some(position), ctx);
            }
            Event::PointerTravelled(travel) => {
                if let Some(Grab::Handle(handle, modifiers)) = state.grab {
                    self.drag_handle(handle, travel, modifiers, state.space);
                    ctx.request_paint();
                }
            }
            Event::PointerExited => self.light(state, frame, None, ctx),
            Event::KeyPressed {
                key: Key::ShiftLeft | Key::ShiftRight,
                ..
            } => self.shift(state, true, ctx),
            Event::KeyReleased {
                key: Key::ShiftLeft | Key::ShiftRight,
                ..
            } => self.shift(state, false, ctx),
            Event::FocusLost => self.shift(state, false, ctx),
            Event::Scroll { delta, .. } => {
                let widened = state.grab == Some(Grab::Easy) && state.easy.widen(delta.up());
                if !widened {
                    self.stage().dolly(delta.zoom());
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.viewport.id() == other.viewport.id()
            && Arc::ptr_eq(&self.stage, &other.stage)
            && self.transport == other.transport
            && self.timeline == other.timeline
            && self.aim == other.aim
            && self.marked == other.marked
            && self.resolution == other.resolution
            && self.lettering.global == other.lettering.global
            && self.lettering.local == other.lettering.local
    }

    fn behavior(&self, _state: &ScreenState) -> Behavior {
        Behavior::TRACKS_PLACE
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE
            | Behavior::NO_TAB_STOP
    }

    fn tooltip_at(
        &self,
        state: &ScreenState,
        _bounds: Rect,
        point: Point,
    ) -> Option<(Tooltip, Rect)> {
        let layout = Layout::new(state.frame);
        let handle = layout.hit(point, self.offer())?;
        Some((Tooltip::new(handle.name()), layout.rect(handle)))
    }

    fn debug_name(&self) -> &'static str {
        "Screen"
    }
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Screen")
            .field("viewport", &self.viewport)
            .field("aim", &self.aim)
            .finish_non_exhaustive()
    }
}

fn hold(button: PointerButton, modifiers: Modifiers) -> Hold {
    match button {
        PointerButton::Primary if modifiers.shift => Hold::Slide,
        PointerButton::Middle => Hold::Slide,
        _ => Hold::Turn,
    }
}
