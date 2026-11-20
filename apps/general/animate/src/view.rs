use animate_fluid::Pour;
use animate_inspector::HoldEditing;
use animate_inspector::{CARDS_CORNER, Desk, Reach, cards, inspector};
use animate_sheet::HoldSpan;
use animate_stage::Blowing;
use animate_timeline::{Dope, Names, playhead};
use animate_viewport::{Gesture, Screen};
use rikka_app::prelude::*;
use rikka_ui::{Accept, DragAction, DragOffer};
use rikka_widgets::{column, row};

use crate::animate::{Animate, DockTab, studio, workshop};
use crate::command::Command;
use crate::{bar, dialogs};

const URI_LIST: &str = "text/uri-list";

pub(crate) fn view(animate: &Animate) -> impl View {
    let ratio = animate.split().clone();
    let workspace = split(
        Axis::Vertical,
        animate.split().first(),
        stage(animate),
        dock(animate),
    )
    .on_resize(move |share: f32| {
        ratio.set(share);
        skip_view();
    })
    .fill();
    let body = row! {
        container(workspace).grow(1.0).fill_height();
        if animate.is_inspector_open() {
            rule_upright();
            inspector(Desk::new(animate, Reach::new(studio)), || {
                animate_shader_editor::node_properties(animate_shader_editor::Desk::new(
                    animate,
                    animate_shader_editor::Reach::new(workshop),
                ))
                .any()
            });
        }
    }
    .fill();

    let shortcuts = shortcuts(body, Command::pressed, Command::enabled, Command::run)
        .standard(Command::from_standard)
        .paused(dialogs::is_open(animate));
    let whole = container(shortcuts).fill();

    let mut layers = vec![
        drop_zone(whole)
            .accepts(|offer: &DragOffer| offer.offers(URI_LIST).then(|| Accept::copy(URI_LIST)))
            .on_drop(|offer: DragOffer, _: DragAction| Animate::dropped(offer))
            .fill()
            .build()
            .keyed(dialogs::STAGE),
    ];
    layers.extend(dialogs::view(animate));
    column_of(layers).fill()
}

fn stage(animate: &Animate) -> impl View {
    let screen = Element::new(
        Screen::new(
            animate.viewport(),
            animate.stage(),
            animate.transport(),
            animate.timeline(),
            animate.aim(),
            |animate: &mut Animate, gesture: Gesture| match gesture {
                Gesture::Focus(marker) => animate.focus(marker),
                Gesture::Pose(after) => animate.preview(*after),
                Gesture::Express(expression, weight) => animate.express(|faces, weights| {
                    faces.express(weights, expression, weight);
                }),
                Gesture::Grasp(hold) => animate.grasp(*hold),
                Gesture::Hold(figure, hold) => animate.lay_hold(figure, *hold),
                Gesture::ReleaseHold(figure, hold) => animate.release_hold(figure, hold),
                Gesture::Pour(emitter, setting, value) => animate.apply(emitter, setting, value),
                Gesture::Attach {
                    setting,
                    anchor,
                    offset,
                } => animate.attach(setting, anchor, offset),
                Gesture::Wind(setting, value) => animate.apply(Blowing, setting, value),
                Gesture::Tailor(figure, cloth_settings) => {
                    animate.apply_cloth_settings(figure, cloth_settings)
                }
                Gesture::Place(node, by) => animate.place(node, by),
            },
        )
        .framed(animate.output().resolution())
        .marking(animate.marked()),
    )
    .grow(1.0);
    let corner = spacer()
        .height(0.0)
        .fill_width()
        .build()
        .keyed(CARDS_CORNER);
    let mut layers = vec![corner, screen];
    if !dialogs::is_open(animate)
        && let Some(cards) = cards(Desk::new(animate, Reach::new(studio)))
    {
        layers.push(cards);
    }
    column_of(layers).fill()
}

fn dock(animate: &Animate) -> impl View {
    let dock = animate.dock();
    let content = match (dock.split, dock.tab) {
        (true, _) => {
            let ratio = dock.ratio.clone();
            split(
                Axis::Horizontal,
                dock.ratio.first(),
                timeline(animate),
                nodes(animate),
            )
            .on_resize(move |share: f32| {
                ratio.set(share);
                skip_view();
            })
            .fill()
            .any()
        }
        (false, DockTab::Timeline) => timeline(animate).any(),
        (false, DockTab::Nodes) => nodes(animate).any(),
    };
    column! {
        bar::bar(animate);
        container(content).grow(1.0).fill_width();
    }
    .fill()
}

fn nodes(animate: &Animate) -> impl View {
    animate_shader_editor::node_editor(animate_shader_editor::Desk::new(
        animate,
        animate_shader_editor::Reach::new(workshop),
    ))
}

fn timeline(animate: &Animate) -> impl View {
    column! {
            Element::new(Dope::new(
                animate.timeline(),
                animate.transport(),
                Names::new(animate.transport().sound_name()),
                |animate: &mut Animate, by: i64| animate.shift_keys(by),
                |animate: &mut Animate, pour: Pour| animate.retime_pour(pour),
                |animate: &mut Animate, span: HoldSpan| animate.retime_chosen_hold(span),
            ))
            .grow(1.0);
            playhead(animate.timeline(), animate.transport())
                .build()
                .position(0.0, 0.0)
                .fill();
    }
    .fill()
}
