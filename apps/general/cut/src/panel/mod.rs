mod animation_presets;
mod camera;
mod diamond;
mod effects;
mod footage;
mod keys;
mod line;
mod outline;
mod placement;
mod region;
mod rows;
mod shape;
mod text;
mod transition;
mod tween;

use cut_project::{Appearance, Art, Clip, Graphic, Project, Region, Source, Treatment};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::cut::{Cut, Popup, Side};
use crate::easing;
use crate::gallery;
use keys::Keying;
use placement::Row;

pub(crate) const WIDTH: f32 = 300.0;
const SIDES: [Side; 2] = [Side::Properties, Side::Transitions];

pub(crate) fn panel(cut: &Cut) -> impl View {
    let frame = cut.transport().frame();
    let project = cut.shown_project();
    let selected = cut
        .selected_clip()
        .and_then(|clip| project.locate(clip.id()))
        .map(|(_, clip)| clip);
    let body = match (cut.side(), cut.selected_edit_point()) {
        (Side::Transitions, _) => Some(gallery::gallery(cut).build()),
        (Side::Properties, Some(edit_point)) => transition::settings(&project, edit_point),
        (Side::Properties, None) => {
            selected.and_then(|clip| properties(cut, &project, clip, frame))
        }
    };
    let popup = match cut.popup() {
        Some(Popup::Effects) => selected.map(|_| effects::adding()),
        Some(Popup::Easing(picking)) => easing::picker(&project, picking),
        _ => None,
    };
    let side = SIDES
        .iter()
        .position(|side| *side == cut.side())
        .unwrap_or(0);
    container(
        column! {
            container(
                segmented(side)
                    .segments([tr!("cut.transition.properties"), tr!("cut.transition.gallery")])
                    .control_size(ControlSize::Small)
                    .focusless()
                    .on_select(|cut: &mut Cut, at: usize| cut.show_side(SIDES[at]))
                    .fill_width(),
            )
            .padding(Inset::xy(Space::NORMAL, Space::SNUG))
            .fill_width();
            page_stack(
                side,
                scrollable(
                    column! {
                        if let Some(body) = body {
                            body;
                        }
                    }
                    .gap(Space::NORMAL)
                    .padding(Space::NORMAL)
                    .fill_width(),
                )
                .fill(),
            )
            .grow(1.0)
            .fill_width();
            if let Some(popup) = popup {
                popup;
            }
        }
        .fill(),
    )
    .background(palette().surface)
    .width(WIDTH)
    .fill_height()
}

fn properties(cut: &Cut, project: &Project, clip: &Clip, frame: i64) -> Option<Element> {
    let keying = Keying::for_clip(clip, frame);
    let mut groups = Vec::new();
    if clip.is_animated() {
        groups.push(keys::navigation(cut));
    }
    let animation_presets = || animation_presets::animation_presets(clip).build();
    let effecting = || effects::effects(cut, clip, keying, frame).build();
    let preview = cut.live().appearance(clip.id());
    let placement = clip.placement_at(frame);
    let moving = || tween::tween(clip, frame).build();
    match clip.source() {
        Source::Camera(_) => {
            groups.push(moving());
            groups.push(camera::camera(clip, clip.camera_at(frame)?, keying).build());
        }
        Source::Graphic(_) => {
            let graphic = match preview {
                Some(Appearance::Graphic(previewed)) => previewed,
                _ => Graphic {
                    placement,
                    art: clip.art_at(frame)?,
                },
            };
            groups.push(look(cut, clip, &graphic, keying));
            groups.push(moving());
            groups.push(
                placement::placement::<Graphic>(clip, graphic.placement, keying, &Row::ALL).build(),
            );
            groups.push(animation_presets());
            groups.push(effecting());
        }
        Source::Footage { media, .. } => {
            let treatment = match preview {
                Some(Appearance::Footage(previewed)) => previewed,
                _ => clip.treatment_at(frame)?,
            };
            let media = project.media(*media)?;
            if let Some(picture) = media.picture() {
                let picture = project.format().fitted(picture.width, picture.height);
                groups.push(moving());
                groups.push(
                    placement::placement::<Treatment>(clip, treatment.placement, keying, &Row::ALL)
                        .build(),
                );
                groups.push(footage::picture(&treatment, picture).build());
                groups.push(footage::playing(&treatment).build());
                groups.push(animation_presets());
                groups.push(effecting());
            }
            if media.sound().is_some() {
                groups.push(footage::sound(clip, treatment.mix, keying, project.rate()).build());
            }
        }
        Source::Region(region) => {
            let region = match preview {
                Some(Appearance::Region(previewed)) => previewed,
                _ => region.with_placement(placement),
            };
            groups.push(region::area(&region, project.format()).build());
            groups.push(moving());
            let fields: &[Row] = match region.is_whole() {
                true => &[Row::Opacity],
                false => &Row::ALL,
            };
            groups.push(
                placement::placement::<Region>(clip, region.placement, keying, fields).build(),
            );
            groups.push(animation_presets());
            groups.push(effecting());
        }
    }
    Some(
        column_of(groups)
            .gap(Space::ROOMY)
            .fill_width()
            .build()
            .keyed(("cut-panel", clip.id())),
    )
}

fn look(cut: &Cut, clip: &Clip, graphic: &Graphic, keying: Keying) -> Element {
    match &graphic.art {
        Art::Shape { .. } => shape::shape(&graphic.art, clip, keying).build(),
        Art::Line { .. } => line::line(&graphic.art, clip, keying).build(),
        Art::Text { lettering, reveal } => {
            text::text(cut, (lettering, *reveal), clip, keying).build()
        }
    }
}
