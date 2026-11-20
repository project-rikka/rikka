use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cut_project::{
    Art, Clip, ClipId, MediaId, Project, SceneItem, Shot, Stream, Transition, TransitionKind,
    TransitionShots, Visual,
};
use rikka_figure::Shape;
use rikka_image_edit::{Imaging, Picture};
use rikka_types::Rect;

use crate::blanks::{Backdrop, Blanks};
use crate::error::Result;
use crate::geometry::{centred, stepped_up};
use crate::layering::Layering;
use crate::pieces::{Pieces, Slot};
use crate::plan::{Entry, Layer, Plan, TransitionPlan};
use crate::region::filter;
use crate::request::Request;
use crate::sight::Sight;
use crate::stack::Stack;
use crate::supply::{FrameRequest, FrameState, Supply};
use crate::transit::{Backdrops, Mix, Timing};
use cut_project::Alignment;

const AHEAD: Duration = Duration::from_millis(500);
const MAX_AHEAD: Duration = Duration::from_secs(2);
const PREROLL: Duration = Duration::from_millis(1500);
const FIT_STEPS: f32 = 4.0;
const LOOKS_AHEAD: usize = 8;
const MIDDLE: f64 = 0.5;

#[derive(Clone, Debug)]
pub struct Composite {
    pub picture: Picture,
    pub exact: bool,
    pub offline: BTreeSet<MediaId>,
}

type Side = Option<(ClipId, Shot)>;

#[derive(Debug)]
struct TransitionEntry {
    transition: Transition,
    timing: Timing,
    sides: [Side; 2],
}

#[derive(Debug)]
enum SceneEntry {
    Clip {
        layer: usize,
        id: ClipId,
        shot: Box<Shot>,
    },
    TransitionSpan {
        layer: usize,
        transition: Box<TransitionEntry>,
    },
}

impl SceneEntry {
    fn new(showing: SceneItem<'_>, frame: i64) -> SceneEntry {
        match showing {
            SceneItem::Clip { layer, clip, shot } => SceneEntry::Clip {
                layer,
                id: clip.id(),
                shot,
            },
            SceneItem::TransitionSpan { layer, shots } => {
                let TransitionShots {
                    transition_span,
                    out,
                    into,
                } = *shots;
                SceneEntry::TransitionSpan {
                    layer,
                    transition: Box::new(TransitionEntry {
                        transition: *transition_span.transition,
                        timing: Timing {
                            step: frame - transition_span.span.start,
                            length: transition_span.length(),
                            seed: transition_span.seed(),
                        },
                        sides: [
                            transition_span.out.map(Clip::id).zip(out),
                            transition_span.into.map(Clip::id).zip(into),
                        ],
                    }),
                }
            }
        }
    }

    fn shots(&self) -> [Option<(usize, ClipId, &Shot)>; 2] {
        match self {
            SceneEntry::Clip { layer, id, shot } => [Some((*layer, *id, &**shot)), None],
            SceneEntry::TransitionSpan { layer, transition } => transition
                .sides
                .each_ref()
                .map(|side| side.as_ref().map(|(id, shot)| (*layer, *id, shot))),
        }
    }
}

#[derive(Debug)]
pub struct FrameInputs {
    entries: Vec<SceneEntry>,
    frames: HashMap<ClipId, FrameState>,
}

impl FrameInputs {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.frames.values().all(FrameState::is_final)
    }
}

pub struct Compositor {
    imaging: Imaging,
    supply: Supply,
    pieces: Pieces,
    blanks: Blanks,
    last: Option<(Plan, Picture)>,
}

impl std::fmt::Debug for Compositor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compositor")
            .field("imaging", &self.imaging)
            .finish_non_exhaustive()
    }
}

impl Compositor {
    #[must_use]
    pub fn new(imaging: Imaging) -> Compositor {
        Compositor {
            supply: Supply::new(imaging.clone()),
            imaging,
            pieces: Pieces::default(),
            blanks: Blanks::default(),
            last: None,
        }
    }

    pub fn on_ready(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.supply.on_ready(Arc::new(wake));
    }

    pub fn compose(
        &mut self,
        request: &Request,
        frame: i64,
        inputs: &FrameInputs,
    ) -> Result<Composite> {
        let plan = self.plan(request, frame, inputs);
        self.pieces.sweep();
        self.blanks.sweep();
        let plan = plan?;
        let picture = match &self.last {
            Some((last, picture)) if *last == plan => picture.clone(),
            _ => plan.render()?,
        };
        self.last = Some((plan, picture.clone()));
        Ok(Composite {
            picture,
            exact: inputs.is_ready(),
            offline: self.supply.offline(),
        })
    }

    pub fn idle(&self) {
        self.imaging.compute().trim();
    }

    pub fn collect_inputs(&mut self, request: &Request, frame: i64) -> FrameInputs {
        let now = Instant::now();
        let project = request.project();
        let view = request.seen_from();
        let forward = request.forward();
        self.supply.begin(request.round());
        let mut frames = HashMap::new();
        let mut aligned = Vec::new();
        let overrides = request.changes();
        let entries: Vec<SceneEntry> = project
            .scene_restyled(frame, view, |id| overrides.appearance(id))
            .into_iter()
            .map(|showing| SceneEntry::new(showing, frame))
            .collect();
        for (layer, id, shot) in entries.iter().flat_map(SceneEntry::shots).flatten() {
            let Some(clip) = project
                .layer(layer)
                .and_then(|layer_ref| layer_ref.find(id))
            else {
                continue;
            };
            let Some(frame_request) = footage_request(request, clip, shot, frame, forward) else {
                continue;
            };
            aligned.push(frame_request.alignment);
            frames.insert(id, self.supply.fetch(&frame_request, now));
        }
        if forward > 0.0 {
            let reach = project
                .rate()
                .frames_in(PREROLL.mul_f64(forward.min(8.0)))
                .max(1);
            let mut at = frame;
            for _ in 0..LOOKS_AHEAD {
                let Some(next) = project
                    .next_change(at, Stream::Picture)
                    .filter(|next| next - frame <= reach)
                else {
                    break;
                };
                let coming: Vec<SceneEntry> = project
                    .scene_restyled(next, view, |_| None)
                    .into_iter()
                    .map(|showing| SceneEntry::new(showing, next))
                    .collect();
                for (layer, id, shot) in coming.iter().flat_map(SceneEntry::shots).flatten() {
                    let Some(clip) = project
                        .layer(layer)
                        .and_then(|layer_ref| layer_ref.find(id))
                    else {
                        continue;
                    };
                    let Some(frame_request) = footage_request(request, clip, shot, next, forward)
                    else {
                        continue;
                    };
                    if aligned
                        .iter()
                        .any(|alignment| alignment.continues(&frame_request.alignment))
                    {
                        continue;
                    }
                    aligned.push(frame_request.alignment);
                    self.supply.prepare(&frame_request, now);
                }
                at = next;
            }
        }
        self.supply.end(now);
        FrameInputs { entries, frames }
    }

    fn plan(&mut self, request: &Request, frame: i64, inputs: &FrameInputs) -> Result<Plan> {
        let (width, height) = request.output();
        let canvas = self
            .blanks
            .get(&self.imaging, width, height, Backdrop::Black)?;
        let project = request.project();
        let overrides = request.changes();
        let mut layering = Layering {
            imaging: &self.imaging,
            pieces: &mut self.pieces,
            blanks: &mut self.blanks,
            request,
        };
        let mut entries = Vec::new();
        for entry in &inputs.entries {
            match entry {
                SceneEntry::Clip { layer, id, shot } => {
                    let drawn = layered(&mut layering, inputs, *layer, *id, shot, frame)?;
                    entries.extend(drawn.map(Entry::Layer));
                }
                SceneEntry::TransitionSpan { layer, transition } => {
                    let mut side = |slot: &Side| match slot {
                        Some((id, shot)) => {
                            layered(&mut layering, inputs, *layer, *id, shot, frame)
                        }
                        None => Ok(None),
                    };
                    let [out, into] = &transition.sides;
                    let (leaving, arriving) = (side(out)?, side(into)?);
                    let fills = [out, into]
                        .map(|slot| slot.as_ref().is_some_and(|(_, shot)| fills(project, shot)));
                    let deep = match transition.transition.kind {
                        TransitionKind::FilmDissolve => Some(layering.blanks.get(
                            &self.imaging,
                            width,
                            height,
                            Backdrop::Deep,
                        )?),
                        _ => None,
                    };
                    let backdrops = Backdrops {
                        clear: layering.blanks.get(
                            &self.imaging,
                            width,
                            height,
                            Backdrop::Clear,
                        )?,
                        deep,
                    };
                    let mix = Mix::new(
                        &transition.transition,
                        transition.timing,
                        request.factor(),
                        fills,
                        backdrops,
                    );
                    entries.push(Entry::TransitionSpan(Box::new(TransitionPlan {
                        out: leaving,
                        into: arriving,
                        mix,
                    })));
                }
            }
        }
        if let Some(drawing) = overrides.drawing() {
            let shot = project.front_shot(Visual::Graphic(drawing.clone()), request.seen_from());
            let drawn =
                layering.graphic(&shot, Slot::Drawing, drawing, &Stack::default(), false)?;
            entries.extend(drawn.map(Entry::Layer));
        }
        Ok(Plan { canvas, entries })
    }
}

fn layered(
    layering: &mut Layering<'_>,
    inputs: &FrameInputs,
    layer: usize,
    id: ClipId,
    shot: &Shot,
    frame: i64,
) -> Result<Option<Layer>> {
    let request = layering.request;
    let project = request.project();
    let Some(clip) = project
        .layer(layer)
        .and_then(|layer_ref| layer_ref.find(id))
    else {
        return Ok(None);
    };
    let stack = Stack::sample(clip, frame, request.changes());
    Ok(match &shot.visual {
        Visual::Footage { treatment, picture } => {
            match inputs.frames.get(&id).and_then(FrameState::picture) {
                Some(frame) => layering.footage(
                    shot,
                    id,
                    frame,
                    treatment,
                    *picture,
                    &stack.blurred(shot.transform.blur),
                )?,
                None => None,
            }
        }
        Visual::Graphic(graphic) => layering.graphic(
            shot,
            Slot::Clip(id),
            graphic,
            &stack.blurred(shot.transform.blur),
            moving(project, clip, shot),
        )?,
        Visual::Region(region) => filter(request, shot, region, &stack),
    })
}

fn fills(project: &Project, shot: &Shot) -> bool {
    let local = match &shot.visual {
        Visual::Footage { picture, .. } => centred(picture.width, picture.height),
        Visual::Region(region) => return region.is_whole(),
        Visual::Graphic(graphic) => match &graphic.art {
            Art::Shape {
                shape: Shape::Rectangle { .. },
                width,
                height,
                look,
            } if look.fill.is_some_and(|fill| fill.a >= 1.0) => centred(*width, *height),
            _ => return false,
        },
    };
    let local = local.inflate(1.0);
    let format = project.format();
    Rect::new(0.0, 0.0, format.width as f32, format.height as f32)
        .corners()
        .iter()
        .all(|corner| {
            shot.projection
                .unmap(*corner)
                .is_some_and(|at| local.contains(at))
        })
}

fn moving(project: &Project, clip: &Clip, shot: &Shot) -> bool {
    clip.is_moving()
        || !clip.animation_presets().is_empty()
        || shot.cameras.iter().any(|camera| {
            project
                .locate(*camera)
                .is_some_and(|(_, shot)| shot.is_moving())
        })
}

fn footage_request<'a>(
    request: &'a Request,
    clip: &Clip,
    shot: &Shot,
    frame: i64,
    forward: f64,
) -> Option<FrameRequest<'a>> {
    let Visual::Footage { picture, .. } = &shot.visual else {
        return None;
    };
    let project = request.project();
    let media_id = clip.media()?;
    let media = project.media(media_id)?;
    let native = media.picture()?;
    let ahead = match forward > 0.0 {
        true => AHEAD.mul_f64(forward * clip.speed()).min(MAX_AHEAD),
        false => Duration::ZERO,
    };
    let sight = Sight::new(
        &shot.projection,
        centred(picture.width, picture.height),
        project.format(),
        request.factor(),
    )?;
    let last = (project.media_frames(media_id) as f64 - MIDDLE).max(0.0);
    let source = clip.source_position(frame as f64 + MIDDLE).clamp(0.0, last);
    Some(FrameRequest {
        alignment: Alignment::for_clip(clip)?,
        path: media.path(),
        at: project.rate().time_at(source),
        ahead,
        fit: fit(native.width, native.height, picture.width * sight.stretch()),
    })
}

fn fit(width: u32, height: u32, target: f32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let needed = target / width as f32;
    if !(needed > 0.0 && needed < 1.0) {
        return None;
    }
    let step = stepped_up(needed, FIT_STEPS);
    if step >= 1.0 {
        return None;
    }
    Some((
        (width as f32 * step).ceil() as u32,
        (height as f32 * step).ceil() as u32,
    ))
}
