use cut_project::{Appearance, Camera, ClipId, Edit, Graphic, Redraw, Region, Treatment};
use rikka_app::prelude::*;

use super::Cut;

pub(crate) trait Adjustable: Sized + 'static {
    fn selected(cut: &Cut) -> Option<(ClipId, Self)>;

    fn appearance(self) -> Appearance;

    fn preview(cut: &mut Cut, id: ClipId, styled: Self) {
        cut.live.show(id, styled.appearance());
    }
}

impl Adjustable for Graphic {
    fn selected(cut: &Cut) -> Option<(ClipId, Graphic)> {
        cut.selected_graphic()
    }

    fn appearance(self) -> Appearance {
        Appearance::Graphic(self)
    }
}

impl Adjustable for Treatment {
    fn selected(cut: &Cut) -> Option<(ClipId, Treatment)> {
        let clip = cut.selected_clip()?;
        Some((clip.id(), clip.treatment_at(cut.transport.frame())?))
    }

    fn appearance(self) -> Appearance {
        Appearance::Footage(self)
    }
}

impl Adjustable for Region {
    fn selected(cut: &Cut) -> Option<(ClipId, Region)> {
        let clip = cut.selected_clip()?;
        let placement = clip.placement_at(cut.transport.frame());
        Some((clip.id(), clip.region()?.with_placement(placement)))
    }

    fn appearance(self) -> Appearance {
        Appearance::Region(self)
    }
}

impl Adjustable for Camera {
    fn selected(cut: &Cut) -> Option<(ClipId, Camera)> {
        let clip = cut.selected_clip()?;
        Some((clip.id(), clip.camera_at(cut.transport.frame())?))
    }

    fn appearance(self) -> Appearance {
        Appearance::Camera(self)
    }

    fn preview(cut: &mut Cut, id: ClipId, camera: Camera) {
        let at = cut.transport.frame();
        cut.preview_edit(&Edit::Redraw {
            changes: vec![(id, Appearance::Camera(camera))],
            by: Redraw::Style,
            at,
        });
    }
}

impl Cut {
    pub(crate) fn restyle<S: Adjustable>(&mut self, change: impl FnOnce(&mut S)) {
        self.commit_text();
        let Some((id, mut styled)) = S::selected(self) else {
            return;
        };
        change(&mut styled);
        self.redraw(vec![(id, styled.appearance())], Redraw::Style);
    }

    pub(crate) fn preview_style<S: Adjustable>(&mut self, change: impl FnOnce(&mut S)) {
        let Some((id, mut styled)) = S::selected(self) else {
            skip_view();
            return;
        };
        change(&mut styled);
        S::preview(self, id, styled);
        skip_view();
    }
}
