use discord_core::{PostLayout, PostSort};
use discord_preferences::ForumView;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{menu, menu_header, menu_item, menu_separator};

use super::SORT_BUTTON;
use crate::chat::Chat;
use crate::conversation::overlay::Overlay;

const WIDTH: f32 = 240.0;

pub(crate) fn sort_menu(chat: &Chat) -> Option<Element> {
    let (_, forum) = chat.conversation().shown()?;
    let settings = chat.current()?.channel(forum)?.forum.as_ref()?;
    let view = *chat.conversation().forum().view;
    let (sort, layout) = (view.sort_in(settings), view.layout_in(settings));
    let items = [
        menu_header(tr!("discord.forum.sort-by")),
        menu_item(tr!("discord.forum.sort-activity"))
            .selected(sort == PostSort::Activity)
            .on_press(|chat: &mut Chat| chat.sort_posts(PostSort::Activity)),
        menu_item(tr!("discord.forum.sort-created"))
            .selected(sort == PostSort::Created)
            .on_press(|chat: &mut Chat| chat.sort_posts(PostSort::Created)),
        menu_separator(),
        menu_header(tr!("discord.forum.view-as")),
        menu_item(tr!("discord.forum.view-list"))
            .icon(MaterialSymbol::VIEW_AGENDA)
            .selected(layout == PostLayout::List)
            .on_press(|chat: &mut Chat| chat.lay_out_posts(PostLayout::List)),
        menu_item(tr!("discord.forum.view-gallery"))
            .icon(MaterialSymbol::GRID_VIEW)
            .selected(layout == PostLayout::Gallery)
            .on_press(|chat: &mut Chat| chat.lay_out_posts(PostLayout::Gallery)),
        menu_separator(),
        menu_item(tr!("discord.forum.reset"))
            .icon(MaterialSymbol::RESTART_ALT)
            .enabled(view != ForumView::default())
            .on_press(|chat: &mut Chat| chat.reset_forum_view()),
    ];
    Some(
        menu()
            .items(items)
            .width(WIDTH)
            .on_activate(|overlay: &mut Overlay, _: usize| overlay.close())
            .on_dismiss(|overlay: &mut Overlay| overlay.close())
            .build()
            .keyed("discord-forum-sort-menu")
            .anchored(
                Anchor::new(SORT_BUTTON, AnchorEdge::BelowStart).offset(0.0, Space::TIGHT.px()),
            ),
    )
}
