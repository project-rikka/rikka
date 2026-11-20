use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cut_project::{
    Appearance, ClipId, Edit, Format, Graphic, MediaId, Placement, Project, Redraw, Viewpoint,
    Visual,
};
use cut_render::Overrides;
use rikka_app::prelude::*;
use rikka_figure::{Form, Grips, ManipulationResult, TextHit, Typeset, enclosing};

use super::Stage;
use super::item_projection::ItemProjection;

pub(super) const HIT_REACH: f32 = 4.0;

#[derive(Clone, Debug)]
pub(crate) struct Item {
    pub(crate) clip: ClipId,
    pub(crate) locked: bool,
    pub(crate) visual: Visual,
    pub(crate) media: Option<MediaId>,
    pub(crate) projection: Option<ItemProjection>,
}

#[derive(Debug, Default)]
pub(crate) struct ItemCache(RefCell<Option<CachedItems>>);

#[derive(Debug)]
struct CachedItems {
    project: Arc<Project>,
    frame: i64,
    view: Viewpoint,
    overrides: Overrides,
    items: Rc<[Item]>,
}

impl Item {
    pub(crate) fn hits(&self, format: Format, at: Point, reach: f32) -> bool {
        let Some(projection) = &self.projection else {
            return false;
        };
        let Some(flat) = projection.to_plane(at) else {
            return false;
        };
        self.visual
            .hits(format, flat, reach / projection.stretch(flat))
    }

    pub(crate) fn outline(&self, format: Format) -> Option<Vec<Point>> {
        let grips = self.visual.grips(format)?;
        self.projection.as_ref()?.all_to_screen(&grips.outline())
    }

    pub(crate) fn bounds(&self, format: Format) -> Option<Rect> {
        let bounds = self.visual.bounds(format)?;
        Some(enclosing(
            self.projection.as_ref()?.all_to_screen(&bounds.corners())?,
        ))
    }

    pub(crate) fn anchor(&self) -> Option<Point> {
        let anchor = self.visual.placement().anchor;
        self.projection.as_ref()?.projection().map(anchor)
    }
}

impl Stage {
    pub(super) fn shown_project(&self) -> Arc<Project> {
        self.live.project_or(&self.project)
    }

    pub(super) fn animated_preview(
        &self,
        clip: ClipId,
        overrides: &Overrides,
    ) -> Option<Arc<Project>> {
        let project = self.shown_project();
        if !project.locate(clip)?.1.is_moving() {
            return None;
        }
        let Some(appearance) = overrides.appearance(clip) else {
            return Some(project);
        };
        let previewed = project
            .apply(&Edit::Redraw {
                changes: vec![(clip, appearance.clone())],
                by: Redraw::Style,
                at: self.transport.frame(),
            })
            .map_or(project, |(previewed, _)| Arc::new(previewed));
        Some(previewed)
    }

    pub(super) fn format(&self) -> Format {
        self.project.format()
    }

    pub(super) fn overrides(&self) -> Overrides {
        let mut overrides = self.live.overrides();
        let Some(draft) = self
            .draft
            .as_ref()
            .filter(|draft| draft.typing || draft.is_dirty())
        else {
            return overrides;
        };
        let project = self.shown_project();
        let frame = self.transport.frame();
        let visual = project
            .locate(draft.clip)
            .and_then(|(_, clip)| project.visual(clip, frame))
            .map(|visual| visual.with_appearance(overrides.appearance(draft.clip)));
        if let Some(Visual::Graphic(graphic)) = visual {
            overrides.show(draft.clip, Appearance::Graphic(draft.with_draft(&graphic)));
        }
        overrides
    }

    pub(super) fn items(&self) -> Rc<[Item]> {
        self.items_seen(self.view(), &self.overrides())
    }

    pub(super) fn items_seen(&self, view: Viewpoint, overrides: &Overrides) -> Rc<[Item]> {
        let project = self.shown_project();
        let frame = self.transport.frame();
        if let Some(sight) = self.sighting.0.borrow().as_ref()
            && Arc::ptr_eq(&sight.project, &project)
            && sight.frame == frame
            && sight.view == view
            && sight.overrides == *overrides
        {
            return Rc::clone(&sight.items);
        }
        let format = project.format();
        let items: Rc<[Item]> = project
            .shots_restyled(frame, view, |id| overrides.appearance(id))
            .into_iter()
            .filter_map(|(layer, clip, shot)| {
                let id = clip.id();
                let visual = project
                    .visual(clip, frame)?
                    .with_appearance(overrides.appearance(id));
                let projection =
                    ItemProjection::new(shot.projection, shot.path, &visual.placement(), format);
                Some(Item {
                    clip: id,
                    locked: project
                        .layer(layer)
                        .is_some_and(|layer_ref| layer_ref.is_locked()),
                    visual,
                    media: clip.media(),
                    projection,
                })
            })
            .collect();
        *self.sighting.0.borrow_mut() = Some(CachedItems {
            project,
            frame,
            view,
            overrides: overrides.clone(),
            items: Rc::clone(&items),
        });
        items
    }

    pub(super) fn item_of(&self, clip: ClipId) -> Option<Item> {
        self.items().iter().find(|item| item.clip == clip).cloned()
    }

    pub(super) fn selected_items<'a>(&self, items: &'a [Item]) -> Vec<&'a Item> {
        items
            .iter()
            .filter(|item| {
                !item.locked && item.projection.is_some() && self.selected.contains(&item.clip)
            })
            .collect()
    }

    pub(super) fn hit<'a>(&self, items: &'a [Item], at: Point, reach: f32) -> Option<&'a Item> {
        let format = self.format();
        items
            .iter()
            .rev()
            .filter(|item| !item.locked)
            .find(|item| item.hits(format, at, reach))
    }
}

pub(crate) fn group(selected: &[&Item], format: Format) -> Option<(Grips, ItemProjection)> {
    let projection = selected.first()?.projection.clone()?;
    let shared = selected.iter().all(|item| {
        item.projection
            .as_ref()
            .is_some_and(|other| other.matches(&projection))
    });
    if !shared {
        return None;
    }
    let grips = Grips::union(selected.iter().filter_map(|item| item.visual.grips(format)))?;
    Some((grips, projection))
}

pub(crate) fn reached(
    originals: &[(ClipId, Visual)],
    reach: ManipulationResult,
    format: Format,
) -> Vec<(ClipId, Visual)> {
    originals
        .iter()
        .map(|(clip, visual)| {
            let moved = match reach {
                ManipulationResult::Mapped { map, .. } => visual.mapped(map, format),
                ManipulationResult::Ended { from, to, .. } => {
                    visual.with_endpoints(from, to, format)
                }
            };
            (*clip, slid_like(visual, moved))
        })
        .collect()
}

fn slid_like(original: &Visual, reached: Visual) -> Visual {
    let (before, after) = (original.placement(), reached.placement());
    let by = after.position() - before.position();
    let placement = Placement {
        x: before.x,
        y: before.y,
        depth: before.depth,
        ..after
    }
    .slid(by);
    reached.placed(placement)
}

pub(crate) fn appearances(visuals: Vec<(ClipId, Visual)>) -> Vec<(ClipId, Appearance)> {
    visuals
        .into_iter()
        .map(|(clip, visual)| (clip, visual.appearance()))
        .collect()
}

pub(crate) fn typeset_of(graphic: &Graphic) -> Option<Typeset> {
    match graphic.art.form() {
        Form::Text(lettering) => Some(Typeset::new(&lettering)),
        _ => None,
    }
}

pub(crate) fn text_hit(item: &Item, at: Point) -> Option<TextHit> {
    let typeset = typeset_of(item.visual.graphic()?)?;
    let local = item.projection.as_ref()?.projection().unmap(at)?;
    Some(typeset.hit(local))
}
