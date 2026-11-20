use animate_body::Body;
use animate_motion::PoseSample;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::cards::{card, expression, eyes, mouth};
use crate::{Card, Desk, FaceTab, wording};

pub(super) fn face(desk: Desk<'_>, body: &Body, pose_sample: &PoseSample, tab: FaceTab) -> Element {
    let chosen = FaceTab::ALL
        .iter()
        .position(|candidate| *candidate == tab)
        .unwrap_or(0);
    let tabs = segmented(chosen)
        .segments(FaceTab::ALL.map(wording::face_tab))
        .on_select(desk.act_with(move |studio, index: usize| {
            if let Some(tab) = FaceTab::ALL.get(index) {
                studio.open_card(Some(Card::Face(*tab)));
            }
        }))
        .focusless()
        .control_size(ControlSize::Small)
        .fill_width();
    let faces = desk.chosen_faces().cloned().unwrap_or_default();
    let weights = desk.current_morph_weights().unwrap_or_default();
    let page = match tab {
        FaceTab::Eyes => eyes::eyes(desk, body, &faces, pose_sample, &weights).any(),
        FaceTab::Expression => {
            expression::expression(desk.reach(), &faces, &weights, desk.controls().mood).any()
        }
        FaceTab::Mouth => mouth::mouth(desk.reach(), &faces, &weights).any(),
    };
    let body = column! {
        tabs;
        page;
    }
    .gap(Space::SNUG)
    .fill_width();
    card(desk.reach(), tr!("animate.card.face"), body).build()
}
