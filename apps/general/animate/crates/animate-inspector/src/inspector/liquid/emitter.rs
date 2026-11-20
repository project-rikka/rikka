use std::sync::Arc;

use animate_cast::Figure;
use animate_fluid::{Aim, Anchor, Emitter, EmitterSetting, Shape, Spot};
use animate_ui::{framed, heading, labelled};
use rikka_app::prelude::*;

use crate::inspector::liquid::row::picker;
use crate::{Desk, Reach, wording};

pub(super) fn card(desk: Desk<'_>, emitter: &Arc<Emitter>) -> Element {
    let mut parts = vec![
        heading(tr!("animate.liquid.emitter")),
        labelled(
            tr!("animate.liquid.name"),
            text_field(desk.naming())
                .on_submit(desk.act(move |studio| studio.rename_emitter()))
                .on_blur(desk.act(move |studio| studio.rename_emitter()))
                .control_size(ControlSize::Small)
                .fill_width()
                .any(),
            None,
        ),
        labelled(
            tr!("animate.liquid.shape"),
            shape_picker(desk, &emitter.shape),
            None,
        ),
    ];
    if emitter.shape.aims() {
        parts.push(labelled(
            tr!("animate.liquid.aim"),
            aim_picker(desk.reach(), emitter.aim),
            None,
        ));
    }
    if let Shape::Blob { size } = emitter.shape {
        let displayed_size = desk.controls().blob_draft.unwrap_or(size);
        parts.push(labelled(
            tr!("animate.liquid.blob_size"),
            slider(displayed_size)
                .range(Shape::BLOB_SIZES)
                .control_size(ControlSize::Small)
                .on_change(desk.act_with(move |studio, size: f32| studio.draft_blob(size)))
                .on_commit(desk.act_with(move |studio, size: f32| studio.apply_blob(size)))
                .fill_width()
                .any(),
            None,
        ));
    }
    if let Shape::Surface { figure, materials } = &emitter.shape {
        parts.extend(surface(desk, figure, materials));
    }
    for setting in [EmitterSetting::Launch, EmitterSetting::Target] {
        if let Some(spot) = emitter.spot(setting).filter(|_| emitter.applies(setting)) {
            parts.extend(attachment(desk, setting, spot));
        }
    }
    framed(parts)
}

fn shapes(current: &Shape, figure: Option<&Figure>) -> [Shape; 5] {
    let blob = match current {
        Shape::Blob { .. } => current.clone(),
        _ => Shape::blob(),
    };
    let surface = match current {
        Shape::Surface { .. } => current.clone(),
        _ => Shape::surface(
            figure.map_or_else(String::new, |figure| figure.name.clone()),
            [],
        ),
    };
    [Shape::Stream, Shape::Shower, Shape::Drip, blob, surface]
}

fn shape_picker(desk: Desk<'_>, current: &Shape) -> Element {
    let offered = shapes(current, desk.cast().figures().first());
    let chosen = offered.iter().position(|shape| shape.id() == current.id());
    let labels = offered.iter().map(wording::shape).collect();
    picker(chosen, labels)
        .on_select(desk.act_with(move |studio, index: usize| {
            if let Some(shape) = offered.get(index) {
                studio.choose_shape(shape.clone());
            }
        }))
        .fill_width()
        .any()
}

fn aim_picker(reach: Reach, aim: Aim) -> Element {
    let chosen = Aim::ALL
        .iter()
        .position(|candidate| *candidate == aim)
        .unwrap_or(0);
    segmented(chosen)
        .segments(Aim::ALL.map(wording::aim))
        .control_size(ControlSize::Small)
        .on_select(reach.act_with(move |studio, index: usize| {
            if let Some(aim) = Aim::ALL.get(index) {
                studio.choose_aim(*aim);
            }
        }))
        .fill_width()
        .any()
}

fn surface(desk: Desk<'_>, figure: &str, materials: &[u32]) -> Vec<Element> {
    let figures = desk.cast().figures();
    let names: Vec<String> = figures.iter().map(|figure| figure.name.clone()).collect();
    let chosen = names.iter().position(|name| name == figure);
    let mut parts = vec![labelled(
        tr!("animate.liquid.figure"),
        picker(
            chosen,
            names
                .iter()
                .map(|name| Label::from(name.as_str()))
                .collect(),
        )
        .placeholder(tr!("animate.liquid.no_figure"))
        .on_select(desk.act_with(move |studio, index: usize| {
            if let Some(name) = names.get(index) {
                studio.choose_surface_figure(name.clone());
            }
        }))
        .fill_width()
        .any(),
        None,
    )];
    let Some(figure) = chosen.and_then(|at| figures.get(at)) else {
        return parts;
    };
    parts.push(
        text(tr!("animate.liquid.materials"))
            .caption()
            .muted()
            .any(),
    );
    parts.extend(
        figure
            .model
            .materials
            .iter()
            .enumerate()
            .map(|(index, material)| {
                let index = index as u32;
                let covered = materials.binary_search(&index).is_ok();
                checkbox(covered)
                    .label(text(material.name.as_str()).caption())
                    .on_toggle(desk.act_with(move |studio, covered: bool| {
                        studio.cover_material(index, covered);
                    }))
                    .any()
            }),
    );
    parts
}

fn attachment(desk: Desk<'_>, setting: EmitterSetting, spot: &Spot) -> Vec<Element> {
    let figures = desk.cast().figures();
    let mut options = vec![tr!("animate.liquid.world")];
    options.extend(
        figures
            .iter()
            .map(|figure| Label::from(figure.name.as_str())),
    );
    let (chosen, bone) = match &spot.anchor {
        Anchor::World => (Some(0), None),
        Anchor::Bone { figure, bone } => (
            figures
                .iter()
                .position(|candidate| candidate.name == *figure)
                .map(|at| at + 1),
            Some(bone.clone()),
        ),
    };
    let figure_names: Vec<String> = figures.iter().map(|figure| figure.name.clone()).collect();
    let bones: Vec<Vec<String>> = figures.iter().map(shown_bones).collect();
    let first_bones = bones.clone();
    let mut parts =
        vec![labelled(
            animate_ui::emitter_setting_label(setting),
            picker(chosen, options)
                .on_select(desk.act_with(move |studio, index: usize| {
                    let anchor =
                        match index.checked_sub(1) {
                            None => Some(Anchor::World),
                            Some(at) => figure_names.get(at).zip(first_bones.get(at)).map(
                                |(figure, bones)| {
                                    Anchor::bone(
                                        figure.as_str(),
                                        bones.first().map_or("", String::as_str),
                                    )
                                },
                            ),
                        };
                    if let Some(anchor) = anchor {
                        studio.anchor(setting, anchor);
                    }
                }))
                .fill_width()
                .any(),
            None,
        )];
    let Some((figure, at)) = chosen
        .and_then(|at| at.checked_sub(1))
        .and_then(|at| Some((figures.get(at)?.name.clone(), at)))
    else {
        return parts;
    };
    let names = bones.get(at).cloned().unwrap_or_default();
    let picked = bone.and_then(|bone| names.iter().position(|name| *name == bone));
    parts.push(labelled(
        tr!("animate.liquid.bone"),
        picker(
            picked,
            names
                .iter()
                .map(|name| Label::from(name.as_str()))
                .collect(),
        )
        .on_select(desk.act_with(move |studio, index: usize| {
            if let Some(bone) = names.get(index) {
                studio.anchor(setting, Anchor::bone(figure.as_str(), bone.as_str()));
            }
        }))
        .fill_width()
        .any(),
        None,
    ));
    parts
}

fn shown_bones(figure: &Figure) -> Vec<String> {
    figure
        .skeleton
        .bones()
        .iter()
        .filter(|bone| bone.shown)
        .map(|bone| bone.name.clone())
        .collect()
}
