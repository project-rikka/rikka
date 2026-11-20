use rikka_app::prelude::*;
use rikka_widgets::row;

use super::Sidebar;
use crate::account::Scope;
use crate::chat::Chat;
use crate::pictures;

pub(crate) const HEIGHT: f32 = 48.0;
const BANNER: f32 = 136.0;

pub(crate) fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().height(HEIGHT).any();
    };
    match &linked.location().scope {
        Scope::Home => search(chat.sidebar()).any(),
        Scope::Space(space) => {
            let Some(space_info) = linked.space(space) else {
                return bar(String::new()).any();
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
                    bar(space_info.name.clone())
                        .background(Color::BLACK.with_alpha(0.35))
                        .position(0.0, 0.0)
                        .any(),
                ])
                .height(BANNER)
                .fill_width()
                .any(),
                None => bar(space_info.name.clone()).any(),
            }
        }
    }
}

fn bar(name: String) -> Container {
    container(
        row! {
            text(name).label().semibold().no_wrap().grow(1.0).min_width(0.0);
        }
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::ROOMY, Space::NONE))
    .align_y(Align::Center)
    .height(HEIGHT)
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
