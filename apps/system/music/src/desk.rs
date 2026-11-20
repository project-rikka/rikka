use rikka_app::prelude::*;
use rikka_ui::{Element, Handler, IntoHandler};
use rikka_widgets::{column, row};

use crate::dialogs::{self, STAGE};
use crate::library::Library;
use crate::menus::{self, Menus};
use crate::music::{Music, Stage};
use crate::player::Player;
use crate::widgets::context_area;
use crate::{pages, player_bar, queue, settings, sidebar, signal, signing};

const SEARCH: f32 = 420.0;
const SEARCH_FLOOR: f32 = 180.0;

pub(crate) fn view(music: &Music) -> impl View {
    match music.stage() {
        Stage::Ready => ready(music),
        Stage::Connecting | Stage::Choosing | Stage::Browser { .. } | Stage::Page(_) => {
            component(|music: &Music| music, signing::view)
        }
    }
}

fn ready(music: &Music) -> Element {
    let content = match music.settings_open() {
        true => component(|music: &Music| music, settings::view)
            .transition(fade())
            .keyed("settings"),
        false => desk(music).transition(fade()).keyed("desk"),
    };
    column! {
        content;
    }
    .fill()
    .any()
}

fn desk(music: &Music) -> Element {
    let body = column! {
        row! {
            component(|music: &Music| music, sidebar::view);
            rule_upright();
            component(|music: &Music| music, main).grow(1.0).fill_height();
            if music.queue_open() {
                row! {
                    rule_upright();
                    component(|music: &Music| music, queue::view);
                }
                .fill_height()
                .transition(slide(Edge::Trailing));
            }
        }
        .grow(1.0)
        .fill_width();
        rule();
        component(|music: &Music| music, player_bar::view);
    }
    .fill();
    let open = Handler::new(|menus: &mut Menus, at: Point| menus.open_at(at));
    let mut layers = vec![
        row! {
            spacer().width(0.0).fill_height().build().keyed(menus::MARK);
            context_area(body, open).grow(1.0).fill_height();
        }
        .fill()
        .build()
        .keyed(STAGE),
    ];
    layers.extend(menus::view(music));
    layers.extend(dialogs::view(music.dialogs()));
    layers.extend(signal::view(music, signal::PLAYER));
    column_of(layers).fill().any()
}

fn main(music: &Music) -> impl View {
    let (root, depth) = music.nav().trail_position();
    column! {
        if let Some(error_message) = music.player().error_message() {
            banner(error_message.to_owned(), true, |player: &mut Player| player.dismiss());
        }
        page_stack((root, depth), component(|music: &Music| music, pages::view))
            .grow(1.0)
            .fill_width();
        if let Some(note) = music.library().note() {
            banner(note.message.clone(), note.failed, |library: &mut Library| library.dismiss());
        }
    }
    .fill()
}

fn banner<M>(message: String, failed: bool, dismiss: impl IntoHandler<(), (), M>) -> impl View {
    let (symbol, tint) = match failed {
        true => (MaterialSymbol::ERROR, palette().danger),
        false => (MaterialSymbol::CHECK_CIRCLE, palette().accent),
    };
    container(
        row! {
            icon(symbol).extent(Length::px(18.0)).color(tint);
            text(message).caption().grow(1.0).min_width(0.0);
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(16.0)))
                .ghost()
                .icon_only()
                .small()
                .on_press(dismiss);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
    .background(tint.with_alpha(0.1))
    .radius(Corner::Panel)
    .margin(Inset::xy(Space::LOOSE, Space::SNUG))
    .transition(fade())
}

pub(crate) fn caption_bar(music: &Music) -> Option<CaptionBar> {
    let bar = match (music.stage(), music.settings_open()) {
        (Stage::Ready, true) => settings_caption(),
        (Stage::Ready, false) => desk_caption(music),
        _ => caption().leading(title()),
    };
    Some(bar.into())
}

fn title() -> impl View {
    row! {
        icon(MaterialSymbol::GRAPHIC_EQ).extent(Length::px(16.0)).color(palette().accent);
        text(tr!("music.title")).label().medium().no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn settings_caption() -> Caption {
    caption()
        .leading(
            button(
                row! {
                    icon(MaterialSymbol::ARROW_BACK).extent(Length::px(16.0));
                    text(tr!("music.settings.back")).label();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .ghost()
            .small()
            .on_press(|music: &mut Music| music.open_settings(false)),
        )
        .center(text(tr!("music.settings.title")).label().medium().no_wrap())
}

fn desk_caption(music: &Music) -> Caption {
    caption()
        .leading(
            row! {
                button(icon(MaterialSymbol::ARROW_BACK).extent(Length::px(18.0)))
                    .ghost()
                    .icon_only()
                    .small()
                    .enabled(music.nav().can_go_back())
                    .on_press(|music: &mut Music| music.back());
                button(icon(MaterialSymbol::ARROW_FORWARD).extent(Length::px(18.0)))
                    .ghost()
                    .icon_only()
                    .small()
                    .enabled(music.nav().can_go_forward())
                    .on_press(|music: &mut Music| music.forward());
                title();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .center(
            text_field(music.field())
                .placeholder(tr!("music.search.placeholder"))
                .control_size(ControlSize::Small)
                .claims_focus(music.seeking())
                .on_blur(|music: &mut Music| music.seek_finished())
                .on_change(|music: &mut Music| music.typed())
                .on_submit(|music: &mut Music| music.search_now())
                .width(SEARCH)
                .shrink(1.0)
                .min_width(SEARCH_FLOOR),
        )
        .trailing(
            button(icon(MaterialSymbol::SETTINGS).extent(Length::px(17.0)))
                .icon_only()
                .ghost()
                .small()
                .on_press(|music: &mut Music| music.open_settings(true)),
        )
}
