use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use cut_project::{Art, ClipId, Homography, Mirror};
use rikka_figure::{Affine, CanvasTransform, Form, LetterPose, Letterforms, Lettering, recipe};
use rikka_image_edit::{Imaging, Picture, Place, Recipe, Result, Step};
use rikka_types::{Point, Rect};

use crate::blanks::{Backdrop, Blanks};
use crate::geometry::LARGEST;
use crate::stack::Stack;

const MARGIN: f32 = 2.0;
const POSED_STEP: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Slot {
    Clip(ClipId),
    Drawing,
}

#[derive(Clone, Debug)]
pub(crate) struct Piece {
    pub(crate) picture: Picture,
    pub(crate) to_local: Affine,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Framing {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pad: u32,
    pub(crate) radius: f32,
    pub(crate) mirror: Mirror,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sketch {
    art: Art,
    poses: Vec<LetterPose>,
    scale: f32,
    pad: u32,
}

impl Sketch {
    pub(crate) fn new(art: &Art, poses: Vec<LetterPose>, scale: f32) -> Sketch {
        Sketch {
            art: art.clone(),
            poses,
            scale,
            pad: 0,
        }
    }
}

struct CachedSketch {
    sketch: Sketch,
    picture: Picture,
    anchor: Point,
    drawn_at: f32,
}

struct CachedText {
    lettering: Lettering,
    centred: Lettering,
    forms: Letterforms,
}

impl CachedText {
    fn new(art: &Art, lettering: &Lettering, scale: f32) -> CachedText {
        let centred = match art.form() {
            Form::Text(centred) => centred,
            _ => lettering.clone(),
        };
        CachedText {
            lettering: lettering.clone(),
            forms: Letterforms::new(&centred, scale),
            centred,
        }
    }

    fn extent(&self, poses: &[LetterPose]) -> Option<Rect> {
        Some(self.forms.bounds(poses)?.scale(1.0 / self.forms.scale()))
    }

    fn redrawn_at(&mut self, scale: f32) {
        if self.forms.scale() != scale {
            self.forms = Letterforms::new(&self.centred, scale);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EffectInput {
    source: Picture,
    stack: Stack,
    factor: f32,
    framing: Option<Framing>,
}

struct CachedEffect {
    input: EffectInput,
    picture: Picture,
}

#[derive(Default)]
pub(crate) struct Pieces {
    extents: HashMap<Slot, (Art, Rect)>,
    sketches: HashMap<Slot, CachedSketch>,
    texts: HashMap<Slot, CachedText>,
    effects: HashMap<Slot, CachedEffect>,
    used: HashSet<Slot>,
}

impl Pieces {
    pub(crate) fn reach(&mut self, slot: Slot, art: &Art) -> Rect {
        self.used.insert(slot);
        if let Some((cached_art, reach)) = self.extents.get(&slot)
            && cached_art == art
        {
            return *reach;
        }
        let reach = art.reach();
        self.extents.insert(slot, (art.clone(), reach));
        reach
    }

    pub(crate) fn graphic(
        &mut self,
        imaging: &Imaging,
        blanks: &mut Blanks,
        slot: Slot,
        sketch: Sketch,
        stack: &Stack,
    ) -> Result<Option<Piece>> {
        self.used.insert(slot);
        let sketch = Sketch {
            pad: stack.padding(sketch.scale),
            ..sketch
        };
        let fresh = self
            .sketches
            .get(&slot)
            .is_some_and(|sketches| sketches.sketch == sketch);
        if !fresh {
            let sketches = match sketch.poses.is_empty() {
                true => draw(imaging, blanks, &sketch)?,
                false => self.pose(imaging, blanks, slot, &sketch)?,
            };
            match sketches {
                Some((picture, anchor, drawn_at)) => self.sketches.insert(
                    slot,
                    CachedSketch {
                        sketch,
                        picture,
                        anchor,
                        drawn_at,
                    },
                ),
                None => {
                    self.sketches.remove(&slot);
                    return Ok(None);
                }
            };
        }
        let sketches = &self.sketches[&slot];
        let (source, anchor, drawn_at) =
            (sketches.picture.clone(), sketches.anchor, sketches.drawn_at);
        let picture = match stack.is_empty() {
            true => source,
            false => self.effects(
                slot,
                EffectInput {
                    source,
                    stack: stack.clone(),
                    factor: drawn_at,
                    framing: None,
                },
                |input| input.source.develop(&input.stack.scaled(input.factor)),
            )?,
        };
        Ok(Some(Piece {
            picture,
            to_local: Affine::translation(-f64::from(anchor.x), -f64::from(anchor.y)).then(
                Affine::scale(1.0 / f64::from(drawn_at), 1.0 / f64::from(drawn_at)),
            ),
        }))
    }

    pub(crate) fn footage(
        &mut self,
        blanks: &mut Blanks,
        slot: Slot,
        frame: &Picture,
        framing: Framing,
        stack: &Stack,
        factor: f32,
    ) -> Result<Picture> {
        self.used.insert(slot);
        let pad = framing.pad;
        let anchor = Point::new(
            framing.width as f32 / 2.0 + pad as f32,
            framing.height as f32 / 2.0 + pad as f32,
        );
        self.effects(
            slot,
            EffectInput {
                source: frame.clone(),
                stack: stack.clone(),
                factor,
                framing: Some(framing),
            },
            |input| {
                let imaging = input.source.imaging();
                let canvas = blanks.get(
                    imaging,
                    framing.width + pad * 2,
                    framing.height + pad * 2,
                    Backdrop::Clear,
                )?;
                let (width, height) = input.source.dimensions();
                let across = framing.width as f32 / width as f32;
                let down = framing.height as f32 / height as f32;
                let (sideways, upright) = framing.mirror.signs();
                let fitting =
                    Affine::translation(-f64::from(width) / 2.0, -f64::from(height) / 2.0)
                        .then(Affine::scale(
                            f64::from(across * sideways),
                            f64::from(down * upright),
                        ))
                        .then(Affine::translation(
                            f64::from(anchor.x),
                            f64::from(anchor.y),
                        ));
                let placing = Place::new(
                    input.source.clone(),
                    Homography::from_affine(fitting).matrix(),
                )
                .radius(framing.radius / across.min(down).max(f32::EPSILON));
                let mut recipe = Recipe::new().then(Step::new(placing));
                for step in input.stack.scaled(input.factor).steps() {
                    recipe.push(step.clone());
                }
                canvas.develop(&recipe)
            },
        )
    }

    pub(crate) fn sweep(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.extents.retain(|slot, _| used.contains(slot));
        self.sketches.retain(|slot, _| used.contains(slot));
        self.texts.retain(|slot, _| used.contains(slot));
        self.effects.retain(|slot, _| used.contains(slot));
    }

    pub(crate) fn letter_reach(
        &mut self,
        slot: Slot,
        art: &Art,
        poses: &[LetterPose],
        hint: f32,
    ) -> Option<Rect> {
        self.used.insert(slot);
        let Art::Text { lettering, .. } = art else {
            return Some(self.reach(slot, art));
        };
        self.texts(slot, art, lettering, hint).extent(poses)
    }

    fn texts(
        &mut self,
        slot: Slot,
        art: &Art,
        lettering: &Lettering,
        scale: f32,
    ) -> &mut CachedText {
        match self.texts.entry(slot) {
            Entry::Occupied(entry) if entry.get().lettering == *lettering => entry.into_mut(),
            Entry::Occupied(mut entry) => {
                entry.insert(CachedText::new(art, lettering, scale));
                entry.into_mut()
            }
            Entry::Vacant(free) => free.insert(CachedText::new(art, lettering, scale)),
        }
    }

    fn pose(
        &mut self,
        imaging: &Imaging,
        blanks: &mut Blanks,
        slot: Slot,
        sketch: &Sketch,
    ) -> Result<Option<(Picture, Point, f32)>> {
        let Art::Text { lettering, .. } = &sketch.art else {
            return draw(imaging, blanks, sketch);
        };
        let text = self.texts(slot, &sketch.art, lettering, sketch.scale);
        let Some(extent) = text.extent(&sketch.poses) else {
            return Ok(None);
        };
        let margin = MARGIN + sketch.pad as f32;
        let drawn_at = largest(sketch.scale, extent, margin);
        if drawn_at <= 0.0 {
            return Ok(None);
        }
        text.redrawn_at(drawn_at);
        let Some(bounds) = text.forms.bounds(&sketch.poses) else {
            return Ok(None);
        };
        let side = |length: f32| ((length + margin * 2.0) / POSED_STEP).ceil() * POSED_STEP;
        let width = side(bounds.width()).min(LARGEST) as u32;
        let height = side(bounds.height()).min(LARGEST) as u32;
        let anchor = Point::new(margin - bounds.x(), margin - bounds.y());
        let canvas = blanks.get(imaging, width, height, Backdrop::Clear)?;
        let picture = canvas.develop(&text.forms.recipe(&sketch.poses, anchor))?;
        Ok(Some((picture, anchor, drawn_at)))
    }

    fn effects(
        &mut self,
        slot: Slot,
        input: EffectInput,
        develop: impl FnOnce(&EffectInput) -> Result<Picture>,
    ) -> Result<Picture> {
        if let Some(cached) = self.effects.get(&slot)
            && cached.input == input
        {
            return Ok(cached.picture.clone());
        }
        let picture = develop(&input)?;
        self.effects.insert(
            slot,
            CachedEffect {
                input,
                picture: picture.clone(),
            },
        );
        Ok(picture)
    }
}

fn largest(scale: f32, reach: Rect, margin: f32) -> f32 {
    scale
        .min((LARGEST - margin * 2.0) / reach.width())
        .min((LARGEST - margin * 2.0) / reach.height())
}

fn draw(
    imaging: &Imaging,
    blanks: &mut Blanks,
    sketch: &Sketch,
) -> Result<Option<(Picture, Point, f32)>> {
    let form = sketch.art.form();
    let reach = form.bounds();
    if reach.width() <= 0.0 || reach.height() <= 0.0 || sketch.scale <= 0.0 {
        return Ok(None);
    }
    let margin = MARGIN + sketch.pad as f32;
    let drawn_at = largest(sketch.scale, reach, margin);
    let width = (reach.width() * drawn_at + margin * 2.0).ceil() as u32;
    let height = (reach.height() * drawn_at + margin * 2.0).ceil() as u32;
    let anchor = Point::new(
        -reach.x() * drawn_at + margin,
        -reach.y() * drawn_at + margin,
    );
    let steps = recipe([&form], CanvasTransform::new(drawn_at, anchor));
    let canvas = blanks.get(imaging, width, height, Backdrop::Clear)?;
    let picture = canvas.develop(&steps)?;
    Ok(Some((picture, anchor, drawn_at)))
}
