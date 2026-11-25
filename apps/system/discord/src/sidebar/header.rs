use discord_core::SpaceId;
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::row;

use super::Sidebar;
use crate::account::Scope;
use crate::chat::Chat;
use crate::menus::Subject;
use crate::pictures;
use crate::widgets::{context_area, mark_of};

pub(crate) const HEIGHT: f32 = 48.0;
const BANNER: f32 = 136.0;

pub(crate) fn area(scope: Option<&Scope>) -> Element {
    let header = component(|chat: &Chat| chat, view);
    let Some(Scope::Space(space)) = scope else {
        return header.any();
    };
    let space = *space;
    context_area(
        mark_of(("discord-space-header-menu", space)),
        header,
        Handler::new(move |chat: &mut Chat, anchor: Anchor| {
            chat.open_context_menu(Subject::Space(space), anchor);
        }),
    )
}

fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().height(HEIGHT).any();
    };
    match &linked.location().scope {
        Scope::Home => search(chat.sidebar()).any(),
        Scope::Space(space) => {
            let Some(space_info) = linked.space(space) else {
                return spacer().height(HEIGHT).any();
            };
            let banner = space_info
                .banner
                .as_ref()
                .and_then(|banner| pictures::fetched(banner, crate::sidebar::WIDTH));
            match banner {
                Some(bytes) => column_of([
                    image(bytes)
                        .fit(ImageFit::Cover)
                        .size(Length::FILL, Length::px(BANNER))
                        .any(),
                    bar(space_info.name.clone(), *space)
                        .background(Color::BLACK.with_alpha(0.35))
                        .position(0.0, 0.0)
                        .any(),
                ])
                .height(BANNER)
                .fill_width()
                .any(),
                None => bar(space_info.name.clone(), *space).any(),
            }
        }
    }
}

fn bar(name: String, space: SpaceId) -> Container {
    let mark = mark_of(("discord-space-header", space));
    container(
        row! {
            text(name).label().semibold().no_wrap().grow(1.0).min_width(0.0);
            icon(MaterialSymbol::EXPAND_MORE).extent(Length::px(18.0));
        }
        .align_items(Align::Center)
        .fill_width()
        .build()
        .keyed(mark),
    )
    .padding(Inset::xy(Space::ROOMY, Space::NONE))
    .align_y(Align::Center)
    .height(HEIGHT)
    .on_press(move |chat: &mut Chat| {
        chat.open_context_menu(
            Subject::Space(space),
            Anchor::new(mark, AnchorEdge::BelowStart),
        );
    })
    .focusless()
    .fill_width()
}

fn search(sidebar: &Sidebar) -> impl View {
    container(
        text_field(sidebar.search())
            .placeholder(tr!("discord.sidebar.find"))
            .control_size(ControlSize::Small)
            .on_change(|sidebar: &mut Sidebar| sidebar.searched())
            .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .align_y(Align::Center)
    .height(HEIGHT)
    .fill_width()
}
