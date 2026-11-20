use rikka_image_edit::{
    Cap, FillRule, GaussianBlur, Group, Join, Marking, Marks, Mask, Path, PathBuilder, PathMark,
    Recipe, Step, StrokeStyle,
};
use rikka_types::{Color, Point, Rect};

use super::lettering::{Band, Lettering};
use super::pose::LetterPose;
use super::raster::with_glyphs;
use super::typeset::Typeset;
use super::vector::{Inking, Letterform, Turn, letterforms, stamp};
use crate::affine::Affine;
use crate::frame::{Frame, enclosing};
use crate::shape::Shape;

const SHARP: f32 = 0.25;
const BLUR_STEPS: f32 = 4.0;
const SPREAD: f32 = 3.0;

#[derive(Clone, Debug)]
pub struct Letterforms {
    letters: Vec<Option<Letterform>>,
    scale: f32,
    lettering: Lettering,
    band: Option<(Rect, Band)>,
}

struct PosedLetter<'a> {
    letter: &'a Letterform,
    pose: LetterPose,
    map: Affine,
}

impl PosedLetter<'_> {
    fn blur(&self, scale: f32) -> Option<i32> {
        let sigma = self.pose.blur * scale;
        (sigma >= SHARP).then(|| (sigma.log2() * BLUR_STEPS).round() as i32)
    }

    fn shown(&self, rect: Rect) -> Rect {
        enclosing(rect.corners().map(|corner| self.map.apply(corner)))
    }
}

struct Unit<'p, 'l> {
    letters: Vec<&'p PosedLetter<'l>>,
    blur: Option<i32>,
    cut: Option<Vec<Point>>,
}

impl Unit<'_, '_> {
    fn sigma(&self) -> f32 {
        self.blur
            .map_or(0.0, |blur| (blur as f32 / BLUR_STEPS).exp2())
    }
}

#[derive(Clone, Copy)]
struct Backing {
    rect: Rect,
    color: Color,
    radius: f32,
    opacity: f32,
}

impl Backing {
    fn marking(&self, offset: Point, shade: Option<Color>) -> Marking {
        let path = Shape::Rectangle {
            radius: self.radius,
        }
        .path(&Frame::from_rect(self.rect.translate(offset)));
        let color = faded(shade.unwrap_or(self.color), self.opacity);
        PathMark::new(path).filled(color, FillRule::NonZero).into()
    }
}

impl Letterforms {
    #[must_use]
    pub fn new(lettering: &Lettering, scale: f32) -> Letterforms {
        let typeset = Typeset::new(lettering);
        let band = lettering.band.map(|band| {
            let rect = typeset.layout().inflate(band.padding);
            (rect.scale(scale), band)
        });
        Letterforms {
            letters: letterforms(&typeset, &lettering.text, scale),
            scale,
            lettering: lettering.clone(),
            band,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.letters.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.letters.is_empty()
    }

    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    #[must_use]
    pub fn bounds(&self, poses: &[LetterPose]) -> Option<Rect> {
        let posed = self.posed(poses);
        let reach = self.lettering.outline_reach() * self.scale;
        let body = posed
            .iter()
            .map(|posed| {
                posed
                    .shown(posed.letter.ink)
                    .inflate(reach + SPREAD * posed.pose.blur.max(0.0) * self.scale)
            })
            .chain(self.backing(&posed, poses).map(|backing| backing.rect))
            .reduce(|total, next| total.union(&next))?;
        Some(match self.lettering.shadow {
            Some(shadow) => body.union(
                &body
                    .translate(shadow.offset * self.scale)
                    .inflate(shadow.blur * SPREAD * self.scale),
            ),
            None => body,
        })
    }

    #[must_use]
    pub fn recipe(&self, poses: &[LetterPose], offset: Point) -> Recipe {
        let posed = self.posed(poses);
        let units = self.units(&posed);
        let backing = self.backing(&posed, poses);
        let mut recipe = Recipe::new();
        if let Some(shadow) = self.lettering.shadow {
            let cast = offset + shadow.offset * self.scale;
            let shade = Color::rgba(shadow.color.r, shadow.color.g, shadow.color.b, 1.0);
            let spread = (shadow.blur * self.scale).max(0.0);
            let lists: Vec<Vec<Step>> = units
                .iter()
                .enumerate()
                .map(|(index, unit)| {
                    let band = backing
                        .filter(|_| index == 0)
                        .map(|backing| backing.marking(cast, Some(shade)));
                    self.layered(unit, cast, Some(shade), band, spread.hypot(unit.sigma()))
                })
                .filter(|list| !list.is_empty())
                .collect();
            let cast_recipe = match lists.len() {
                1 => recipe_of(lists.into_iter().flatten()),
                _ => recipe_of(
                    lists
                        .into_iter()
                        .map(|list| Step::new(Group::new(recipe_of(list)))),
                ),
            };
            if !cast_recipe.is_empty() {
                recipe.push(Step::new(Group::new(cast_recipe)).opacity(shadow.color.a));
            }
        }
        for (index, unit) in units.iter().enumerate() {
            let band = backing
                .filter(|_| index == 0)
                .map(|backing| backing.marking(offset, None));
            let steps = self.layered(unit, offset, None, band, unit.sigma());
            match unit.blur.is_some() && !steps.is_empty() {
                true => recipe.push(Step::new(Group::new(recipe_of(steps)))),
                false => steps.into_iter().for_each(|step| recipe.push(step)),
            }
        }
        recipe
    }

    fn posed(&self, poses: &[LetterPose]) -> Vec<PosedLetter<'_>> {
        self.letters
            .iter()
            .enumerate()
            .filter_map(|(index, letter)| {
                let letter = letter.as_ref()?;
                let pose = poses.get(index).copied().unwrap_or(LetterPose::REST);
                (!pose.is_hidden()).then(|| PosedLetter {
                    letter,
                    pose,
                    map: pose.map(letter.centre, self.scale),
                })
            })
            .collect()
    }

    fn units<'p, 'l>(&self, posed: &'p [PosedLetter<'l>]) -> Vec<Unit<'p, 'l>> {
        let mut units = vec![Unit {
            letters: Vec::new(),
            blur: None,
            cut: None,
        }];
        for posed in posed {
            let blur = posed.blur(self.scale);
            match self.cut(posed) {
                Some(cut) => units.push(Unit {
                    letters: vec![posed],
                    blur,
                    cut: Some(cut),
                }),
                None => match units
                    .iter_mut()
                    .find(|unit| unit.cut.is_none() && unit.blur == blur)
                {
                    Some(unit) => unit.letters.push(posed),
                    None => units.push(Unit {
                        letters: vec![posed],
                        blur,
                        cut: None,
                    }),
                },
            }
        }
        units
    }

    fn cut(&self, posed: &PosedLetter<'_>) -> Option<Vec<Point>> {
        let wipe = posed.pose.wipe.filter(|wipe| wipe.progress < 1.0)?;
        let ink = posed.letter.ink;
        let spread =
            (self.lettering.outline_reach() + SPREAD * posed.pose.blur.max(0.0)) * self.scale + 1.0;
        Some(
            wipe.clipped_polygon(ink, ink.inflate(spread))
                .into_iter()
                .map(|point| posed.map.apply(point))
                .collect(),
        )
    }

    fn backing(&self, posed: &[PosedLetter<'_>], poses: &[LetterPose]) -> Option<Backing> {
        let (rest, band) = self.band?;
        let backing = |rect: Rect, opacity: f32| Backing {
            rect,
            color: band.color,
            radius: band.radius * self.scale,
            opacity,
        };
        let whole = self
            .letters
            .iter()
            .flatten()
            .map(|letter| letter.frame)
            .reduce(|total, next| total.union(&next));
        let Some(whole) = whole.filter(|_| !poses.iter().all(LetterPose::is_rest)) else {
            return Some(backing(rest, 1.0));
        };
        let shown = posed
            .iter()
            .map(|posed| posed.shown(posed.letter.frame))
            .reduce(|total, next| total.union(&next))?;
        let opacity = posed
            .iter()
            .map(|posed| posed.pose.opacity.min(1.0))
            .fold(0.0, f32::max);
        let left = rest.min_x() + shown.min_x() - whole.min_x();
        let top = rest.min_y() + shown.min_y() - whole.min_y();
        let right = (rest.max_x() + shown.max_x() - whole.max_x()).max(left);
        let bottom = (rest.max_y() + shown.max_y() - whole.max_y()).max(top);
        Some(backing(
            Rect::new(left, top, right - left, bottom - top),
            opacity,
        ))
    }

    fn layered(
        &self,
        unit: &Unit<'_, '_>,
        offset: Point,
        shade: Option<Color>,
        band: Option<Marking>,
        sigma: f32,
    ) -> Vec<Step> {
        let mut marks: Vec<Marking> = band.into_iter().collect();
        marks.extend(self.inked(&self.inks(unit.letters.iter().copied(), offset), shade));
        let mut body: Vec<Step> = Vec::new();
        if !marks.is_empty() {
            body.push(Step::new(Marks::new(marks)));
        }
        if shade.is_none() {
            body.extend(self.stamps(&unit.letters, offset));
        }
        if body.is_empty() {
            return body;
        }
        let mut steps = match &unit.cut {
            Some(cut) => {
                let polygon = Path::polygon(cut.iter().map(|point| *point + offset));
                vec![
                    Step::new(Group::new(recipe_of(body)))
                        .masked(Mask::path(polygon, FillRule::NonZero)),
                ]
            }
            None => body,
        };
        if sigma > 0.0 {
            steps.push(Step::new(GaussianBlur::new(sigma)));
        }
        steps
    }

    fn stamps(&self, letters: &[&PosedLetter<'_>], offset: Point) -> Vec<Step> {
        with_glyphs(|raster| {
            let mut steps = Vec::new();
            for posed in letters {
                let pose = posed.pose;
                let unturned = LetterPose {
                    rotation: 0.0,
                    ..pose
                }
                .map(posed.letter.centre, self.scale);
                let inking = Inking {
                    scale: self.scale * pose.zoom,
                    color: self.lettering.color,
                    opacity: pose.opacity,
                    turn: Turn {
                        centre: posed.map.apply(posed.letter.centre) + offset,
                        degrees: pose.rotation,
                    },
                };
                for (glyph, pen) in &posed.letter.colored {
                    let at = unturned.apply(*pen) + offset;
                    if let Some(stamp) = stamp(raster, glyph, at, &inking) {
                        steps.push(Step::new(stamp));
                    }
                }
            }
            steps
        })
    }

    fn inks<'p, 'l: 'p>(
        &self,
        posed: impl Iterator<Item = &'p PosedLetter<'l>>,
        shift: Point,
    ) -> Vec<(Path, f32)> {
        let lift = Affine::translation(f64::from(shift.x), f64::from(shift.y));
        let mut inks: Vec<(f32, PathBuilder)> = Vec::new();
        for posed in posed {
            let Some(outline) = &posed.letter.outline else {
                continue;
            };
            let opacity = posed.pose.opacity.min(1.0);
            let index = match inks
                .iter()
                .position(|(existing_opacity, _)| *existing_opacity == opacity)
            {
                Some(index) => index,
                None => {
                    inks.push((opacity, Path::builder()));
                    inks.len() - 1
                }
            };
            let map = posed.map.then(lift);
            let (_, builder) = &mut inks[index];
            *builder = traced(std::mem::take(builder), outline, map);
        }
        inks.into_iter()
            .map(|(opacity, builder)| (builder.build(), opacity))
            .filter(|(path, _)| !path.is_empty())
            .collect()
    }

    fn inked(&self, inks: &[(Path, f32)], shade: Option<Color>) -> Vec<Marking> {
        let rings: Vec<(f32, Color)> = match shade {
            Some(shade) => match self.lettering.outline_reach() {
                reach if reach > 0.0 => vec![(reach, shade)],
                _ => Vec::new(),
            },
            None => self
                .lettering
                .outlines
                .iter()
                .scan(0.0, |total, pen| {
                    *total += pen.width.max(0.0);
                    Some((*total, pen.color))
                })
                .collect(),
        };
        let mut marks: Vec<Marking> = Vec::new();
        for (reach, color) in rings.iter().rev() {
            for (path, opacity) in inks {
                marks.push(
                    PathMark::new(path.clone())
                        .stroked(
                            faded(*color, *opacity),
                            StrokeStyle::new(reach * 2.0 * self.scale)
                                .join(Join::Round)
                                .cap(Cap::Round),
                        )
                        .antialiased(self.lettering.antialias)
                        .into(),
                );
            }
        }
        for (path, opacity) in inks {
            marks.push(
                PathMark::new(path.clone())
                    .filled(
                        faded(shade.unwrap_or(self.lettering.color), *opacity),
                        FillRule::NonZero,
                    )
                    .antialiased(self.lettering.antialias)
                    .into(),
            );
        }
        marks
    }
}

fn traced(mut builder: PathBuilder, path: &Path, map: Affine) -> PathBuilder {
    for subpath in path.subpaths() {
        for (at, point) in subpath.points().iter().enumerate() {
            let point = map.apply(*point);
            builder = match at {
                0 => builder.move_to(point),
                _ => builder.line_to(point),
            };
        }
        if subpath.is_closed() {
            builder = builder.close();
        }
    }
    builder
}

fn recipe_of(steps: impl IntoIterator<Item = Step>) -> Recipe {
    let mut recipe = Recipe::new();
    for step in steps {
        recipe.push(step);
    }
    recipe
}

fn faded(color: Color, opacity: f32) -> Color {
    match opacity >= 1.0 {
        true => color,
        false => Color::rgba(color.r, color.g, color.b, color.a * opacity),
    }
}
