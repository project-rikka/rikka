mod bar;

use animate_body::{Morph, MorphWeights, Panel, SidedMorph, group_sides};
use animate_motion::Morphs;
use animate_ui::glyph;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::{Desk, Reach, wording};
use bar::MorphBar;

const ROW: f32 = 24.0;
const EXPANDER: f32 = 24.0;
const INDENT: f32 = 12.0;
const GLYPH: f32 = 16.0;
const RESTING: f32 = 0.35;

struct Facts<'a> {
    weights: MorphWeights,
    pending: MorphWeights,
    chosen: Option<String>,
    morphs: &'a Morphs,
    frame: u32,
    typed: Option<&'a [String]>,
    nested: bool,
}

impl Facts<'_> {
    fn pending(&self, morph: &Morph) -> bool {
        self.pending.holds(morph.name())
    }

    fn here(&self, morph: &Morph) -> bool {
        self.morphs.key(morph.name(), self.frame).is_some()
    }

    fn keyed(&self, morph: &Morph) -> bool {
        !self.morphs.keys(morph.name()).is_empty()
    }
}

pub(super) fn morphs(desk: Desk<'_>) -> Option<Element> {
    let figure = desk.chosen_figure()?;
    if !desk.morphs_open() {
        return Some(header(
            desk.reach(),
            false,
            !desk.unregistered_morphs().is_empty(),
        ));
    }
    let (weights, pending) = desk.morphs_now()?;
    let header = header(desk.reach(), true, !pending.is_empty());
    let listed: Vec<&Morph> = figure
        .faces
        .morphs()
        .iter()
        .filter(|morph| morph.panel().is_some())
        .collect();
    if listed.is_empty() {
        return Some(
            column! {
                header;
                text(tr!("animate.morph.none")).caption().muted();
            }
            .gap(Space::TIGHT)
            .fill_width()
            .any(),
        );
    }
    let panels: Vec<Panel> = Panel::ALL
        .into_iter()
        .filter(|panel| listed.iter().any(|morph| morph.panel() == Some(*panel)))
        .collect();
    let panel = Some(desk.controls().morph_panel)
        .filter(|panel| panels.contains(panel))
        .unwrap_or(panels[0]);
    let query = desk.morph_query().text();
    let query = query.trim();
    let searching = !query.is_empty();
    let groups = match searching {
        true => group_sides(
            figure
                .faces
                .search(query)
                .filter(|morph| morph.panel().is_some()),
        ),
        false => group_sides(
            listed
                .iter()
                .copied()
                .filter(|morph| morph.panel() == Some(panel)),
        ),
    };
    let facts = Facts {
        weights,
        pending,
        chosen: desk.chosen_morph(),
        morphs: &figure.take.morphs,
        frame: desk.transport().frame(),
        typed: desk.typed_morphs(),
        nested: groups.iter().any(SidedMorph::has_sides),
    };
    let index = panels
        .iter()
        .position(|candidate| *candidate == panel)
        .unwrap_or(0);
    let tabs = segmented(index)
        .segments(panels.iter().copied().map(wording::panel))
        .on_select(desk.act_with(move |studio, index: usize| {
            if let Some(panel) = panels.get(index) {
                studio.controls_mut().morph_panel = *panel;
            }
        }))
        .enabled(!searching)
        .focusless()
        .control_size(ControlSize::Small)
        .fill_width();
    let list = match groups.is_empty() {
        true => text(tr!("animate.morph.unfound")).caption().muted().any(),
        false => column_of(groups.iter().flat_map(|sided| group(desk, &facts, sided)))
            .gap(Space::TIGHT)
            .fill_width()
            .any(),
    };
    Some(
        column! {
            header;
            tabs;
            text_field(desk.morph_query())
                .placeholder(tr!("animate.morph.search"))
                .control_size(ControlSize::Small)
                .fill_width();
            list;
        }
        .gap(Space::TIGHT)
        .fill_width()
        .any(),
    )
}

fn header(reach: Reach, open: bool, pending: bool) -> Element {
    let fold = button(
        row! {
            icon(match open {
                true => MaterialSymbol::EXPAND_MORE,
                false => MaterialSymbol::CHEVRON_RIGHT,
            })
            .extent(Length::px(GLYPH));
            text(tr!("animate.morph.title")).bold().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .focusless()
    .on_press(reach.act(move |studio| studio.show_morphs(!open)));
    row! {
        fold;
        spacer().grow(1.0);
        glyph(MaterialSymbol::RESTART_ALT)
            .ghost()
            .enabled(pending)
            .on_press(reach.act(move |studio| studio.revert_morphs()))
            .tooltip(tr!("animate.tip.revert_expressions"));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn group(desk: Desk<'_>, facts: &Facts<'_>, sided: &SidedMorph<'_>) -> Vec<Element> {
    let name = sided.name.to_owned();
    let expanded = sided.has_sides() && desk.sides_open(&name);
    let members: Vec<&Morph> = match sided.whole {
        Some(whole) => vec![whole],
        None => sided.members().collect(),
    };
    let names: Vec<String> = members
        .iter()
        .map(|morph| morph.name().to_owned())
        .collect();
    let weight = members
        .iter()
        .map(|morph| facts.weights.weight(morph.name()))
        .fold(0.0, f32::max);
    let hidden_pending = !expanded && sided.members().any(|morph| facts.pending(morph));
    let opener = name.clone();
    let expander = match sided.has_sides() {
        true => glyph(match expanded {
            true => MaterialSymbol::EXPAND_MORE,
            false => MaterialSymbol::CHEVRON_RIGHT,
        })
        .ghost()
        .on_press(desk.act(move |studio| studio.toggle_sides(&opener)))
        .any(),
        false => spacer().any(),
    };
    let picked = match sided.whole {
        Some(whole) => Pick::Lane(whole.name().to_owned()),
        None => Pick::Open(name.clone()),
    };
    let mut rows = vec![line(
        desk,
        facts
            .nested
            .then(|| container(expander).width(EXPANDER).any()),
        Slot {
            label: name,
            weight,
            pending: hidden_pending || members.iter().any(|morph| facts.pending(morph)),
            chosen: facts.chosen.as_deref() == names.first().map(String::as_str)
                && sided.whole.is_some(),
            here: members.iter().all(|morph| facts.here(morph)),
            keyed: members.iter().any(|morph| facts.keyed(morph)),
            typing: facts.typed == Some(names.as_slice()),
            names,
            picked,
        },
    )];
    if expanded {
        rows.extend(
            [sided.left, sided.right]
                .into_iter()
                .flatten()
                .map(|morph| {
                    let names = vec![morph.name().to_owned()];
                    line(
                        desk,
                        Some(spacer().width(EXPANDER + INDENT).any()),
                        Slot {
                            label: morph.name().to_owned(),
                            weight: facts.weights.weight(morph.name()),
                            pending: facts.pending(morph),
                            chosen: facts.chosen.as_deref() == Some(morph.name()),
                            here: facts.here(morph),
                            keyed: facts.keyed(morph),
                            typing: facts.typed == Some(names.as_slice()),
                            picked: Pick::Lane(morph.name().to_owned()),
                            names,
                        },
                    )
                }),
        );
    }
    rows
}

enum Pick {
    Lane(String),
    Open(String),
}

struct Slot {
    label: String,
    names: Vec<String>,
    weight: f32,
    pending: bool,
    chosen: bool,
    here: bool,
    keyed: bool,
    typing: bool,
    picked: Pick,
}

fn line(desk: Desk<'_>, lead: Option<Element>, slot: Slot) -> Element {
    let Slot {
        label,
        names,
        weight,
        pending,
        chosen,
        here,
        keyed,
        typing,
        picked,
    } = slot;
    let keying = names.clone();
    let key = glyph(MaterialSymbol::DIAMOND)
        .on_press(desk.act(move |studio| studio.toggle_morph_keys(&keying)));
    let key = match (here, keyed, pending) {
        (true, _, _) => key.primary().any(),
        (false, true, _) => key.secondary().any(),
        (false, false, true) => key.ghost().any(),
        (false, false, false) => key.ghost().opacity(RESTING).any(),
    };
    let key = key.tooltip(match here {
        true => tr!("animate.tip.remove_key"),
        false => tr!("animate.tip.key"),
    });
    let value = match typing {
        true => entry(desk),
        false => {
            let slid = names.clone();
            Element::new(
                MorphBar::new(
                    label,
                    weight,
                    desk.act_with(move |studio, weight: f32| studio.set_morphs(&slid, weight)),
                    desk.act(move |studio| match &picked {
                        Pick::Lane(morph) => studio.choose_morph(morph),
                        Pick::Open(name) => studio.toggle_sides(name),
                    }),
                    desk.act(move |studio| studio.type_morphs(names.clone(), weight)),
                )
                .pending(pending)
                .chosen(chosen),
            )
            .height(ROW)
            .grow(1.0)
            .any()
        }
    };
    row_of(lead.into_iter().chain([value, key]))
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
}

fn entry(desk: Desk<'_>) -> Element {
    text_field(desk.morph_entry())
        .claims_focus(true)
        .control_size(ControlSize::Small)
        .on_submit(desk.act(move |studio| studio.apply_typed_morphs()))
        .on_blur(desk.act(move |studio| studio.apply_typed_morphs()))
        .on_key(desk.act_with_pair(move |studio, key: Key, _: Modifiers| {
            let leaving = key == Key::Escape;
            if leaving {
                studio.drop_typed_morphs();
            }
            leaving
        }))
        .height(ROW)
        .grow(1.0)
        .any()
}
