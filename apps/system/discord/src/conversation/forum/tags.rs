use discord_core::{ForumSettings, ForumTag, TagId};
use rikka_app::prelude::*;
use rikka_widgets::{Container, MenuItem, Toolbar, menu_item, row, tool_group, tool_item, toolbar};

use super::Forum;
use crate::chat::Chat;

const PILL: f32 = 15.0;
const CHIP: f32 = 10.0;
const PILL_EMOJI: f32 = 16.0;
const CHIP_EMOJI: f32 = 14.0;

pub(super) fn pill(content: impl View, lit: bool) -> Container {
    let palette = palette();
    let (fill, edge) = match lit {
        true => (palette.accent.with_alpha(0.18), palette.accent),
        false => (palette.surface, palette.outline),
    };
    container(content)
        .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
        .background(fill)
        .border(1.0, edge)
        .radius(PILL)
        .focusless()
}

pub(super) fn chip(tag: &ForumTag) -> Element {
    face(tag, CHIP_EMOJI)
        .padding(Inset::xy(Space::SNUG, Space::HAIR))
        .background(palette().surface_variant)
        .radius(CHIP)
        .any()
}

pub(super) fn chips(tags: &[ForumTag]) -> Element {
    row_of(tags.iter().map(chip))
        .gap(Space::TIGHT)
        .wrap_items()
        .fill_width()
        .any()
}

pub(super) fn filter_bar(tags: &[ForumTag], filter: &[TagId]) -> Toolbar {
    tags.iter().fold(bar(), |bar, tag| {
        let (id, lit) = (tag.id, filter.contains(&tag.id));
        let toggle = move |chat: &mut Chat| chat.filter_posts(id);
        bar.group(tool_group([tool_item(
            pill(face(tag, PILL_EMOJI), lit).on_press(toggle),
        )
        .overflow([listed(tag, lit).on_press(toggle)])]))
    })
}

pub(super) fn choice_bar(settings: &ForumSettings, chosen: &[TagId], manages: bool) -> Toolbar {
    let full = chosen.len() >= ForumSettings::MAX_POST_TAGS;
    settings
        .tags
        .iter()
        .filter(|tag| !tag.moderated || manages)
        .fold(bar(), |bar, tag| {
            let (id, lit) = (tag.id, chosen.contains(&tag.id));
            let open = lit || !full;
            let toggle = move |forum: &mut Forum| forum.toggle_tag(id);
            let face = pill(face(tag, PILL_EMOJI), lit);
            let face = match open {
                true => face.on_press(toggle).any(),
                false => face.opacity(0.4).any(),
            };
            bar.group(tool_group([
                tool_item(face).overflow([listed(tag, lit).enabled(open).on_press(toggle)])
            ]))
        })
}

fn bar() -> Toolbar {
    toolbar()
        .gap(Space::TIGHT)
        .padding(Inset::NONE)
        .fill_width()
}

fn face(tag: &ForumTag, emoji: f32) -> Container {
    container(
        row! {
            if let Some(shown) = &tag.emoji {
                crate::conversation::emoji::face(shown, emoji);
            }
            text(tag.name.clone()).caption().semibold().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
}

fn listed(tag: &ForumTag, lit: bool) -> MenuItem {
    let label = match &tag.emoji {
        Some(emoji) if !emoji.is_custom() => format!("{} {}", emoji.name, tag.name),
        _ => tag.name.clone(),
    };
    menu_item(label).selected(lit)
}
