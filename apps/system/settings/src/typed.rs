use rikka_app::prelude::*;
use rikka_widgets::row;

pub(crate) fn secret<C, S>(
    document: &TextDocument,
    hint: Label,
    shown: bool,
    width: f32,
    change: impl IntoHandler<(), (), C>,
    show: impl IntoHandler<(bool,), (), S>,
) -> Flex {
    let show = show.into_handler();
    row! {
        text_field(document)
            .placeholder(hint)
            .secret(!shown)
            .on_change(change)
            .width(width);
        button(icon(if shown {
            MaterialSymbol::VISIBILITY_OFF
        } else {
            MaterialSymbol::VISIBILITY
        }))
        .ghost()
        .icon_only()
        .on_press(show.map(move |()| (!shown,)));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}
